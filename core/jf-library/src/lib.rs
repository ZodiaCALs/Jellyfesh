//! Music library: SQLite catalogue, scanner, tag reading, artwork cache and search.
//!
//! [`Library`] owns one SQLite connection (WAL, foreign keys on). It is `Send` but
//! not `Sync`: the core runs it on its library worker thread and sends it requests.
//!
//! Catalogue rules:
//!
//! * One row per audio file. Missing tags are stored as NULL (UIs show "Unknown");
//!   only the title falls back, to the file stem.
//! * Albums group tracks by normalised album title and album artist, or the
//!   primary track artist when there is no album artist. Tracks without an album
//!   tag belong to no album.
//! * Artist tags are split conservatively for [`Artist`] links: on `;`, on ` / `
//!   with spaces, and on `feat.`/`ft.`/`featuring`; never on `&` or `,`. Several
//!   tag values, or an ARTISTS tag, are used as given.
//! * Text for matching is NFKC-normalised, lowercased and stripped of Latin
//!   diacritics, both when indexed and when searched ([`Library::search`]).
//! * Paths are stored as text; files whose paths are not valid UTF-8 are reported
//!   in [`ScanReport::errors`] and skipped.
//!
//! Besides the catalogue the library keeps playlists ([`Library::create_playlist`],
//! with M3U8 import and export), favourites and play statistics
//! ([`Library::set_favorite`], [`Library::record_play`]) and artwork thumbnails
//! ([`Library::artwork_thumbnail`]). Playlist entries and statistics refer to
//! tracks by path, so they survive a track being removed and scanned in again.
//! Every change bumps [`Library::revision`].
//!
//! [`LibraryWatcher`] watches the roots for changes, for [`Library::scan_paths`]
//! to rescan just the changed files and folders.
//!
//! ```no_run
//! use jf_library::{Library, TrackQuery};
//! # fn main() -> Result<(), jf_library::LibraryError> {
//! let lib = Library::open("library.db".as_ref(), "cache".as_ref())?;
//! lib.add_root("/music".as_ref())?;
//! let report = lib.scan(&mut |p| eprintln!("{} {}", p.files_seen, p.current.display()))?;
//! println!("{} added, {} errors", report.added, report.errors.len());
//! for track in lib.tracks(&TrackQuery::default())? {
//!     println!("{} — {}", track.artist.as_deref().unwrap_or("Unknown"), track.title);
//! }
//! # Ok(()) }
//! ```

mod artwork;
mod error;
mod fs;
mod item;
mod lyrics;
mod normalize;
mod playlist;
mod query;
mod scan;
mod schema;
mod search;
mod stats;
mod tags;
mod thumbnail;
mod util;
mod watch;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub use error::LibraryError;
pub use fs::{FileEntry, FileSystem, PathKind, ReadSeek, StdFileSystem};
pub use watch::{LibraryWatcher, WatchErrorKind};

/// Row id of a track. Stable while the file keeps its path.
pub type TrackId = i64;

/// One audio file in the catalogue.
///
/// Missing tags are `None`; UIs show "Unknown". Only the title falls back, to
/// the file name without its extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    /// The [`Album`] this track is grouped into; `None` without an album tag.
    pub album_id: Option<i64>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    /// Short lowercase codec name: "mp3", "flac", "aac", "alac", "vorbis", "opus", "pcm", …
    pub codec: Option<String>,
    pub replaygain: ReplayGain,
    pub isrc: Option<String>,
    pub has_embedded_lyrics: bool,
    /// blake3 hex of the embedded front cover; see [`Library::artwork_path`].
    pub artwork_hash: Option<String>,
    pub file_size: u64,
    pub mtime_ns: i64,
}

/// ReplayGain values from the tags (gains in dB, peaks as linear amplitude).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Tracks grouped by album title and album artist (or, without one, the
/// primary track artist).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub album_artist: Option<String>,
    /// Earliest year among the album's tracks.
    pub year: Option<i32>,
    pub track_count: u32,
    /// Artwork of the first track (by disc and track number) that has some.
    pub artwork_hash: Option<String>,
}

/// One artist, after splitting multi-artist tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
}

/// Reported once per audio file the scan visits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanProgress {
    /// Audio files visited so far.
    pub files_seen: u64,
    /// Of those, files that were new or changed and had their tags read.
    pub files_changed: u64,
    pub current: PathBuf,
}

/// What a scan did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub unchanged: u64,
    /// Files (or, for walk errors, roots) that could not be read, with the reason.
    pub errors: Vec<(PathBuf, String)>,
}

/// Sort order for [`Library::tracks`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackSort {
    /// Artist, then album, disc, track number and title. Unknown artists last.
    #[default]
    Artist,
    Title,
    /// Album, then disc, track number and title. Unknown albums last.
    Album,
    Year,
    Duration,
    Path,
    /// When the file was first scanned.
    Added,
}

/// A page of tracks for [`Library::tracks`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackQuery {
    pub offset: usize,
    /// Maximum number of tracks returned (100 by default).
    pub limit: usize,
    pub sort: TrackSort,
    pub descending: bool,
    /// Only tracks of this album.
    pub album_id: Option<i64>,
    /// Only tracks credited to this artist.
    pub artist_id: Option<i64>,
}

impl Default for TrackQuery {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
            sort: TrackSort::default(),
            descending: false,
            album_id: None,
            artist_id: None,
        }
    }
}

/// Results of [`Library::search`], each list ranked best first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

/// Counts for a status line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryStats {
    pub roots: u64,
    pub tracks: u64,
    pub albums: u64,
    pub artists: u64,
    /// Distinct cached artwork images.
    pub artwork: u64,
    pub total_duration_ms: u64,
    pub total_bytes: u64,
}

/// Sort order for [`Library::albums_sorted`]. Unknown artists and years sort
/// last in both directions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlbumSort {
    /// Title, then album artist.
    #[default]
    Title,
    /// Album artist (or the primary track artist without one), then year and title.
    Artist,
    /// Earliest year among the album's tracks, then title.
    Year,
    /// When the album's first track was first scanned.
    Added,
}

/// What a playlist entry, a favourite or a play statistic refers to.
///
/// Its stable key ([`Library::item_key`]) is the file path for library tracks
/// and files (so both share favourites and counts) and `plugin:id` for plugin
/// items.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaylistItem {
    /// A track in the catalogue. Stored with its path: when the file is removed
    /// and scanned in again, the entry finds the new track (and its new id).
    Library(TrackId),
    /// An audio file by path, whether or not it is in the catalogue. Relative
    /// paths are made absolute against the current directory.
    File(PathBuf),
    /// An item of a source plugin.
    Source {
        /// The plugin's id, such as `org.jellyfesh.subsonic`.
        plugin: String,
        /// The item's id within the plugin.
        id: String,
        /// The plugin's metadata for the item, stored as given. When it is a JSON
        /// object with a numeric `duration_ms`, that counts toward
        /// [`Playlist::duration_ms`].
        meta_json: String,
    },
}

/// A user playlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    /// Never reused, even after the playlist is deleted.
    pub id: i64,
    pub name: String,
    pub entry_count: u32,
    /// The known durations added up: entries whose track is in the catalogue,
    /// and plugin items with a `duration_ms` in their metadata.
    pub duration_ms: u64,
    /// Milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    /// The last change to the name or the entries, in milliseconds since the Unix epoch.
    pub updated_at_ms: i64,
    /// The artwork of the first entry whose track has some.
    pub artwork_hash: Option<String>,
}

/// One entry of a playlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// Stable while the entry is in the playlist, and never reused.
    pub entry_id: i64,
    /// Index in the playlist, from 0, without gaps.
    pub position: usize,
    /// A library item carries the id of the track its path has now. While the
    /// file is not in the catalogue it reads as [`PlaylistItem::File`] with its
    /// path (track ids are reused, so the id it last had may name another
    /// track by now), and as a library item again once the file is scanned back in.
    pub item: PlaylistItem,
    /// The catalogue track at the entry's path, for library and file entries.
    /// `None` when the file is not (or no longer) in the catalogue, and for
    /// plugin items.
    pub track: Option<Track>,
}

/// An item with its favourite state and play statistics: the rows of
/// [`Library::favorites`], [`Library::most_played`] and
/// [`Library::recently_played`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStats {
    /// As last given to [`Library::set_favorite`] or [`Library::record_play`]; a
    /// library item carries the id of the track its path has now, and reads as
    /// a file while its file is not in the catalogue (see [`PlaylistEntry::item`]).
    pub item: PlaylistItem,
    /// The catalogue track at the item's path; see [`PlaylistEntry::track`].
    pub track: Option<Track>,
    pub favorite: bool,
    /// When it became a favourite, in milliseconds since the Unix epoch.
    pub favorited_at_ms: Option<i64>,
    /// Plays recorded as counting (completed or past half, as the caller decides).
    pub play_count: u64,
    /// Plays recorded as not counting.
    pub skip_count: u64,
    /// The latest counted play, as given to [`Library::record_play`].
    pub last_played_ms: Option<i64>,
}

/// The music library.
pub struct Library {
    conn: Connection,
    cache_dir: PathBuf,
    fs: Box<dyn FileSystem>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library")
            .field("db", &self.conn.path())
            .field("cache_dir", &self.cache_dir)
            .finish_non_exhaustive()
    }
}

impl Library {
    /// Opens (creating and migrating if needed) the database at `db_path`, reading
    /// music through [`StdFileSystem`]. Artwork is cached under `cache_dir/artwork`.
    pub fn open(db_path: &Path, cache_dir: &Path) -> Result<Self, LibraryError> {
        Self::open_with_fs(db_path, cache_dir, Box::new(StdFileSystem))
    }

    /// Like [`Library::open`], reading music through `fs`.
    pub fn open_with_fs(
        db_path: &Path,
        cache_dir: &Path,
        fs: Box<dyn FileSystem>,
    ) -> Result<Self, LibraryError> {
        Self::open_inner(db_path, cache_dir, fs, schema::SCHEMA_VERSION)
    }

    /// Opens the database migrated no further than schema `version`, to test
    /// migrations from older databases. Not for the app: methods that need a
    /// newer schema fail on such a database.
    #[doc(hidden)]
    pub fn open_at_schema_version(
        db_path: &Path,
        cache_dir: &Path,
        version: i64,
    ) -> Result<Self, LibraryError> {
        Self::open_inner(db_path, cache_dir, Box::new(StdFileSystem), version)
    }

    fn open_inner(
        db_path: &Path,
        cache_dir: &Path,
        fs: Box<dyn FileSystem>,
        version: i64,
    ) -> Result<Self, LibraryError> {
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        schema::migrate_to(&conn, version)?;
        Ok(Self {
            conn,
            cache_dir: cache_dir.to_path_buf(),
            fs,
        })
    }

    /// Adds a folder to scan and returns its id. Adding an existing root returns
    /// its id and changes nothing. Roots may nest: a file belongs to the deepest
    /// root that contains it.
    pub fn add_root(&self, path: &Path) -> Result<i64, LibraryError> {
        let key = root_key(path)?;
        let tx = self.conn.unchecked_transaction()?;
        let inserted = tx.execute(
            "INSERT INTO roots (path, added_at) VALUES (?1, ?2) ON CONFLICT (path) DO NOTHING",
            params![key, scan::unix_now()],
        )?;
        let id = tx.query_row("SELECT id FROM roots WHERE path = ?1", [&key], |row| row.get(0))?;
        if inserted > 0 {
            scan::bump_revision(&tx)?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Removes a root and every track that belongs to it.
    pub fn remove_root(&self, path: &Path) -> Result<(), LibraryError> {
        let key = root_key(path)?;
        let tx = self.conn.unchecked_transaction()?;
        let root_id: Option<i64> = tx
            .query_row("SELECT id FROM roots WHERE path = ?1", [&key], |row| row.get(0))
            .optional()?;
        let Some(root_id) = root_id else {
            return Err(LibraryError::RootNotFound(path.to_path_buf()));
        };
        tx.execute("DELETE FROM tracks WHERE root_id = ?1", [root_id])?;
        tx.execute("DELETE FROM roots WHERE id = ?1", [root_id])?;
        let stale_art = scan::delete_orphans(&tx)?;
        scan::bump_revision(&tx)?;
        tx.commit()?;
        self.delete_artwork_files(&stale_art);
        Ok(())
    }

    /// The library's roots, sorted by path.
    pub fn roots(&self) -> Result<Vec<PathBuf>, LibraryError> {
        Ok(self.root_rows()?.into_iter().map(|(_, path)| path).collect())
    }

    pub(crate) fn root_rows(&self) -> Result<Vec<(i64, PathBuf)>, LibraryError> {
        let rows = self
            .conn
            .prepare_cached("SELECT id, path FROM roots ORDER BY path")?
            .query_map([], |row| {
                Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?)))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Scans every root incrementally: new and changed files are read, unchanged
    /// ones (same size and mtime) skipped, vanished ones removed. Unreadable files
    /// are listed in [`ScanReport::errors`] and never stop the scan.
    pub fn scan(&self, progress: &mut dyn FnMut(ScanProgress)) -> Result<ScanReport, LibraryError> {
        scan::scan(self, progress)
    }

    /// Rescans only `paths`, files or folders, as reported by [`LibraryWatcher`]:
    /// new and changed audio files at or below them are read, unchanged ones
    /// skipped, and tracks at or below them whose files are gone are removed.
    /// Nothing else is looked at, so tracks elsewhere stay as they are.
    ///
    /// A path above library roots stands for those roots. Paths outside every
    /// root are reported in [`ScanReport::errors`], as are unreadable files. A
    /// path whose whole root is missing (an unmounted drive) removes nothing.
    pub fn scan_paths(
        &self,
        paths: &[PathBuf],
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<ScanReport, LibraryError> {
        scan::scan_paths(self, paths, progress)
    }

    /// Searches titles, artists and albums; up to `limit` tracks, albums and artists.
    ///
    /// Case, width (full-width Latin, half-width katakana) and Latin diacritics are
    /// ignored. Exact and prefix title matches rank first, then word-prefix matches,
    /// then substrings. One- and two-character queries (common for CJK titles) use a
    /// bounded substring scan. The text is never interpreted as FTS5 syntax.
    pub fn search(&self, text: &str, limit: usize) -> Result<SearchResults, LibraryError> {
        let Some(query) = search::Query::parse(text) else {
            return Ok(SearchResults::default());
        };
        if limit == 0 {
            return Ok(SearchResults::default());
        }
        let tracks = search::search(&self.conn, &search::TRACKS, &query, limit)?
            .into_iter()
            .filter_map(|id| query::track(&self.conn, id).transpose())
            .collect::<Result<_, _>>()?;
        let albums = search::search(&self.conn, &search::ALBUMS, &query, limit)?
            .into_iter()
            .filter_map(|id| query::album(&self.conn, id).transpose())
            .collect::<Result<_, _>>()?;
        let artists = search::search(&self.conn, &search::ARTISTS, &query, limit)?
            .into_iter()
            .filter_map(|id| query::artist(&self.conn, id).transpose())
            .collect::<Result<_, _>>()?;
        Ok(SearchResults {
            tracks,
            albums,
            artists,
        })
    }

    /// One track by id.
    pub fn track(&self, id: TrackId) -> Result<Option<Track>, LibraryError> {
        query::track(&self.conn, id)
    }

    /// A sorted, filtered page of tracks.
    pub fn tracks(&self, q: &TrackQuery) -> Result<Vec<Track>, LibraryError> {
        query::tracks(&self.conn, q)
    }

    /// A page of albums sorted by title, then album artist.
    pub fn albums(&self, offset: usize, limit: usize) -> Result<Vec<Album>, LibraryError> {
        query::albums_sorted(&self.conn, AlbumSort::Title, false, offset, limit)
    }

    /// A page of albums in the given order.
    pub fn albums_sorted(
        &self,
        sort: AlbumSort,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Album>, LibraryError> {
        query::albums_sorted(&self.conn, sort, descending, offset, limit)
    }

    /// An artist's albums: those with a track credited to the artist and those
    /// grouped under the artist's name. By year (unknown last), then title.
    pub fn artist_albums(&self, artist_id: i64) -> Result<Vec<Album>, LibraryError> {
        query::artist_albums(&self.conn, artist_id)
    }

    /// An album's tracks by disc (none counts as disc 1), track number (none
    /// last) and title.
    pub fn album_tracks(&self, album_id: i64) -> Result<Vec<Track>, LibraryError> {
        query::album_tracks(&self.conn, album_id)
    }

    /// The `limit` tracks first scanned most recently, newest first.
    pub fn recently_added(&self, limit: usize) -> Result<Vec<Track>, LibraryError> {
        query::recently_added(&self.conn, limit)
    }

    /// A page of artists sorted by name.
    pub fn artists(&self, offset: usize, limit: usize) -> Result<Vec<Artist>, LibraryError> {
        query::artists(&self.conn, offset, limit)
    }

    /// One album by id.
    pub fn album(&self, id: i64) -> Result<Option<Album>, LibraryError> {
        query::album(&self.conn, id)
    }

    /// One artist by id.
    pub fn artist(&self, id: i64) -> Result<Option<Artist>, LibraryError> {
        query::artist(&self.conn, id)
    }

    /// A counter that increases with every change to the catalogue, playlists,
    /// favourites and play statistics; UIs compare it to decide whether to refetch.
    pub fn revision(&self) -> Result<u64, LibraryError> {
        let value: i64 = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'revision'", [], |row| {
                row.get(0)
            })?;
        Ok(u64::try_from(value).unwrap_or(0))
    }

    /// The cached front cover of a track: `cache_dir/artwork/<blake3>.<ext>`.
    ///
    /// If the cache file was deleted (caches get purged), the cover is extracted
    /// from the audio file again. `None` when the track has no embedded artwork.
    pub fn artwork_path(&self, id: TrackId) -> Result<Option<PathBuf>, LibraryError> {
        let row: Option<(String, Option<String>, Option<String>)> = self
            .conn
            .prepare_cached(
                "SELECT t.path, t.artwork_hash, a.file_name
                 FROM tracks t LEFT JOIN artwork a ON a.hash = t.artwork_hash WHERE t.id = ?1",
            )?
            .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .optional()?;
        let Some((track_path, Some(hash), Some(file_name))) = row else {
            return Ok(None);
        };
        let cached = self.artwork_dir().join(&file_name);
        if cached.is_file() {
            return Ok(Some(cached));
        }
        self.extract_artwork(Path::new(&track_path), &hash)
    }

    /// A thumbnail of the artwork with this hash (see [`Track::artwork_hash`],
    /// [`Album::artwork_hash`], [`Playlist::artwork_hash`]):
    /// `cache_dir/artwork/<hash>/<size>.jpg`.
    ///
    /// The size snaps up to 64, 128, 256, 512 or 1024 pixels (1024 at most). The
    /// image keeps its aspect ratio and fits that square; it is never enlarged.
    /// It is made once (JPEG, quality 85, written atomically) and reused; it goes
    /// when the artwork leaves the catalogue. `None` for a hash the catalogue
    /// does not have. Covers that are neither JPEG nor PNG, or are damaged, fail
    /// with [`LibraryError::Artwork`].
    pub fn artwork_thumbnail(&self, hash: &str, size_px: u32) -> Result<Option<PathBuf>, LibraryError> {
        if !thumbnail::is_hash(hash) {
            return Ok(None);
        }
        let file_name: Option<String> = self
            .conn
            .prepare_cached("SELECT file_name FROM artwork WHERE hash = ?1")?
            .query_row([hash], |row| row.get(0))
            .optional()?;
        let Some(file_name) = file_name else {
            return Ok(None);
        };
        let size = thumbnail::bucket(size_px);
        let thumb = self.artwork_dir().join(hash).join(format!("{size}.jpg"));
        if thumb.is_file() {
            return Ok(Some(thumb));
        }
        let mut source = self.artwork_dir().join(file_name);
        if !source.is_file() {
            let track_path: Option<String> = self
                .conn
                .prepare_cached("SELECT path FROM tracks WHERE artwork_hash = ?1 ORDER BY id LIMIT 1")?
                .query_row([hash], |row| row.get(0))
                .optional()?;
            let restored = match track_path {
                Some(track_path) => self.extract_artwork(Path::new(&track_path), hash)?,
                None => None,
            };
            let Some(restored) = restored else {
                return Ok(None);
            };
            source = restored;
        }
        let bytes = std::fs::read(&source)?;
        thumbnail::make(&bytes, size, &thumb).map_err(|message| LibraryError::Artwork {
            hash: hash.to_owned(),
            message,
        })?;
        Ok(Some(thumb))
    }

    /// Extracts the front cover of `track_path` into the artwork cache again (caches
    /// get purged); `None` unless it is still the image with `hash`.
    fn extract_artwork(&self, track_path: &Path, hash: &str) -> Result<Option<PathBuf>, LibraryError> {
        let parsed = match tags::parse(self.fs.as_ref(), track_path, true) {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::warn!(path = %track_path.display(), %err, "cannot re-extract artwork");
                return Ok(None);
            }
        };
        let Some(picture) = parsed.front_cover() else {
            return Ok(None);
        };
        let stored = artwork::store(&self.artwork_dir(), picture.data(), picture.mime_type())?;
        // A different hash means the file changed since the scan; the next scan fixes it.
        Ok((stored.hash == hash).then(|| self.artwork_dir().join(stored.file_name)))
    }

    /// Creates an empty playlist and returns its id. The name is trimmed and
    /// must not be empty ([`LibraryError::EmptyPlaylistName`]); names need not
    /// be unique.
    pub fn create_playlist(&self, name: &str) -> Result<i64, LibraryError> {
        playlist::create(&self.conn, name)
    }

    /// Renames a playlist (the name as for [`Library::create_playlist`]).
    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<(), LibraryError> {
        playlist::rename(&self.conn, id, name)
    }

    /// Deletes a playlist with its entries.
    pub fn delete_playlist(&self, id: i64) -> Result<(), LibraryError> {
        playlist::delete(&self.conn, id)
    }

    /// Every playlist, by name.
    pub fn playlists(&self) -> Result<Vec<Playlist>, LibraryError> {
        playlist::list(&self.conn)
    }

    /// One playlist by id.
    pub fn playlist(&self, id: i64) -> Result<Option<Playlist>, LibraryError> {
        playlist::get(&self.conn, id)
    }

    /// Up to `limit` entries of a playlist in order, from position `offset`.
    /// Empty for a playlist that does not exist.
    pub fn playlist_entries(
        &self,
        id: i64,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PlaylistEntry>, LibraryError> {
        playlist::entries(&self.conn, id, offset, limit)
    }

    /// Inserts `items`, in order, at position `at` (appended when `None` or past
    /// the end) and returns their entry ids. All or nothing: a library item
    /// whose track does not exist fails with [`LibraryError::TrackNotFound`].
    /// An item can appear any number of times.
    pub fn add_to_playlist(
        &self,
        id: i64,
        items: &[PlaylistItem],
        at: Option<usize>,
    ) -> Result<Vec<i64>, LibraryError> {
        playlist::add(&self.conn, id, items, at)
    }

    /// Removes entries; ids that are not entries of this playlist are ignored.
    /// The entries after them move up.
    pub fn remove_from_playlist(&self, id: i64, entry_ids: &[i64]) -> Result<(), LibraryError> {
        playlist::remove(&self.conn, id, entry_ids)
    }

    /// Moves an entry to position `to` (the last position when past the end);
    /// the entries in between move by one.
    pub fn move_playlist_entry(&self, id: i64, entry_id: i64, to: usize) -> Result<(), LibraryError> {
        playlist::move_entry(&self.conn, id, entry_id, to)
    }

    /// Imports an M3U or M3U8 file (UTF-8, or UTF-16 with a byte-order mark) as a
    /// new playlist and returns its id.
    ///
    /// Relative paths are taken from the playlist's folder (backslashes read as
    /// separators outside Windows) and `file:` URLs are decoded. Files in the
    /// catalogue become library entries; others, missing ones included, file
    /// entries. Other URLs, comments and `#EXT` lines are skipped.
    pub fn import_m3u(&self, path: &Path, name: &str) -> Result<i64, LibraryError> {
        playlist::import_m3u(self, path, name)
    }

    /// Writes a playlist to `path` as M3U8 (UTF-8) with `#EXTINF` lines, atomically.
    ///
    /// Files below the playlist file's folder are written relative to it, others
    /// absolute. Plugin items have no file and are left out. The file is written
    /// with `std::fs` ([`FileSystem`] only reads), so `path` must be a local path.
    pub fn export_m3u(&self, id: i64, path: &Path) -> Result<(), LibraryError> {
        playlist::export_m3u(self, id, path)
    }

    /// The stable key of an item: the path of a library track or file, or
    /// `plugin:id` for a plugin item. `None` for a library id with no track.
    pub fn item_key(&self, item: &PlaylistItem) -> Result<Option<String>, LibraryError> {
        stats::item_key(&self.conn, item)
    }

    /// Marks an item as a favourite, or not. A library item's track must exist
    /// ([`LibraryError::TrackNotFound`]). Favouriting a favourite keeps its time.
    pub fn set_favorite(&self, item: &PlaylistItem, favorite: bool) -> Result<(), LibraryError> {
        stats::set_favorite(&self.conn, item, favorite)
    }

    /// Whether an item is a favourite; false for a library id with no track.
    pub fn is_favorite(&self, item: &PlaylistItem) -> Result<bool, LibraryError> {
        stats::is_favorite(&self.conn, item)
    }

    /// A page of favourites, the latest first.
    pub fn favorites(&self, offset: usize, limit: usize) -> Result<Vec<ItemStats>, LibraryError> {
        stats::favorites(&self.conn, offset, limit)
    }

    /// Records a play at `at_ms` (milliseconds since the Unix epoch).
    /// `completed` says whether it counts as a play (the caller decides: played
    /// to the end, or past half); otherwise it counts as a skip. A library item's
    /// track must exist ([`LibraryError::TrackNotFound`]).
    pub fn record_play(&self, item: &PlaylistItem, at_ms: i64, completed: bool) -> Result<(), LibraryError> {
        stats::record_play(&self.conn, item, at_ms, completed)
    }

    /// The `limit` items with the most counted plays; ties by latest play.
    pub fn most_played(&self, limit: usize) -> Result<Vec<ItemStats>, LibraryError> {
        stats::most_played(&self.conn, limit)
    }

    /// The `limit` items played (counted) most recently, latest first.
    pub fn recently_played(&self, limit: usize) -> Result<Vec<ItemStats>, LibraryError> {
        stats::recently_played(&self.conn, limit)
    }

    /// Lyrics embedded in the track's tags, as LRC text (or plain text when the
    /// lyrics are unsynced): ID3 USLT, Vorbis LYRICS, MP4 ©lyr, or ID3 SYLT
    /// converted to LRC. Read from the file on each call.
    pub fn embedded_lyrics(&self, id: TrackId) -> Result<Option<String>, LibraryError> {
        let Some(path) = self.track_path(id)? else {
            return Ok(None);
        };
        match tags::parse(self.fs.as_ref(), &path, false) {
            Ok(parsed) => Ok(parsed.lyrics()),
            Err(tags::ReadError::Io(err)) => Err(err.into()),
            Err(tags::ReadError::Format(message)) => Err(LibraryError::Tags { path, message }),
        }
    }

    /// A lyrics file next to the track with the same stem: `.ttml`, else `.lrc`,
    /// else `.vtt`. Returns its path and text (UTF-8, or UTF-16 with a BOM).
    pub fn sidecar_lyrics(&self, id: TrackId) -> Result<Option<(PathBuf, String)>, LibraryError> {
        Ok(self
            .track_path(id)?
            .and_then(|path| lyrics::find_sidecar(self.fs.as_ref(), &path)))
    }

    fn track_path(&self, id: TrackId) -> Result<Option<PathBuf>, LibraryError> {
        Ok(self
            .conn
            .prepare_cached("SELECT path FROM tracks WHERE id = ?1")?
            .query_row([id], |row| row.get::<_, String>(0))
            .optional()?
            .map(PathBuf::from))
    }

    /// Counts of roots, tracks, albums, artists and artwork, total duration and size.
    pub fn stats(&self) -> Result<LibraryStats, LibraryError> {
        Ok(self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM roots), (SELECT COUNT(*) FROM tracks),
                    (SELECT COUNT(*) FROM albums), (SELECT COUNT(*) FROM artists),
                    (SELECT COUNT(*) FROM artwork), (SELECT IFNULL(SUM(duration_ms), 0) FROM tracks),
                    (SELECT IFNULL(SUM(file_size), 0) FROM tracks)",
            [],
            |row| {
                let count = |index| row.get(index).map(query::non_negative);
                Ok(LibraryStats {
                    roots: count(0)?,
                    tracks: count(1)?,
                    albums: count(2)?,
                    artists: count(3)?,
                    artwork: count(4)?,
                    total_duration_ms: count(5)?,
                    total_bytes: count(6)?,
                })
            },
        )?)
    }

    pub(crate) fn artwork_dir(&self) -> PathBuf {
        self.cache_dir.join("artwork")
    }

    /// Deletes cached images and their thumbnails. Best effort: a stale cache
    /// file only wastes space.
    pub(crate) fn delete_artwork_files(&self, file_names: &[String]) {
        for name in file_names {
            let path = self.artwork_dir().join(name);
            if let Err(err) = std::fs::remove_file(&path)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(path = %path.display(), %err, "cannot delete cached artwork");
            }
            let Some(hash) = name.split('.').next().filter(|hash| thumbnail::is_hash(hash)) else {
                continue;
            };
            let thumbs = self.artwork_dir().join(hash);
            if let Err(err) = std::fs::remove_dir_all(&thumbs)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(path = %thumbs.display(), %err, "cannot delete artwork thumbnails");
            }
        }
    }
}

/// Roots are stored absolute, without `.` components or trailing separators.
fn root_key(path: &Path) -> Result<String, LibraryError> {
    util::absolute_components(path)?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| LibraryError::NonUtf8Path(path.to_path_buf()))
}

/// Tags, embedded lyrics and cover of one audio file, read directly. For
/// files played without being in the catalogue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub replaygain: ReplayGain,
    /// Embedded lyrics as LRC (or plain text when unsynced), as from
    /// [`Library::embedded_lyrics`].
    pub lyrics: Option<String>,
    /// The embedded front cover (or the best other picture), when asked for.
    #[serde(skip)]
    pub cover: Option<Vec<u8>>,
}

/// Reads the tags of `path` without touching any catalogue. The cover is
/// only decoded out of the file when `with_cover` is true.
pub fn read_file_tags(path: &Path, with_cover: bool) -> Result<FileTags, LibraryError> {
    let parsed = match tags::parse(&StdFileSystem, path, with_cover) {
        Ok(parsed) => parsed,
        Err(tags::ReadError::Io(err)) => return Err(err.into()),
        Err(tags::ReadError::Format(message)) => {
            return Err(LibraryError::Tags {
                path: path.to_path_buf(),
                message,
            });
        }
    };
    let meta = parsed.meta();
    Ok(FileTags {
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        album_artist: meta.album_artist,
        duration_ms: meta.duration_ms,
        replaygain: meta.replaygain,
        lyrics: parsed.lyrics(),
        cover: if with_cover {
            parsed.front_cover().map(|p| p.data().to_vec())
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_and_watcher_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Library>();
        assert_send::<LibraryWatcher>();
        assert_send::<LibraryError>();
    }

    #[test]
    fn root_keys_are_clean() {
        let key = root_key(Path::new("/music/./rock/")).unwrap();
        // On Windows the current drive comes first: D:\music\rock.
        let expected = std::path::absolute("/music/rock").unwrap();
        assert_eq!(PathBuf::from(&key), expected);
        assert!(!key.ends_with(['/', '\\']), "{key}");
    }

    #[cfg(windows)]
    #[test]
    fn root_keys_keep_the_drive() {
        let key = root_key(Path::new(r"C:\music\.\rock\")).unwrap();
        assert_eq!(key, r"C:\music\rock");
    }
}
