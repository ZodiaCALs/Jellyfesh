//! The exported API: the `JellyfeshCore` object, the `CoreListener` it calls, and the free
//! functions. The C ABI (`c_api`) wraps the same object.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, RwLockReadGuard};
use std::thread::{self, JoinHandle, ThreadId};

use jf_core as jf;

use crate::convert::Conv;
use crate::error::CoreFfiError;
use crate::secrets::{Foreign, SecretStore};
use crate::types::*;

/// Receives the core's events: implement it in Swift, Kotlin or C# and pass it to
/// `JellyfeshCore.set_listener`.
///
/// It is called on one core thread, in order. Hop to the UI thread to update views, and never
/// block in `on_event` waiting for a thread that is calling `set_listener` or `shutdown`: they
/// wait for a call in progress to return.
#[uniffi::export(foreign)]
pub trait CoreListener: Send + Sync {
    fn on_event(&self, event: Event);
}

/// The running core: commands in, events and snapshots out, library queries on the caller's
/// thread (`docs/core-api.md`). Shut it down with `shutdown` (dropping the last reference does
/// too); afterwards queries fail with `CoreFfiError::ShutDown`, `send` returns 0 and `snapshot`
/// returns the last state.
#[derive(uniffi::Object)]
pub struct JellyfeshCore {
    state: RwLock<State>,
    pump: Arc<EventPump>,
    pump_thread: Mutex<Option<JoinHandle<()>>>,
    /// Runs once the core has shut down (the C ABI's secret store stops calling out then).
    after_shutdown: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

enum State {
    Running(Box<jf::Core>),
    ShutDown(Box<Snapshot>),
}

impl std::fmt::Debug for JellyfeshCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let running = matches!(*read(&self.state), State::Running(_));
        f.debug_struct("JellyfeshCore")
            .field("running", &running)
            .finish_non_exhaustive()
    }
}

#[uniffi::export]
impl JellyfeshCore {
    /// Opens the library and session in `config.data_dir` and starts the core. Audio output
    /// opens on first playback.
    #[uniffi::constructor]
    pub fn new(config: CoreConfig) -> Result<Arc<Self>, CoreFfiError> {
        Self::start(config, None, None).map(Arc::new)
    }

    /// Like `new`, with the app's own store for the sign-ins of online sources (the Keychain,
    /// the Android Keystore, the Credential Manager). `new` uses the system keystore where the
    /// core reaches one itself (macOS, Windows, Linux) and keeps sign-ins in memory elsewhere.
    #[uniffi::constructor]
    pub fn with_secret_store(
        config: CoreConfig,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Arc<Self>, CoreFfiError> {
        Self::start(config, Some(secrets), None).map(Arc::new)
    }

    /// Queues a command and returns its id; a failure comes back as `Event::CommandFailed` with
    /// that id. Returns 0 after `shutdown`.
    pub fn send(&self, command: Command) -> u64 {
        match &*read(&self.state) {
            State::Running(core) => core.send(command.into()).0,
            State::ShutDown(_) => 0,
        }
    }

    /// The current state of every slice.
    pub fn snapshot(&self) -> Snapshot {
        match &*read(&self.state) {
            State::Running(core) => core.snapshot().into(),
            State::ShutDown(last) => (**last).clone(),
        }
    }

    /// Sets the listener that receives every event from now on, replacing any earlier one;
    /// `None` removes it. When this returns, the earlier listener is no longer called (unless
    /// this is called from inside its own `on_event`). Read `snapshot` after setting it for the
    /// state so far.
    pub fn set_listener(&self, listener: Option<Arc<dyn CoreListener>>) {
        self.pump.set(listener);
    }

    pub fn albums(&self, offset: u32, limit: u32) -> Result<Vec<Album>, CoreFfiError> {
        self.query(|core| core.albums(offset.conv(), limit.conv()))
    }

    /// A page of albums in the given order.
    pub fn albums_sorted(
        &self,
        sort: AlbumSort,
        descending: bool,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Album>, CoreFfiError> {
        self.query(|core| core.albums_sorted(sort.into(), descending, offset.conv(), limit.conv()))
    }

    pub fn artists(&self, offset: u32, limit: u32) -> Result<Vec<Artist>, CoreFfiError> {
        self.query(|core| core.artists(offset.conv(), limit.conv()))
    }

    pub fn tracks(&self, query: TrackQuery) -> Result<Vec<Track>, CoreFfiError> {
        self.query(|core| core.tracks(&query.into()))
    }

    /// One library track by id.
    pub fn track(&self, id: i64) -> Result<Option<Track>, CoreFfiError> {
        self.query(|core| core.track(id))
    }

    /// The tracks of an album, by disc and track number.
    pub fn album_tracks(&self, album_id: i64) -> Result<Vec<Track>, CoreFfiError> {
        self.query(|core| core.album_tracks(album_id))
    }

    /// The albums an artist appears on, by year then title.
    pub fn artist_albums(&self, artist_id: i64) -> Result<Vec<Album>, CoreFfiError> {
        self.query(|core| core.artist_albums(artist_id))
    }

    /// Searches the library only (fast, synchronous). `Command::Search` searches the sources too.
    pub fn search_library(&self, text: String, limit: u32) -> Result<SearchResults, CoreFfiError> {
        self.query(|core| core.search(&text, limit.conv()))
    }

    /// Library tracks to play for a search: an exactly named album or artist plays whole,
    /// anything else plays the matching tracks.
    pub fn tracks_for_query(&self, text: String, limit: u32) -> Result<Vec<TrackRef>, CoreFfiError> {
        self.query(|core| core.tracks_for_query(&text, limit.conv()))
    }

    /// The most recently added tracks.
    pub fn recently_added(&self, limit: u32) -> Result<Vec<Track>, CoreFfiError> {
        self.query(|core| core.recently_added(limit.conv()))
    }

    /// Recently played tracks (plays that counted), newest first.
    pub fn recently_played(&self, limit: u32) -> Result<Vec<TrackRow>, CoreFfiError> {
        self.query(|core| core.recently_played(limit.conv()))
    }

    /// The most played tracks, most first.
    pub fn most_played(&self, limit: u32) -> Result<Vec<TrackRow>, CoreFfiError> {
        self.query(|core| core.most_played(limit.conv()))
    }

    /// Every playlist, by name.
    pub fn playlists(&self) -> Result<Vec<PlaylistSummary>, CoreFfiError> {
        self.query(|core| core.playlists())
    }

    /// One playlist, or `None` when there is no such playlist.
    pub fn playlist(&self, id: i64) -> Result<Option<PlaylistSummary>, CoreFfiError> {
        self.query(|core| core.playlist(id))
    }

    /// Up to `limit` entries of a playlist in order, from position `offset`.
    pub fn playlist_entries(
        &self,
        id: i64,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<PlaylistEntry>, CoreFfiError> {
        self.query(|core| core.playlist_entries(id, offset.conv(), limit.conv()))
    }

    /// Writes a playlist to `path` as M3U8 (UTF-8, with `#EXTINF` lines), replacing the file
    /// atomically. Source entries are left out.
    pub fn export_playlist(&self, id: i64, path: String) -> Result<(), CoreFfiError> {
        self.query(|core| core.export_playlist(id, std::path::Path::new(&path)))
    }

    /// A page of favourites, the latest first.
    pub fn favorites(&self, offset: u32, limit: u32) -> Result<Vec<TrackRow>, CoreFfiError> {
        self.query(|core| core.favorites(offset.conv(), limit.conv()))
    }

    /// Whether a track is a favourite.
    pub fn is_favorite(&self, track: TrackRef) -> Result<bool, CoreFfiError> {
        self.query(|core| core.is_favorite(&track.into()))
    }

    /// A square-fitted JPEG of the artwork with this content hash at about `size_px` (64, 128,
    /// 256, 512 or 1024), created on first use: its path, or `None` for an unknown hash.
    pub fn artwork_thumbnail(&self, hash: String, size_px: u32) -> Result<Option<String>, CoreFfiError> {
        self.query(|core| core.artwork_thumbnail(&hash, size_px))
    }

    /// A page of the queue in playback order.
    pub fn queue_window(&self, offset: u32, limit: u32) -> Result<Vec<QueueItem>, CoreFfiError> {
        self.query(|core| Ok(core.queue_window(offset.conv(), limit.conv())))
    }

    /// Recently played queue entries, oldest first.
    pub fn history(&self) -> Result<Vec<HistoryEntry>, CoreFfiError> {
        self.query(|core| Ok(core.history()))
    }

    /// The library folders.
    pub fn roots(&self) -> Result<Vec<String>, CoreFfiError> {
        self.query(|core| core.roots())
    }

    pub fn stats(&self) -> Result<LibraryStats, CoreFfiError> {
        self.query(|core| core.stats())
    }

    /// Saves the session, stops playback and the listener, and releases the listener. Further
    /// calls do nothing.
    pub fn shutdown(&self) {
        let core = {
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            let State::Running(core) = &*state else {
                return;
            };
            let last = Box::new(Snapshot::from(core.snapshot()));
            match std::mem::replace(&mut *state, State::ShutDown(last)) {
                State::Running(core) => core,
                State::ShutDown(_) => unreachable!("checked above"),
            }
        };
        self.pump.set(None);
        core.shutdown();
        // The event channel closes with the core, so the pump thread ends; joining it means no
        // event is delivered after this returns. A listener can call this from `on_event`,
        // on the pump thread itself, which must not join itself.
        let pump_thread = lock(&self.pump_thread).take();
        if let Some(handle) = pump_thread
            && handle.thread().id() != thread::current().id()
        {
            let _ = handle.join();
        }
        if let Some(after) = lock(&self.after_shutdown).take() {
            after();
        }
    }
}

impl JellyfeshCore {
    /// Starts the core and the thread that forwards its events to the listener, with the app's
    /// secret store if it passed one; `after_shutdown` runs once the core has shut down.
    pub(crate) fn start(
        config: CoreConfig,
        secrets: Option<Arc<dyn SecretStore>>,
        after_shutdown: Option<Box<dyn FnOnce() + Send>>,
    ) -> Result<Self, CoreFfiError> {
        let mut config = jf::Config::from(config);
        if let Some(store) = secrets {
            config.sources.secrets = Some(Arc::new(Foreign(store)));
        }
        let core = jf::Core::start(config)?;
        let events = core.subscribe();
        let pump = Arc::new(EventPump::default());
        let handle = thread::Builder::new().name("jf-ffi-events".into()).spawn({
            let pump = pump.clone();
            move || {
                for event in events {
                    pump.deliver(event);
                }
            }
        })?;
        let _ = pump.thread.set(handle.thread().id());
        Ok(JellyfeshCore {
            state: RwLock::new(State::Running(Box::new(core))),
            pump,
            pump_thread: Mutex::new(Some(handle)),
            after_shutdown: Mutex::new(after_shutdown),
        })
    }

    /// Runs a query against the running core and converts its result.
    fn query<T: Conv<U>, U>(
        &self,
        run: impl FnOnce(&jf::Core) -> Result<T, jf::CoreError>,
    ) -> Result<U, CoreFfiError> {
        match &*read(&self.state) {
            State::Running(core) => Ok(run(core)?.conv()),
            State::ShutDown(_) => Err(CoreFfiError::ShutDown),
        }
    }
}

impl Drop for JellyfeshCore {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Forwards the core's events to the current listener, on the core's event thread.
#[derive(Default)]
pub(crate) struct EventPump {
    listener: Mutex<Option<Arc<dyn CoreListener>>>,
    /// Held while the listener runs, so replacing the listener can wait for a call in progress.
    delivering: Mutex<()>,
    /// The thread that delivers, which must never wait for itself.
    thread: OnceLock<ThreadId>,
}

impl EventPump {
    pub(crate) fn set(&self, listener: Option<Arc<dyn CoreListener>>) {
        let old = std::mem::replace(&mut *lock(&self.listener), listener);
        if self.thread.get() != Some(&thread::current().id()) {
            // A delivery in progress may still be using the old listener: wait for it.
            drop(lock(&self.delivering));
        }
        drop(old);
    }

    fn deliver(&self, event: jf::Event) {
        let _delivering = lock(&self.delivering);
        let Some(listener) = lock(&self.listener).clone() else {
            return;
        };
        let event = Event::from(event);
        // A panic must not end the pump: later events would never arrive.
        if catch_unwind(AssertUnwindSafe(|| listener.on_event(event))).is_err() {
            tracing::error!("the core listener panicked; one event was not delivered");
        }
    }
}

// ---- free functions -----------------------------------------------------------------------------

/// Audio output devices, for a device picker.
#[uniffi::export]
pub fn output_devices() -> Result<Vec<DeviceInfo>, CoreFfiError> {
    Ok(jf::output_devices()?.conv())
}

/// The equalizer presets, in the order every app lists them. Applying one sets `Settings.eq`.
#[uniffi::export]
pub fn eq_presets() -> Vec<EqPreset> {
    jf::eq_presets().conv()
}

/// Nanoseconds on the monotonic clock that playback anchors are stamped with. Read it for
/// `DisplayClock` (it is the same clock in the same process).
#[uniffi::export]
pub fn monotonic_now_ns() -> u64 {
    jf::monotonic_now_ns()
}

/// The platform's default folder for the library and session databases (`JF_DATA_DIR`
/// overrides it). Apps usually pass their own container's folder instead.
#[uniffi::export]
pub fn default_data_dir() -> Option<String> {
    jf::paths::default_data_dir().conv()
}

/// The platform's default cache folder (`JF_CACHE_DIR` overrides it).
#[uniffi::export]
pub fn default_cache_dir() -> Option<String> {
    jf::paths::default_cache_dir().conv()
}

/// The version of this library (`jf-ffi`, released with the core).
#[uniffi::export]
pub fn library_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}
