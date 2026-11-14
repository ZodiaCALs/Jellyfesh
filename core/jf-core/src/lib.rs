//! The Jellyfesh core facade: commands in, state snapshots and events out,
//! on one actor thread.
//!
//! A UI (a native shell through FFI, or the `jf` CLI) does three things:
//!
//! 1. [`Core::subscribe`] to receive [`Event`]s, and [`Core::snapshot`] for
//!    the current state;
//! 2. [`Core::send`] [`Command`]s, which the actor applies in order;
//! 3. read paged library data with the query methods ([`Core::search`],
//!    [`Core::tracks`], …), which run on the caller's thread against a
//!    read-only connection.
//!
//! Progress bars and lyrics never poll: they extrapolate from the
//! [`PlaybackAnchor`] in the playback slice (see `docs/architecture.md`).
//!
//! Online sources are plugins run by `jf-plugin-host` inside the core
//! ([`SourcesConfig`]; the `Sources` slice and the source commands). Their
//! secrets go to a [`SecretStore`]: the system keystore by default, or one
//! the app passes in.

mod actor;
mod items;
pub mod lyrics;
pub mod paths;
mod secrets;
mod sources;
pub mod targets;
pub mod theme;
pub mod timing;
mod types;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, unbounded};

pub use jf_engine::{
    BandKind, CrossfadeSettings, DeviceInfo, EqBand, EqSettings, OutputKind, PlaybackAnchor, ReplayGainMode,
    TrackInfo, monotonic_now_ns,
};
pub use jf_library::{
    Album, AlbumSort, Artist, LibraryStats, ScanProgress, ScanReport, SearchResults, Track, TrackQuery,
    TrackSort,
};
pub use jf_lyrics::{AuxKind, AuxTrack, Cursor, Line, LyricDoc, LyricTimeline, Word};
pub use jf_palette::{Appearance, Theme};
/// What embedders need of the plugin host: the secret store they may pass
/// in [`SourcesConfig::secrets`], and the knobs in [`SourcesConfig::host`].
pub use jf_plugin_host::{
    HostOptions, Limits, MemorySecretStore, SecretError, SecretStore, trust::TrustList,
};
pub use jf_session::{HistoryEntry, QueueItem, RepeatMode, TrackRef};
pub use timing::DisplayClock;
pub use types::*;

use actor::{Actor, Msg, Shared};
use jf_library::Library;

/// Errors from the core.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("library: {0}")]
    Library(#[from] jf_library::LibraryError),
    #[error("session: {0}")]
    Session(#[from] jf_session::SessionError),
    #[error("queue: {0}")]
    Queue(#[from] jf_session::QueueError),
    #[error("audio: {0}")]
    Engine(#[from] jf_engine::EngineError),
    #[error("lyrics: {0}")]
    Lyrics(#[from] jf_lyrics::LyricsError),
    #[error("theme: {0}")]
    Palette(#[from] jf_palette::PaletteError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// A running core. Cheap to share between threads; dropping it saves the
/// session and stops playback.
pub struct Core {
    tx: Sender<Msg>,
    shared: Arc<Shared>,
    queries: Mutex<Library>,
    next_id: AtomicU64,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("running", &self.thread.is_some())
            .finish_non_exhaustive()
    }
}

impl Core {
    /// Opens the library and session in `config.data_dir`, restores the
    /// session and starts the actor. Audio output opens on first playback.
    pub fn start(config: Config) -> Result<Core, CoreError> {
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.cache_dir)?;
        // The first open creates and migrates the catalogue; the actor and
        // its workers open their own connections after it.
        let queries = Library::open(&config.data_dir.join("library.sqlite"), &config.cache_dir)?;
        let (tx, rx) = unbounded();
        let (actor, shared) = Actor::new(config, tx.clone())?;
        let thread = thread::Builder::new()
            .name("jf-core".into())
            .spawn(move || actor.run(rx))?;
        Ok(Core {
            tx,
            shared,
            queries: Mutex::new(queries),
            next_id: AtomicU64::new(1),
            thread: Some(thread),
        })
    }

    /// Queues a command for the actor. A failure comes back as
    /// [`Event::CommandFailed`] with the returned id.
    pub fn send(&self, command: Command) -> CommandId {
        let id = CommandId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let _ = self.tx.send(Msg::Command(id, command));
        id
    }

    /// A new event stream. Subscribe before sending commands whose events
    /// you need.
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = unbounded();
        self.shared
            .subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx);
        rx
    }

    /// The current state of every slice.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot()
    }

    /// A page of the queue in playback order.
    pub fn queue_window(&self, offset: usize, limit: usize) -> Vec<QueueItem> {
        self.shared.read_queue().window(offset, limit)
    }

    /// Recently played tracks, oldest first.
    pub fn history(&self) -> Vec<HistoryEntry> {
        self.shared.read_queue().history().cloned().collect()
    }

    pub fn track(&self, id: i64) -> Result<Option<Track>, CoreError> {
        Ok(self.library()?.track(id)?)
    }

    pub fn tracks(&self, query: &TrackQuery) -> Result<Vec<Track>, CoreError> {
        Ok(self.library()?.tracks(query)?)
    }

    pub fn albums(&self, offset: usize, limit: usize) -> Result<Vec<Album>, CoreError> {
        Ok(self.library()?.albums(offset, limit)?)
    }

    pub fn artists(&self, offset: usize, limit: usize) -> Result<Vec<Artist>, CoreError> {
        Ok(self.library()?.artists(offset, limit)?)
    }

    pub fn search(&self, text: &str, limit: usize) -> Result<SearchResults, CoreError> {
        Ok(self.library()?.search(text, limit)?)
    }

    pub fn roots(&self) -> Result<Vec<std::path::PathBuf>, CoreError> {
        Ok(self.library()?.roots()?)
    }

    pub fn stats(&self) -> Result<LibraryStats, CoreError> {
        Ok(self.library()?.stats()?)
    }

    /// Library tracks to play for a search. A query that names an album
    /// exactly plays that album in disc and track order; one that names an
    /// artist plays their tracks album by album; anything else plays the
    /// matching tracks in result order.
    pub fn tracks_for_query(&self, text: &str, limit: usize) -> Result<Vec<TrackRef>, CoreError> {
        let library = self.library()?;
        let results = library.search(text, limit)?;
        let wanted = text.trim().to_lowercase();
        let all = |q: TrackQuery| -> Result<Vec<TrackRef>, CoreError> {
            Ok(library
                .tracks(&q)?
                .into_iter()
                .map(|t| TrackRef::Library(t.id))
                .collect())
        };
        if let Some(album) = results.albums.iter().find(|a| a.title.to_lowercase() == wanted) {
            return all(TrackQuery {
                album_id: Some(album.id),
                sort: TrackSort::Album,
                limit: usize::MAX,
                ..TrackQuery::default()
            });
        }
        if let Some(artist) = results.artists.iter().find(|a| a.name.to_lowercase() == wanted) {
            return all(TrackQuery {
                artist_id: Some(artist.id),
                sort: TrackSort::Artist,
                limit: usize::MAX,
                ..TrackQuery::default()
            });
        }
        Ok(results.tracks.iter().map(|t| TrackRef::Library(t.id)).collect())
    }

    /// The tracks of an album, by disc and track number.
    pub fn album_tracks(&self, album_id: i64) -> Result<Vec<Track>, CoreError> {
        Ok(self.library()?.album_tracks(album_id)?)
    }

    /// The albums an artist appears on, by year then title.
    pub fn artist_albums(&self, artist_id: i64) -> Result<Vec<Album>, CoreError> {
        Ok(self.library()?.artist_albums(artist_id)?)
    }

    /// A page of albums in the given order.
    pub fn albums_sorted(
        &self,
        sort: AlbumSort,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Album>, CoreError> {
        Ok(self.library()?.albums_sorted(sort, descending, offset, limit)?)
    }

    /// The most recently added tracks.
    pub fn recently_added(&self, limit: usize) -> Result<Vec<Track>, CoreError> {
        Ok(self.library()?.recently_added(limit)?)
    }

    /// Recently played tracks (plays that counted), newest first.
    pub fn recently_played(&self, limit: usize) -> Result<Vec<TrackRow>, CoreError> {
        let rows = self.library()?.recently_played(limit)?;
        Ok(rows
            .iter()
            .map(|r| items::row(&r.item, r.track.as_ref(), r.favorite))
            .collect())
    }

    /// The most played tracks, most first.
    pub fn most_played(&self, limit: usize) -> Result<Vec<TrackRow>, CoreError> {
        let rows = self.library()?.most_played(limit)?;
        Ok(rows
            .iter()
            .map(|r| items::row(&r.item, r.track.as_ref(), r.favorite))
            .collect())
    }

    /// Every playlist, by name.
    pub fn playlists(&self) -> Result<Vec<PlaylistSummary>, CoreError> {
        let lists = self.library()?.playlists()?;
        Ok(lists.into_iter().map(items::playlist_summary).collect())
    }

    /// One playlist, or `None` when there is no such playlist.
    pub fn playlist(&self, id: i64) -> Result<Option<PlaylistSummary>, CoreError> {
        Ok(self.library()?.playlist(id)?.map(items::playlist_summary))
    }

    /// Up to `limit` entries of a playlist in order, from position `offset`.
    pub fn playlist_entries(
        &self,
        id: i64,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PlaylistEntry>, CoreError> {
        let library = self.library()?;
        let entries = library.playlist_entries(id, offset, limit)?;
        entries
            .iter()
            .map(|e| {
                let favorite = library.is_favorite(&e.item)?;
                Ok(PlaylistEntry {
                    entry_id: e.entry_id,
                    position: u32::try_from(e.position).unwrap_or(u32::MAX),
                    row: items::row(&e.item, e.track.as_ref(), favorite),
                })
            })
            .collect()
    }

    /// Writes a playlist to `path` as M3U8 (UTF-8, with `#EXTINF` lines),
    /// replacing the file atomically. Source entries are left out.
    pub fn export_playlist(&self, id: i64, path: &std::path::Path) -> Result<(), CoreError> {
        Ok(self.library()?.export_m3u(id, path)?)
    }

    /// A page of favourites, the latest first.
    pub fn favorites(&self, offset: usize, limit: usize) -> Result<Vec<TrackRow>, CoreError> {
        let rows = self.library()?.favorites(offset, limit)?;
        Ok(rows
            .iter()
            .map(|r| items::row(&r.item, r.track.as_ref(), true))
            .collect())
    }

    /// Whether a track is a favourite.
    pub fn is_favorite(&self, track: &TrackRef) -> Result<bool, CoreError> {
        Ok(self.library()?.is_favorite(&items::item_for(track))?)
    }

    /// A square-fitted JPEG of the artwork with this content hash, at about
    /// `size_px` (64, 128, 256, 512 or 1024), created on first use. `None`
    /// for an unknown hash.
    pub fn artwork_thumbnail(
        &self,
        hash: &str,
        size_px: u32,
    ) -> Result<Option<std::path::PathBuf>, CoreError> {
        Ok(self.library()?.artwork_thumbnail(hash, size_px)?)
    }

    /// Stops the actor after it has saved the session. Plugin installs,
    /// uninstalls and logouts already sent finish first (at most a minute),
    /// and their results are still reported to subscribers.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    fn library(&self) -> Result<std::sync::MutexGuard<'_, Library>, CoreError> {
        Ok(self.queries.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Audio output devices, for a device picker.
pub fn output_devices() -> Result<Vec<DeviceInfo>, CoreError> {
    Ok(jf_engine::list_devices()?)
}

/// The equalizer presets every app offers, in their order. Applying one sets
/// [`Settings::eq`] to its settings (keeping the person's preamp is up to the app).
pub fn eq_presets() -> Vec<EqPreset> {
    EqSettings::presets()
        .into_iter()
        .map(|(id, settings)| EqPreset {
            id: id.to_owned(),
            settings,
        })
        .collect()
}

impl Drop for Core {
    fn drop(&mut self) {
        self.stop();
    }
}
