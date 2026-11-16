//! The core actor: one thread owns all state and applies commands in order.
//!
//! Helpers run on their own threads so the actor never blocks on slow I/O:
//! the library worker (the single SQLite writer: roots and scans), the
//! media worker (lyrics files and tags, artwork, backdrop bakes; for source
//! tracks, lyrics and artwork through their plugin) and the helpers of the
//! online sources ([`crate::sources`]: plugin calls, stream URLs, logins).
//! All of them park on a channel when idle, so an idle core uses no CPU and
//! no timers tick.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, after, never, select, unbounded};
use jf_engine::{
    Engine, EngineConfig, EngineEvent, EqSettings, HttpSource, MediaSource, OutputKind, PlayItem,
    PlaybackAnchor, ReplayGainInfo, ReplayGainMode, TrackInfo, monotonic_now_ns,
};
use jf_library::{Library, LibraryWatcher, ScanProgress, ScanReport};
use jf_plugin_host::{ImageRef, Item};
use jf_session::{Advance, PlaybackState, Queue, QueueItem, SessionStore, SourceTrack, TrackRef};

use crate::CoreError;
use crate::remote::{self, Op};
use crate::sources::{self, Answer, Bundled, Ctx, Done, SearchJob, Sources, SourcesMsg, Then};
use crate::types::{
    ArtworkRef, Collection, Command, CommandId, Config, DevicesSnapshot, Event, HueSource, LyricsSnapshot,
    LyricsSource, NowPlaying, PlaybackSnapshot, QueueSnapshot, RemoteAction, ScanStatus, SearchScope,
    SearchUpdate, Settings, Snapshot, SourcesSnapshot, Status, ThemeSnapshot, UP_NEXT_WINDOW,
};
use crate::{items, lyrics, theme};

/// Queue changes are written at most this often.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);
/// Previous restarts the current track after this much of it has played.
const RESTART_THRESHOLD_MS: i64 = 3_000;
/// Scan progress events are sent at most this often.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// While playing, the position is saved this often so a killed process
/// resumes close to where it was. No timer runs while paused or stopped.
const POSITION_SAVE_INTERVAL: Duration = Duration::from_secs(10);
/// A play stopped early counts once this much of the track has played (or
/// half of a shorter track).
const PLAY_COUNTS_AFTER_MS: u64 = 4 * 60 * 1_000;
/// Watched folders are scanned once they have been quiet this long.
const WATCH_DEBOUNCE: Duration = Duration::from_secs(2);
/// The settings key of the bundled plugins the person uninstalled.
const DECLINED_BUNDLED_KEY: &str = "uninstalled_bundled_plugins";
/// Longest the core waits at shutdown for installs, uninstalls and logouts in flight (a logout
/// may take 45 s when the service is slow to revoke its tokens).
const JOB_DRAIN: Duration = Duration::from_secs(60);

pub(crate) enum Msg {
    Command(CommandId, Command),
    Library(LibraryMsg),
    Media(MediaMsg),
    Sources(SourcesMsg),
    Shutdown,
}

pub(crate) enum LibraryJob {
    /// Adds a folder and scans it.
    AddRoot(CommandId, PathBuf),
    RemoveRoot(CommandId, PathBuf),
    /// Checks every folder.
    Scan(CommandId),
    /// Scans paths the folder watcher reported.
    ScanPaths(Vec<PathBuf>),
    /// Turns folder watching on or off.
    Watch(bool),
    Shutdown,
}

pub(crate) enum LibraryMsg {
    ScanStarted,
    Progress(ScanProgress),
    ScanFinished(Result<ScanReport, String>),
    Failed(CommandId, String),
    Changed(u64),
}

/// The play in progress, recorded in the play statistics when it ends.
struct PlayRecord {
    track: TrackRef,
    /// When it started, in milliseconds since the Unix epoch.
    started_ms: i64,
    duration_ms: Option<u64>,
}

/// Lyrics and theme to find for the track that started.
enum MediaJob {
    /// A file: sidecar or embedded lyrics, artwork from the library, the
    /// tags or the folder.
    Local {
        uid: u64,
        audio: PathBuf,
        library_id: Option<i64>,
        artwork: Option<PathBuf>,
    },
    /// A source track: lyrics and artwork through its plugin.
    Source {
        uid: u64,
        plugin: String,
        item: Box<Item>,
        /// The picture it was listed with, when the plugin has no `artwork`.
        listed: Option<ImageRef>,
    },
}

impl MediaJob {
    fn uid(&self) -> u64 {
        match self {
            MediaJob::Local { uid, .. } | MediaJob::Source { uid, .. } => *uid,
        }
    }
}

pub(crate) enum MediaMsg {
    Lyrics(Box<LyricsSnapshot>),
    Theme(Box<ThemeSnapshot>),
}

/// State shared with [`crate::Core`] handles on other threads.
pub(crate) struct Shared {
    pub(crate) snapshot: RwLock<Snapshot>,
    pub(crate) subscribers: Mutex<Vec<Sender<Event>>>,
    pub(crate) queue: RwLock<Queue>,
    /// The remote session whose updates the `Remote` slice takes (0: none).
    pub(crate) remote_session: AtomicU64,
}

pub(crate) struct Actor {
    config: Config,
    shared: Arc<Shared>,
    library: Library,
    store: Option<SessionStore>,
    engine: Option<Engine>,
    engine_events: Receiver<EngineEvent>,
    status: Status,
    anchor: PlaybackAnchor,
    volume: f32,
    eq: EqSettings,
    rg_mode: ReplayGainMode,
    preamp_db: f32,
    /// The entry the engine was asked to start and has not reported yet.
    /// Engine events from before that request are stale while it is set.
    awaiting_start: Option<u64>,
    /// The entry handed to the engine for gapless continuation.
    armed_next: Option<u64>,
    /// Restore point from the saved session, used by the first start.
    resume_at: Option<(u64, u64)>,
    /// Consecutive entries that failed to start; stops a queue of
    /// unplayable files from looping forever.
    failures: usize,
    media_uid: Option<u64>,
    playback_version: u64,
    queue_version: u64,
    save_due: Option<Instant>,
    position_save_due: Option<Instant>,
    library_jobs: Sender<LibraryJob>,
    media_jobs: Sender<MediaJob>,
    /// Preferences; the source of truth for eq, ReplayGain and the rest.
    settings: Settings,
    muted: bool,
    devices_version: u64,
    /// The newest library revision published.
    library_revision: u64,
    play: Option<PlayRecord>,
    /// The current track's theme as its artwork gives it, before the hue
    /// setting applies (see `publish_theme`).
    artwork_theme: Option<Arc<ThemeSnapshot>>,
    /// Online sources: the plugin host and its helpers.
    sources: Sources,
    /// The entry being started whose stream URL is still being resolved;
    /// the engine gets it once the URL is there.
    pending_start: Option<u64>,
    /// The entry the core announced as buffering while its stream URL was
    /// resolved; cleared when it starts (or something else plays).
    buffering_uid: Option<u64>,
    /// The remote session, while Jellyfesh controls a service's player.
    remote: Option<remote::Session>,
}

impl Shared {
    pub(crate) fn new(queue: Queue) -> Self {
        Shared {
            snapshot: RwLock::new(Snapshot {
                playback: PlaybackSnapshot {
                    version: 0,
                    status: Status::Stopped,
                    anchor: stopped_anchor(0),
                    volume: 1.0,
                    eq: EqSettings::flat(),
                    replaygain_mode: ReplayGainMode::Off,
                    preamp_db: 0.0,
                },
                queue: QueueSnapshot {
                    version: 0,
                    len: 0,
                    position: None,
                    current: None,
                    shuffle: false,
                    repeat: jf_session::RepeatMode::Off,
                    up_next: Vec::new(),
                },
                now_playing: None,
                lyrics: None,
                theme: None,
                library_revision: 0,
                scanning: false,
                settings: Arc::new(Settings::default()),
                devices: Arc::new(DevicesSnapshot {
                    version: 0,
                    devices: Vec::new(),
                    selected: None,
                }),
                sources: Arc::new(SourcesSnapshot {
                    version: 0,
                    plugins: Vec::new(),
                    presets: Vec::new(),
                    allow_community: false,
                    secret_storage: Default::default(),
                    unavailable: None,
                }),
                remote: Arc::new(remote::ended(0, None)),
                muted: false,
            }),
            subscribers: Mutex::new(Vec::new()),
            queue: RwLock::new(queue),
            remote_session: AtomicU64::new(0),
        }
    }

    pub(crate) fn read_queue(&self) -> RwLockReadGuard<'_, Queue> {
        self.queue.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_queue(&self) -> RwLockWriteGuard<'_, Queue> {
        self.queue.write().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        self.snapshot.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(crate) fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        f(&mut self.snapshot.write().unwrap_or_else(|e| e.into_inner()));
    }

    pub(crate) fn emit(&self, event: Event) {
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain(|s| s.send(event.clone()).is_ok());
    }

    /// Emits `event` if `keep` says so, deciding under the subscribers' lock
    /// so nothing emitted after a change of mind can overtake it.
    pub(crate) fn emit_if(&self, keep: impl FnOnce() -> bool, event: Event) {
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        if keep() {
            subs.retain(|s| s.send(event.clone()).is_ok());
        }
    }
}

fn stopped_anchor(generation: u64) -> PlaybackAnchor {
    PlaybackAnchor {
        track_generation: generation,
        position_us: 0,
        anchor_mono_ns: monotonic_now_ns(),
        rate: 0.0,
        duration_us: None,
    }
}

impl Actor {
    /// Opens the databases, restores the session and starts the helper
    /// threads. Returns the actor and the shared state for `Core`.
    pub(crate) fn new(config: Config, tx: Sender<Msg>) -> Result<(Actor, Arc<Shared>), CoreError> {
        let db = config.data_dir.join("library.sqlite");
        let library = Library::open(&db, &config.cache_dir)?;
        let store = if config.persist_session || config.restore_session {
            Some(SessionStore::open(&config.data_dir.join("session.sqlite"))?)
        } else {
            None
        };

        let mut queue = Queue::new();
        let mut resume_at = None;
        let mut volume = 1.0_f32;
        let mut eq = EqSettings::flat();
        let mut rg = (ReplayGainMode::Off, 0.0_f32);
        let mut settings: Option<Settings> = None;
        let mut declined_bundled = std::collections::BTreeSet::new();
        if config.restore_session
            && let Some(store) = &store
        {
            match store.load_queue() {
                Ok(Some(q)) => queue = q,
                Ok(None) => {}
                Err(e) => tracing::warn!("could not restore the queue: {e}"),
            }
            if let Ok(Some(pb)) = store.load_playback() {
                resume_at = pb.uid.map(|u| (u, pb.position_ms));
            }
            if let Ok(Some(v)) = store.setting::<f32>("volume") {
                volume = v.clamp(0.0, 1.0);
            }
            if let Ok(Some(e)) = store.setting::<EqSettings>("eq") {
                eq = e;
            }
            if let Ok(Some(r)) = store.setting::<(ReplayGainMode, f32)>("replaygain") {
                rg = r;
            }
            match store.setting::<Settings>("settings") {
                Ok(found) => settings = found,
                Err(e) => tracing::warn!("could not restore settings: {e}"),
            }
            match store.setting(DECLINED_BUNDLED_KEY) {
                Ok(found) => declined_bundled = found.unwrap_or_default(),
                Err(e) => tracing::warn!("could not restore the uninstalled bundled plugins: {e}"),
            }
        }
        // Settings from before v0.0.3 lived under separate keys.
        let settings = settings.unwrap_or_else(|| Settings {
            eq: eq.clone(),
            replaygain_mode: rg.0,
            preamp_db: rg.1,
            ..Settings::default()
        });
        let (eq, rg) = (
            settings.eq.clone(),
            (settings.replaygain_mode, settings.preamp_db),
        );

        let shared = Arc::new(Shared::new(queue));
        let library_jobs = spawn_library_worker(
            db.clone(),
            config.cache_dir.clone(),
            config.watch_folders && settings.watch_folders,
            tx.clone(),
        )?;
        let mut sources = Sources::new(&config, settings.allow_community_plugins, &shared, &tx);
        sources.install_bundled(config.bundled_plugins.clone(), declined_bundled);
        let media_jobs = spawn_media_worker(db, config.cache_dir.clone(), sources.ctx(), tx)?;

        let mut actor = Actor {
            config,
            shared: shared.clone(),
            library,
            store,
            engine: None,
            engine_events: never(),
            status: Status::Stopped,
            anchor: stopped_anchor(0),
            volume,
            eq,
            rg_mode: rg.0,
            preamp_db: rg.1,
            awaiting_start: None,
            armed_next: None,
            resume_at,
            failures: 0,
            media_uid: None,
            playback_version: 0,
            queue_version: 0,
            save_due: None,
            position_save_due: None,
            library_jobs,
            media_jobs,
            settings,
            muted: false,
            devices_version: 0,
            library_revision: 0,
            play: None,
            artwork_theme: None,
            sources,
            pending_start: None,
            buffering_uid: None,
            remote: None,
        };
        let revision = actor.library.revision().unwrap_or(0);
        actor.library_revision = revision;
        // A fixed hue colours the app before anything plays.
        let idle = actor.current_theme().map(Arc::new);
        shared.update(|s| s.theme = idle);
        let settings = Arc::new(actor.settings.clone());
        shared.update(|s| {
            s.library_revision = revision;
            s.settings = settings;
        });
        actor.publish_playback();
        actor.publish_queue();
        actor.publish_sources(true);
        Ok((actor, shared))
    }

    pub(crate) fn run(mut self, rx: Receiver<Msg>) {
        loop {
            let save_timer = match self.save_due {
                Some(due) => after(due.saturating_duration_since(Instant::now())),
                None => never(),
            };
            if self.status == Status::Playing && self.config.persist_session {
                self.position_save_due
                    .get_or_insert_with(|| Instant::now() + POSITION_SAVE_INTERVAL);
            } else {
                self.position_save_due = None;
            }
            let position_timer = match self.position_save_due {
                Some(due) => after(due.saturating_duration_since(Instant::now())),
                None => never(),
            };
            let engine_events = self.engine_events.clone();
            select! {
                recv(rx) -> msg => match msg {
                    Ok(Msg::Command(id, cmd)) => self.on_command(id, cmd),
                    Ok(Msg::Library(msg)) => self.on_library(msg),
                    Ok(Msg::Media(msg)) => self.on_media(msg),
                    Ok(Msg::Sources(msg)) => self.on_sources(msg),
                    Ok(Msg::Shutdown) | Err(_) => break,
                },
                recv(engine_events) -> ev => match ev {
                    Ok(ev) => self.on_engine(ev),
                    Err(_) => self.engine_events = never(),
                },
                recv(save_timer) -> _ => self.save_queue(),
                recv(position_timer) -> _ => {
                    self.position_save_due = None;
                    self.save_playback();
                }
            }
        }
        self.finish_source_jobs(&rx);
        self.shutdown();
    }

    /// Lets the installs, uninstalls and logouts in flight finish before the core stops, so a
    /// sign-out deletes the secrets even when the app quits right after it, and reports their
    /// results (a failure as `CommandFailed`, the `Sources` slice). Nothing else is applied
    /// any more. At most [`JOB_DRAIN`].
    fn finish_source_jobs(&mut self, rx: &Receiver<Msg>) {
        let deadline = Instant::now() + JOB_DRAIN;
        while self.sources.jobs_in_flight() > 0 {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                tracing::warn!("stopping with online source jobs still running");
                return;
            };
            match rx.recv_timeout(left) {
                Ok(Msg::Sources(msg @ SourcesMsg::Done { .. })) => self.on_sources(msg),
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    fn shutdown(&mut self) {
        // A play cut short by quitting is not recorded: the session resumes it.
        self.play = None;
        self.save_playback();
        self.save_queue();
        if let Some(engine) = self.engine.take() {
            engine.stop();
        }
    }

    // ---- commands -----------------------------------------------------

    fn on_command(&mut self, id: CommandId, cmd: Command) {
        if let Err(e) = self.apply(id, cmd) {
            self.shared.emit(Event::CommandFailed {
                id,
                error: e.to_string(),
            });
        }
    }

    fn apply(&mut self, id: CommandId, cmd: Command) -> Result<(), CoreError> {
        if self.remote.as_ref().is_some_and(|s| !s.is_alive()) {
            self.remote = None;
        }
        if let Some(op) = self.remote_op(&cmd) {
            let stop = op == Op::Stop;
            if let Some(session) = &self.remote {
                session.send(op);
            }
            if stop {
                // The worker pauses the player and ends; the slice is cleared now.
                remote::end(&self.shared, self.remote.take());
            }
            return Ok(());
        }
        match cmd {
            Command::PlayTracks { tracks, start } => {
                if tracks.is_empty() {
                    return Err(CoreError::Other("nothing to play".into()));
                }
                if let Some(plugin) = self.remote_plugin(&tracks, start) {
                    return self.play_remote_tracks(&plugin, &tracks, start, None);
                }
                self.refuse_remote_tracks(&tracks)?;
                self.shared.write_queue().set_items(tracks, Some(start))?;
                self.resume_at = None;
                self.failures = 0;
                self.queue_changed();
                self.start_current()?;
            }
            Command::Enqueue { tracks, play_next } => {
                self.refuse_remote_tracks(&tracks)?;
                {
                    let mut q = self.shared.write_queue();
                    if play_next {
                        q.insert_next(tracks);
                    } else {
                        q.append(tracks);
                    }
                }
                self.queue_changed();
                self.rearm();
            }
            Command::RemoveFromQueue { uid } => {
                let outcome = self.shared.write_queue().remove(uid)?;
                self.queue_changed();
                if outcome.was_current {
                    if self.is_active() && self.shared.read_queue().current().is_some() {
                        self.start_current()?;
                    } else {
                        self.stop_playback(Status::Stopped);
                    }
                } else {
                    self.rearm();
                }
            }
            Command::MoveInQueue { uid, to } => {
                self.shared.write_queue().move_item(uid, to)?;
                self.queue_changed();
                self.rearm();
            }
            Command::JumpTo { uid } => {
                self.shared.write_queue().jump_to(uid)?;
                self.resume_at = None;
                self.failures = 0;
                self.queue_changed();
                self.start_current()?;
            }
            Command::ClearQueue => {
                self.shared.write_queue().clear();
                self.queue_changed();
                self.stop_playback(Status::Stopped);
            }
            Command::SetShuffle { on } => {
                self.shared.write_queue().set_shuffle(on);
                self.queue_changed();
                self.rearm();
            }
            Command::SetRepeat { mode } => {
                self.shared.write_queue().set_repeat(mode);
                self.queue_changed();
                self.rearm();
            }
            Command::Play => self.play()?,
            Command::Pause => self.pause(),
            Command::TogglePlayPause => {
                if self.status == Status::Playing {
                    self.pause();
                } else {
                    self.play()?;
                }
            }
            Command::Stop => self.stop_playback(Status::Stopped),
            Command::Next => {
                let next = self.shared.write_queue().next(Advance::Manual);
                self.queue_changed();
                self.failures = 0;
                match next {
                    Some(_) => self.start_current()?,
                    None => self.end_of_queue(),
                }
            }
            Command::Previous => {
                if self.is_active() && self.position_us() > RESTART_THRESHOLD_MS * 1_000 {
                    self.seek(0);
                } else {
                    self.shared.write_queue().previous();
                    self.queue_changed();
                    self.failures = 0;
                    if self.shared.read_queue().current().is_some() {
                        self.start_current()?;
                    }
                }
            }
            Command::Seek { position_ms } => self.seek(position_ms),
            Command::SetVolume { volume } => {
                let volume = if volume.is_finite() {
                    volume.clamp(0.0, 1.0)
                } else {
                    1.0
                };
                self.volume = volume;
                if let Some(engine) = &self.engine {
                    engine.set_volume(volume);
                }
                self.save_setting("volume", &volume);
                self.publish_playback();
            }
            Command::SetEq { eq } => {
                let settings = Settings {
                    eq,
                    ..self.settings.clone()
                };
                self.apply_settings(settings);
            }
            Command::SetReplayGain { mode, preamp_db } => {
                let preamp_db = if preamp_db.is_finite() {
                    preamp_db.clamp(-15.0, 15.0)
                } else {
                    0.0
                };
                let settings = Settings {
                    replaygain_mode: mode,
                    preamp_db,
                    ..self.settings.clone()
                };
                self.apply_settings(settings);
            }
            Command::AddRoot { path } => {
                let path = std::path::absolute(&path).unwrap_or(path);
                self.library_job(LibraryJob::AddRoot(id, path))?;
            }
            Command::RemoveRoot { path } => {
                let path = std::path::absolute(&path).unwrap_or(path);
                self.library_job(LibraryJob::RemoveRoot(id, path))?;
            }
            Command::Rescan => self.library_job(LibraryJob::Scan(id))?,
            Command::PlayCollection {
                collection: Collection::Source { plugin, id: node },
                start,
                shuffle,
            } => {
                if self.sources.is_remote(&plugin) {
                    let context = self.sources.remote_node(&plugin, &node)?;
                    let op = Op::Play {
                        context: Some(Box::new(context)),
                        tracks: Vec::new(),
                        start: u32::try_from(start).unwrap_or(u32::MAX),
                        shuffle: Some(shuffle),
                    };
                    return self.start_remote(&plugin, op);
                }
                self.sources
                    .fetch_collection(id, plugin, node, Then::Play { start, shuffle })?;
            }
            Command::EnqueueCollection {
                collection: Collection::Source { plugin, id: node },
                play_next,
            } => {
                if self.sources.is_remote(&plugin) {
                    return Err(self.remote_queue_refusal(&plugin));
                }
                self.sources
                    .fetch_collection(id, plugin, node, Then::Enqueue { play_next })?;
            }
            Command::PlayCollection {
                collection,
                start,
                shuffle,
            } => {
                let (tracks, start) = self.collection_entries(&collection, start)?;
                if tracks.is_empty() {
                    return Err(CoreError::Other("nothing to play".into()));
                }
                let start = start.min(tracks.len() - 1);
                if let Some(plugin) = self.remote_plugin(&tracks, start) {
                    return self.play_remote_tracks(&plugin, &tracks, start, Some(shuffle));
                }
                self.refuse_remote_tracks(&tracks)?;
                {
                    let mut q = self.shared.write_queue();
                    q.set_shuffle(shuffle);
                    q.set_items(tracks, Some(start))?;
                }
                self.resume_at = None;
                self.failures = 0;
                self.queue_changed();
                self.start_current()?;
            }
            Command::EnqueueCollection {
                collection,
                play_next,
            } => {
                let tracks = self.collection_tracks(&collection)?;
                self.refuse_remote_tracks(&tracks)?;
                {
                    let mut q = self.shared.write_queue();
                    if play_next {
                        q.insert_next(tracks);
                    } else {
                        q.append(tracks);
                    }
                }
                self.queue_changed();
                self.rearm();
            }
            Command::SetSettings { settings } => self.apply_settings(settings),
            Command::SetMuted { muted } => {
                self.muted = muted;
                if let Some(engine) = &self.engine {
                    engine.set_volume(if muted { 0.0 } else { self.volume });
                }
                self.shared.update(|s| s.muted = muted);
                self.publish_playback();
            }
            Command::SetCrossfade { crossfade } => {
                let settings = Settings {
                    crossfade,
                    ..self.settings.clone()
                };
                self.apply_settings(settings);
            }
            Command::SetOutputDevice { name } => {
                let settings = Settings {
                    output_device: name,
                    ..self.settings.clone()
                };
                self.apply_settings(settings);
            }
            Command::RefreshDevices => self.refresh_devices()?,
            Command::Search { query, scope } => self.search(id, query, &scope)?,
            Command::CreatePlaylist { name, tracks } => {
                let playlist = self.library.create_playlist(&name)?;
                let added = if tracks.is_empty() {
                    Ok(Vec::new())
                } else {
                    let items: Vec<_> = tracks.iter().map(items::item_for).collect();
                    self.library.add_to_playlist(playlist, &items, None)
                };
                self.library_changed();
                self.shared.emit(Event::PlaylistCreated {
                    request: id,
                    id: playlist,
                });
                added?;
            }
            Command::RenamePlaylist { id: playlist, name } => {
                self.library.rename_playlist(playlist, &name)?;
                self.library_changed();
            }
            Command::DeletePlaylist { id: playlist } => {
                self.library.delete_playlist(playlist)?;
                self.library_changed();
            }
            Command::AddToPlaylist { id: playlist, tracks } => {
                let items: Vec<_> = tracks.iter().map(items::item_for).collect();
                self.library.add_to_playlist(playlist, &items, None)?;
                self.library_changed();
            }
            Command::RemoveFromPlaylist {
                id: playlist,
                entries,
            } => {
                self.library.remove_from_playlist(playlist, &entries)?;
                self.library_changed();
            }
            Command::MovePlaylistEntry {
                id: playlist,
                entry,
                to,
            } => {
                self.library.move_playlist_entry(playlist, entry, to)?;
                self.library_changed();
            }
            Command::ImportPlaylist { path } => {
                let name = items::file_title(&path);
                let playlist = self.library.import_m3u(&path, &name)?;
                self.library_changed();
                self.shared.emit(Event::PlaylistCreated {
                    request: id,
                    id: playlist,
                });
            }
            Command::SetFavorite { track, favorite } => {
                self.library.set_favorite(&items::item_for(&track), favorite)?;
                self.library_changed();
            }
            Command::InstallPlugin { path } => {
                let path = std::path::absolute(&path).unwrap_or(path);
                self.sources.install(id, path)?;
            }
            Command::UninstallPlugin { id: plugin } => self.sources.uninstall(id, plugin)?,
            Command::SetPluginEnabled { id: plugin, enabled } => {
                let result = self.sources.set_enabled(&plugin, enabled);
                self.publish_sources(true);
                result?;
            }
            Command::SetPluginSettings {
                id: plugin,
                settings_json,
            } => {
                self.sources.set_settings(&plugin, &settings_json)?;
                self.publish_sources(true);
            }
            Command::AcknowledgeUnofficial { id: plugin } => {
                self.sources.acknowledge(&plugin)?;
                if !self.settings.allow_community_plugins {
                    // Accepting the disclosure turns community plugins on.
                    let settings = Settings {
                        allow_community_plugins: true,
                        ..self.settings.clone()
                    };
                    self.apply_settings(settings);
                }
                let result = self.sources.enable_acknowledged(&plugin);
                self.publish_sources(true);
                result?;
            }
            Command::BeginLogin { plugin, method } => self.sources.begin_login(&plugin, method)?,
            Command::ConnectSource {
                preset,
                address,
                method,
            } => {
                let result = self.sources.connect(&preset, address.as_deref(), method);
                self.publish_sources(true);
                result?;
            }
            Command::SubmitLogin { plugin, flow, fields } => {
                self.sources.submit_login(&plugin, flow, fields)?
            }
            Command::CancelLogin { plugin, flow } => self.sources.cancel_login(&plugin, flow)?,
            Command::Logout { plugin } => self.sources.logout(id, plugin)?,
            Command::Browse { plugin, node, page } => self.sources.browse(id, plugin, node, page)?,
            Command::Remote { action } => self.remote_action(action)?,
        }
        Ok(())
    }

    // ---- remote sessions (ADR-0018) -----------------------------------------

    /// While a remote session runs, the transport commands it takes.
    fn remote_op(&self, cmd: &Command) -> Option<Op> {
        self.remote.as_ref().filter(|s| s.is_alive())?;
        Some(match cmd {
            Command::Play => Op::Resume,
            Command::Pause => Op::Pause,
            Command::TogglePlayPause => Op::Toggle,
            Command::Stop => Op::Stop,
            Command::Next => Op::Next,
            Command::Previous => Op::Previous,
            Command::Seek { position_ms } => Op::Seek(*position_ms),
            Command::SetShuffle { on } => Op::Shuffle(*on),
            Command::SetRepeat { mode } => Op::Repeat(*mode),
            _ => return None,
        })
    }

    fn remote_action(&mut self, action: RemoteAction) -> Result<(), CoreError> {
        if matches!(action, RemoteAction::End) {
            remote::end(&self.shared, self.remote.take());
            return Ok(());
        }
        let Some(session) = self.remote.as_ref().filter(|s| s.is_alive()) else {
            return Err(CoreError::Other("nothing is playing on another device".into()));
        };
        session.send(match action {
            RemoteAction::Refresh => Op::Refresh,
            RemoteAction::Transfer { device } => Op::Transfer(device),
            RemoteAction::SetVolume { percent } => Op::Volume(percent.min(100)),
            RemoteAction::End => unreachable!("handled above"),
        });
        Ok(())
    }

    /// The remote source whose track `start` is, if it is one.
    fn remote_plugin(&self, tracks: &[TrackRef], start: usize) -> Option<String> {
        match tracks.get(start)? {
            TrackRef::Source(track) if self.sources.is_remote(&track.plugin) => Some(track.plugin.clone()),
            _ => None,
        }
    }

    /// Remote sources' tracks never join the queue (`mixed_queue` is theirs
    /// to decide, and the engine cannot play them anyway).
    fn refuse_remote_tracks(&self, tracks: &[TrackRef]) -> Result<(), CoreError> {
        match tracks.iter().find_map(|t| match t {
            TrackRef::Source(track) if self.sources.is_remote(&track.plugin) => Some(track.plugin.clone()),
            _ => None,
        }) {
            Some(plugin) => Err(self.remote_queue_refusal(&plugin)),
            None => Ok(()),
        }
    }

    fn remote_queue_refusal(&self, plugin: &str) -> CoreError {
        CoreError::Other(format!(
            "{} plays on your own devices; its songs cannot join the queue",
            self.sources.display_name(plugin)
        ))
    }

    /// Plays the tracks of one remote source (others in the list are left
    /// out), from `start`.
    fn play_remote_tracks(
        &mut self,
        plugin: &str,
        tracks: &[TrackRef],
        start: usize,
        shuffle: Option<bool>,
    ) -> Result<(), CoreError> {
        let mut items = Vec::new();
        let mut first = 0;
        for (i, track) in tracks.iter().enumerate() {
            if let TrackRef::Source(track) = track
                && track.plugin == plugin
            {
                if i == start {
                    first = items.len();
                }
                items.push(self.sources.remote_track(track)?);
            }
        }
        let op = Op::Play {
            context: None,
            tracks: items,
            start: u32::try_from(first).unwrap_or(0),
            shuffle,
        };
        self.start_remote(plugin, op)
    }

    /// Pauses local playback and starts a session with `first`.
    fn start_remote(&mut self, plugin: &str, first: Op) -> Result<(), CoreError> {
        let (backend, source) = self.sources.remote_backend(plugin)?;
        if self.status == Status::Playing {
            self.pause();
        }
        // The new session takes the slice over first, so the old one ends quietly.
        let session = remote::Session::start(Box::new(backend), source, self.shared.clone(), first)?;
        self.remote = Some(session);
        Ok(())
    }

    /// Local playback is starting: a remote session ends (the service's
    /// player carries on as it was).
    fn end_remote(&mut self) {
        if self.remote.is_some() {
            remote::end(&self.shared, self.remote.take());
        }
    }

    /// The tracks of a collection, in its natural order.
    fn collection_tracks(&self, collection: &Collection) -> Result<Vec<TrackRef>, CoreError> {
        let library = |q: jf_library::TrackQuery| -> Result<Vec<TrackRef>, CoreError> {
            Ok(self
                .library
                .tracks(&q)?
                .into_iter()
                .map(|t| TrackRef::Library(t.id))
                .collect())
        };
        match collection {
            Collection::Album { id } => library(jf_library::TrackQuery {
                album_id: Some(*id),
                sort: jf_library::TrackSort::Album,
                limit: usize::MAX,
                ..jf_library::TrackQuery::default()
            }),
            Collection::Artist { id } => library(jf_library::TrackQuery {
                artist_id: Some(*id),
                sort: jf_library::TrackSort::Artist,
                limit: usize::MAX,
                ..jf_library::TrackQuery::default()
            }),
            Collection::Tracks { tracks } => Ok(tracks.clone()),
            Collection::Playlist { .. } | Collection::Favorites => {
                Ok(self.collection_entries(collection, 0)?.0)
            }
            // Fetched through the plugin, off the actor (see `apply`).
            Collection::Source { .. } => Err(CoreError::Other(
                "source collections are fetched through their plugin".into(),
            )),
        }
    }

    /// The playable entries of a playlist or the favourites, and the queue
    /// index of the list's entry `start`.
    fn collection_entries(
        &self,
        collection: &Collection,
        start: usize,
    ) -> Result<(Vec<TrackRef>, usize), CoreError> {
        match collection {
            Collection::Playlist { id } => {
                if self.library.playlist(*id)?.is_none() {
                    return Err(CoreError::Other(format!("there is no playlist {id}")));
                }
                let entries = self.library.playlist_entries(*id, 0, usize::MAX)?;
                Ok(items::queue_entries(
                    entries.iter().map(|e| (&e.item, e.track.as_ref())),
                    start,
                ))
            }
            Collection::Favorites => {
                let favorites = self.library.favorites(0, usize::MAX)?;
                Ok(items::queue_entries(
                    favorites.iter().map(|f| (&f.item, f.track.as_ref())),
                    start,
                ))
            }
            _ => Ok((self.collection_tracks(collection)?, start)),
        }
    }

    /// Publishes the library's revision after a change made here.
    fn library_changed(&mut self) {
        match self.library.revision() {
            Ok(revision) => self.set_library_revision(revision),
            Err(e) => tracing::warn!("could not read the library revision: {e}"),
        }
    }

    /// Publishes a library revision unless a newer one already went out (the
    /// library worker and the actor both write).
    fn set_library_revision(&mut self, revision: u64) {
        if revision <= self.library_revision {
            return;
        }
        self.library_revision = revision;
        self.shared.update(|s| s.library_revision = revision);
        self.shared.emit(Event::LibraryChanged { revision });
    }

    /// Applies new settings: the parts the core owns take effect at once;
    /// all of them are saved and published.
    fn apply_settings(&mut self, settings: Settings) {
        let mut settings = settings;
        settings.preamp_db = if settings.preamp_db.is_finite() {
            settings.preamp_db.clamp(-15.0, 15.0)
        } else {
            0.0
        };
        settings.crossfade.duration_ms = settings.crossfade.duration_ms.clamp(1_000, 12_000);
        settings.transparency = if settings.transparency.is_finite() {
            settings.transparency.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if let Some(engine) = &self.engine {
            if settings.eq != self.settings.eq {
                engine.set_eq(settings.eq.clone());
            }
            if (settings.replaygain_mode, settings.preamp_db)
                != (self.settings.replaygain_mode, self.settings.preamp_db)
            {
                engine.set_replaygain(settings.replaygain_mode, settings.preamp_db);
            }
            if settings.crossfade != self.settings.crossfade {
                engine.set_crossfade(settings.crossfade);
            }
        }
        self.eq = settings.eq.clone();
        self.rg_mode = settings.replaygain_mode;
        self.preamp_db = settings.preamp_db;
        let devices_changed = settings.output_device != self.settings.output_device;
        let hue_changed = settings.hue_source != self.settings.hue_source;
        let community_changed = settings.allow_community_plugins != self.settings.allow_community_plugins;
        let switch_output = devices_changed
            && matches!(self.config.output, OutputKind::Device { name: None })
            && self.engine.is_some();
        if settings.watch_folders != self.settings.watch_folders && self.config.watch_folders {
            let _ = self.library_job(LibraryJob::Watch(settings.watch_folders));
        }
        self.settings = settings;
        if switch_output && let Some(engine) = &self.engine {
            // Playback moves to the new device where it is.
            engine.set_output(self.output());
        }
        self.save_setting("settings", &self.settings);
        let snapshot = Arc::new(self.settings.clone());
        self.shared.update(|s| s.settings = snapshot.clone());
        self.shared.emit(Event::Settings(snapshot));
        if hue_changed {
            self.publish_theme(true);
        }
        if community_changed {
            self.sources
                .set_allow_community(self.settings.allow_community_plugins);
            self.publish_sources(true);
        }
        if devices_changed {
            let selected = self.settings.output_device.clone();
            self.devices_version += 1;
            let version = self.devices_version;
            let devices = {
                let current = self.shared.snapshot();
                Arc::new(DevicesSnapshot {
                    version,
                    devices: current.devices.devices.clone(),
                    selected,
                })
            };
            self.shared.update(|s| s.devices = devices.clone());
            self.shared.emit(Event::Devices(devices));
        }
        self.publish_playback();
    }

    /// Lists output devices and publishes them.
    fn refresh_devices(&mut self) -> Result<(), CoreError> {
        if let Some(engine) = &self.engine {
            // Following the system default: playback moves when it changed.
            engine.devices_changed();
        }
        let devices = jf_engine::list_devices().unwrap_or_default();
        self.devices_version += 1;
        let snapshot = Arc::new(DevicesSnapshot {
            version: self.devices_version,
            devices,
            selected: self.settings.output_device.clone(),
        });
        self.shared.update(|s| s.devices = snapshot.clone());
        self.shared.emit(Event::Devices(snapshot));
        Ok(())
    }

    /// Searches the library at once; the sources answer on the search
    /// thread, each in its own section merged with the ones before.
    fn search(&mut self, id: CommandId, query: String, scope: &SearchScope) -> Result<(), CoreError> {
        let plugins = match scope {
            SearchScope::Library => Vec::new(),
            SearchScope::All if query.trim().is_empty() => Vec::new(),
            SearchScope::All => self.sources.unified_search_plugins(),
            SearchScope::Plugin { id: plugin } => {
                self.sources.check_search(plugin)?;
                vec![plugin.clone()]
            }
        };
        self.sources.begin_search(id);
        let library = match scope {
            SearchScope::Plugin { .. } => None,
            SearchScope::All | SearchScope::Library => {
                let results = self.library.search(&query, 50)?;
                let (items, rows) = sources::library_answer(&results);
                Some(Answer {
                    source: sources::LIBRARY.into(),
                    items,
                    rows,
                })
            }
        };
        if let Some(answer) = &library {
            let update = SearchUpdate {
                request: id,
                query: query.clone(),
                source: sources::LIBRARY.into(),
                items: answer.rows.clone(),
                error: None,
                done: plugins.is_empty(),
            };
            self.sources.emit_search(&self.shared, update);
        }
        if !plugins.is_empty() {
            self.sources.start_search(SearchJob {
                request: id,
                query,
                library,
                plugins,
            })?;
        }
        Ok(())
    }

    fn library_job(&self, job: LibraryJob) -> Result<(), CoreError> {
        self.library_jobs
            .send(job)
            .map_err(|_| CoreError::Other("the library worker has stopped".into()))
    }

    fn is_active(&self) -> bool {
        matches!(self.status, Status::Playing | Status::Paused)
    }

    fn play(&mut self) -> Result<(), CoreError> {
        match self.status {
            Status::Playing => {}
            // Paused before its stream URL arrived: start it now.
            Status::Paused if self.pending_start.is_some() => {
                self.status = Status::Playing;
                self.start_current()?;
            }
            Status::Paused => {
                if let Some(engine) = &self.engine {
                    engine.play();
                }
                self.status = Status::Playing;
                self.publish_playback();
            }
            Status::Stopped | Status::Ended => {
                if self.shared.read_queue().current().is_none() {
                    if self.shared.write_queue().next(Advance::Manual).is_none() {
                        return Err(CoreError::Other("the queue is empty".into()));
                    }
                    self.queue_changed();
                }
                self.failures = 0;
                self.start_current()?;
            }
        }
        Ok(())
    }

    fn pause(&mut self) {
        if self.status == Status::Playing {
            if let Some(engine) = &self.engine {
                engine.pause();
            }
            self.status = Status::Paused;
            self.publish_playback();
            self.save_playback();
        }
    }

    fn seek(&mut self, position_ms: u64) {
        if let Some(uid) = self.pending_start {
            // Not started yet: start there.
            self.resume_at = Some((uid, position_ms));
            self.anchor.position_us = i64::try_from(position_ms).unwrap_or(i64::MAX / 1_000) * 1_000;
            self.publish_playback();
            return;
        }
        match &self.engine {
            Some(engine) if self.is_active() => engine.seek(position_ms),
            _ => {
                let current = self.shared.read_queue().current_uid();
                self.resume_at = current.map(|u| (u, position_ms));
            }
        }
    }

    fn position_us(&self) -> i64 {
        self.anchor.position_at(monotonic_now_ns())
    }

    // ---- playback -----------------------------------------------------

    /// Starts the current queue entry, skipping entries that cannot be
    /// resolved to a playable file. A source track whose stream URL is not
    /// known yet waits for it (`pending_start`); [`Actor::on_sources`]
    /// starts it once its plugin has answered.
    fn start_current(&mut self) -> Result<(), CoreError> {
        self.end_remote();
        self.finish_play(false);
        self.pending_start = None;
        let len = self.shared.read_queue().len();
        for _ in 0..len.max(1) {
            let Some(item) = self.shared.read_queue().current() else {
                break;
            };
            match self.resolve(&item) {
                Ok(Resolution::Ready(play)) => {
                    if self.buffering_uid != Some(item.uid) {
                        self.set_buffering(None);
                    }
                    let next = self.lookahead();
                    self.ensure_engine()?;
                    let resume = self
                        .resume_at
                        .take()
                        .filter(|(uid, ms)| *uid == item.uid && *ms > 0);
                    if let Some(engine) = &self.engine {
                        engine.set_queue(vec![play], 0);
                        engine.set_upcoming_after(item.uid, next.iter().cloned().collect());
                        if let Some((_, ms)) = resume {
                            engine.seek(ms);
                        }
                        engine.play();
                    }
                    self.armed_next = next.map(|p| p.id);
                    self.awaiting_start = Some(item.uid);
                    self.status = Status::Playing;
                    // Hold the progress at the start point until the engine
                    // reports the new track with a fresh anchor.
                    let at_us = resume.map_or(0, |(_, ms)| i64::try_from(ms).unwrap_or(0) * 1_000);
                    self.anchor = PlaybackAnchor {
                        position_us: at_us,
                        ..stopped_anchor(self.anchor.track_generation + 1)
                    };
                    self.publish_playback();
                    return Ok(());
                }
                Ok(Resolution::Pending) => {
                    self.wait_for_stream(&item);
                    return Ok(());
                }
                Err(e) => {
                    self.shared.emit(Event::Error {
                        message: format!("skipping {}: {e}", describe(&item.track)),
                    });
                    self.failures += 1;
                    if self.failures > len {
                        break;
                    }
                    let next = self.shared.write_queue().next(Advance::Manual);
                    self.queue_changed();
                    if next.is_none() {
                        break;
                    }
                }
            }
        }
        self.end_of_queue();
        Ok(())
    }

    /// The current entry is a source track whose stream URL is on its way:
    /// silence what played, show it as starting (buffering), and resolve the
    /// entry after it meanwhile.
    fn wait_for_stream(&mut self, item: &QueueItem) {
        if let Some(engine) = &self.engine {
            engine.stop();
        }
        self.pending_start = Some(item.uid);
        // Engine events from before are stale from here on.
        self.awaiting_start = Some(item.uid);
        self.armed_next = None;
        self.status = Status::Playing;
        let at_us = match self.resume_at {
            Some((uid, ms)) if uid == item.uid => i64::try_from(ms).unwrap_or(0) * 1_000,
            _ => 0,
        };
        self.anchor = PlaybackAnchor {
            position_us: at_us,
            ..stopped_anchor(self.anchor.track_generation + 1)
        };
        self.publish_playback();
        self.set_buffering(Some(item.uid));
        let _ = self.lookahead();
    }

    /// Announces which entry is waiting for its stream (`None`: none).
    fn set_buffering(&mut self, uid: Option<u64>) {
        if self.buffering_uid == uid {
            return;
        }
        if let Some(old) = self.buffering_uid.take() {
            self.shared.emit(Event::Buffering {
                uid: old,
                buffering: false,
            });
        }
        if let Some(uid) = uid {
            self.shared.emit(Event::Buffering { uid, buffering: true });
            self.buffering_uid = Some(uid);
        }
    }

    /// Records the play in progress, if any: as counted when it played to its
    /// end (`natural`) or far enough, as skipped otherwise.
    fn finish_play(&mut self, natural: bool) {
        let Some(play) = self.play.take() else {
            return;
        };
        let counted = natural || {
            let played_ms = u64::try_from(self.position_us() / 1_000).unwrap_or(0);
            let needed = play
                .duration_ms
                .map_or(PLAY_COUNTS_AFTER_MS, |d| (d / 2).min(PLAY_COUNTS_AFTER_MS));
            played_ms >= needed
        };
        let item = items::item_for(&play.track);
        match self.library.record_play(&item, play.started_ms, counted) {
            Ok(()) => self.library_changed(),
            Err(e) => tracing::warn!("could not record a play of {}: {e}", describe(&play.track)),
        }
    }

    fn ensure_engine(&mut self) -> Result<(), CoreError> {
        if self.engine.is_none() {
            let config = |output| EngineConfig {
                output,
                buffer_ms: self.config.buffer_ms,
                // HTTP streams keep what they downloaded here while they play.
                cache_dir: Some(self.config.cache_dir.join("streams")),
                // Source streams refresh their URLs through their plugin.
                resolver: self.sources.refresher(),
            };
            let engine = match Engine::new(config(self.output())) {
                Ok(engine) => engine,
                // The device chosen in the settings is gone (unplugged):
                // play on the system default rather than not at all.
                Err(e) if self.output() != self.config.output => {
                    self.shared.emit(Event::Error {
                        message: format!(
                            "{} is not available ({e}); playing on the system default device",
                            self.settings.output_device.as_deref().unwrap_or_default()
                        ),
                    });
                    Engine::new(config(self.config.output.clone()))?
                }
                Err(e) => return Err(e.into()),
            };
            engine.set_volume(if self.muted { 0.0 } else { self.volume });
            engine.set_crossfade(self.settings.crossfade);
            engine.set_eq(self.eq.clone());
            engine.set_replaygain(self.rg_mode, self.preamp_db);
            self.engine_events = engine.events();
            self.engine = Some(engine);
        }
        Ok(())
    }

    /// Where audio goes: the configured output, where the system default
    /// device gives way to the device chosen in the settings.
    fn output(&self) -> OutputKind {
        match &self.config.output {
            OutputKind::Device { name: None } => OutputKind::Device {
                name: self.settings.output_device.clone(),
            },
            other => other.clone(),
        }
    }

    /// The entry that plays next on its own, resolved for the engine. A
    /// source track is resolved ahead (so gapless joins and crossfades work
    /// across source tracks) and armed once its URL is there.
    fn lookahead(&mut self) -> Option<PlayItem> {
        let item = self.shared.read_queue().peek_next(Advance::Auto)?;
        match self.resolve(&item) {
            Ok(Resolution::Ready(play)) => Some(play),
            Ok(Resolution::Pending) | Err(_) => None,
        }
    }

    /// Hands the engine the entry that now follows the current one, when
    /// that changed.
    fn rearm(&mut self) {
        // While a source track waits for its stream, its start arms the next.
        if !self.is_active() || self.pending_start.is_some() {
            return;
        }
        let next = self.lookahead();
        let id = next.as_ref().map(|p| p.id);
        if id != self.armed_next {
            // Relative to the entry the queue is at: the engine may already be past it
            // (short tracks), and must not take the list as following a later one.
            let current = self.shared.read_queue().current_uid();
            if let Some(engine) = &self.engine {
                match current {
                    Some(uid) => engine.set_upcoming_after(uid, next.into_iter().collect()),
                    None => engine.set_upcoming(next.into_iter().collect()),
                }
            }
            self.armed_next = id;
        }
    }

    fn stop_playback(&mut self, status: Status) {
        self.finish_play(false);
        if let Some(engine) = &self.engine {
            engine.stop();
        }
        self.status = status;
        self.awaiting_start = None;
        self.pending_start = None;
        self.set_buffering(None);
        self.armed_next = None;
        self.anchor = stopped_anchor(self.anchor.track_generation);
        self.publish_playback();
        self.set_now_playing(None);
        self.save_playback();
    }

    /// The queue played out: stop, and rewind to the first entry so Play
    /// starts it again.
    fn end_of_queue(&mut self) {
        self.stop_playback(Status::Ended);
        let first = self.shared.read_queue().item_at(0).map(|i| i.uid);
        if let Some(uid) = first {
            let _ = self.shared.write_queue().jump_to(uid);
            self.queue_changed();
        }
    }

    fn on_engine(&mut self, ev: EngineEvent) {
        match ev {
            EngineEvent::TrackStarted { id, info, .. } => self.on_track_started(id, info),
            EngineEvent::Anchor(anchor) => {
                if self.awaiting_start.is_none() {
                    self.anchor = anchor;
                    self.publish_playback();
                }
            }
            EngineEvent::TrackEnded { .. } => {}
            EngineEvent::Buffering { id, buffering } => {
                let current = self.shared.read_queue().current_uid();
                if current == Some(id) || self.awaiting_start == Some(id) {
                    self.shared.emit(Event::Buffering { uid: id, buffering });
                }
            }
            EngineEvent::OutputFailed { message } => {
                // The output died and could not be reopened. The engine keeps
                // the position and reopens the output on the next play: show
                // that as paused, with the reason.
                self.shared.emit(Event::Error { message });
                if self.status == Status::Playing {
                    self.status = Status::Paused;
                    self.publish_playback();
                    self.save_playback();
                }
            }
            EngineEvent::QueueEnded => {
                if self.awaiting_start.is_some() || !self.is_active() {
                    return; // from before the latest start, or already stopped
                }
                self.finish_play(true);
                // Nothing was armed (the next entry could not be resolved, or
                // the queue really ends): move on in the session queue.
                let next = self.shared.write_queue().next(Advance::Auto);
                self.queue_changed();
                match next {
                    Some(_) => {
                        if let Err(e) = self.start_current() {
                            self.shared.emit(Event::Error {
                                message: e.to_string(),
                            });
                            self.stop_playback(Status::Stopped);
                        }
                    }
                    None => self.end_of_queue(),
                }
            }
            EngineEvent::Error { id, message } => {
                self.shared.emit(Event::Error { message });
                // Without an id: an output problem that did not stop playback
                // (a device switch that failed keeps the old device).
                let Some(id) = id else {
                    return;
                };
                if self.awaiting_start == Some(id) {
                    // The entry just started could not be opened. The engine
                    // moves on to the armed next entry by itself (reported as
                    // a gapless start), or reports that its queue ended.
                    self.awaiting_start = None;
                } else if self.awaiting_start.is_none() && self.armed_next == Some(id) {
                    // The next entry failed at a join: step onto it, so the
                    // queue moves past it when the engine reports the end.
                    self.shared.write_queue().next(Advance::Auto);
                    self.armed_next = None;
                    self.queue_changed();
                } else {
                    return; // from before the latest start
                }
                self.failures += 1;
                if self.failures > self.shared.read_queue().len() {
                    self.stop_playback(Status::Stopped);
                }
            }
        }
    }

    fn on_track_started(&mut self, id: u64, info: TrackInfo) {
        if self.buffering_uid == Some(id) {
            self.set_buffering(None);
        }
        let current = self.shared.read_queue().current_uid();
        if self.awaiting_start == Some(id) && current == Some(id) {
            self.awaiting_start = None;
        } else if self.awaiting_start.is_none() && current == Some(id) {
            // Repeat one: the same entry started again.
            self.finish_play(true);
        } else if self.awaiting_start.is_none() && self.armed_next == Some(id) {
            self.finish_play(true);
            let advanced = self.shared.write_queue().next(Advance::Auto).map(|i| i.uid);
            if advanced != Some(id) {
                let _ = self.shared.write_queue().jump_to(id);
            }
        } else {
            return; // stale: from before the latest start
        }
        self.failures = 0;
        self.status = Status::Playing;

        let Some(item) = self.shared.read_queue().current() else {
            return;
        };
        // The history is part of the queue state and is saved with it.
        self.shared
            .write_queue()
            .record_played(item.track.clone(), unix_ms());
        self.queue_changed();

        let mut now = match self.now_playing(&item) {
            Ok(meta) => meta,
            Err(e) => {
                self.shared.emit(Event::Error {
                    message: e.to_string(),
                });
                return;
            }
        };
        now.format = Some(info);
        self.play = Some(PlayRecord {
            track: item.track.clone(),
            started_ms: unix_ms(),
            duration_ms: now.duration_ms,
        });
        let changed = self.media_uid != Some(item.uid);
        let job = match &item.track {
            TrackRef::Source(track) => self.sources.ctx().map(|ctx| {
                let item_record = ctx.track_item(track);
                let listed = item_record.artwork.clone();
                MediaJob::Source {
                    uid: item.uid,
                    plugin: track.plugin.clone(),
                    item: Box::new(item_record),
                    listed,
                }
            }),
            TrackRef::Library(_) | TrackRef::File(_) => now.path.clone().map(|audio| MediaJob::Local {
                uid: item.uid,
                audio,
                library_id: match item.track {
                    TrackRef::Library(id) => Some(id),
                    TrackRef::File(_) | TrackRef::Source(_) => None,
                },
                artwork: match &now.artwork {
                    Some(ArtworkRef::File(path)) => Some(path.clone()),
                    _ => None,
                },
            }),
        };
        self.set_now_playing(Some(now));
        if changed && let Some(job) = job {
            self.media_uid = Some(item.uid);
            let _ = self.media_jobs.send(job);
        }
        self.armed_next = None;
        self.rearm();
        self.publish_playback();
        self.save_playback();
    }

    /// Resolves a queue entry to the engine's play item and display data. A
    /// source track is `Pending` until its plugin has given its stream URL
    /// (asking for it the first time).
    fn resolve(&mut self, item: &QueueItem) -> Result<Resolution, CoreError> {
        match &item.track {
            TrackRef::Source(track) => match self.sources.stream(item.uid, track)? {
                Some(source) => Ok(Resolution::Ready(source_play_item(item.uid, track, source))),
                None => Ok(Resolution::Pending),
            },
            TrackRef::Library(_) | TrackRef::File(_) => Ok(Resolution::Ready(self.resolve_local(item)?.0)),
        }
    }

    /// What to show for a queue entry.
    fn now_playing(&self, item: &QueueItem) -> Result<NowPlaying, CoreError> {
        match &item.track {
            TrackRef::Source(track) => {
                let link = self.sources.ctx().and_then(|ctx| {
                    ctx.links(&track.plugin)
                        .url_for(jf_plugin_host::ItemKind::Track, &track.id)
                });
                Ok(source_now_playing(item, track, link))
            }
            TrackRef::Library(_) | TrackRef::File(_) => Ok(self.resolve_local(item)?.1),
        }
    }

    /// Resolves a library track or a file.
    fn resolve_local(&self, item: &QueueItem) -> Result<(PlayItem, NowPlaying), CoreError> {
        match &item.track {
            TrackRef::Library(id) => {
                let track = self
                    .library
                    .track(*id)?
                    .ok_or_else(|| CoreError::Other(format!("library track {id} no longer exists")))?;
                let artwork = self
                    .library
                    .artwork_path(*id)
                    .ok()
                    .flatten()
                    .or_else(|| theme::folder_artwork(&track.path));
                let rg = &track.replaygain;
                let play = PlayItem {
                    id: item.uid,
                    source: MediaSource::File(track.path.clone()),
                    album: album_key(
                        track.album_id.map(|id| id.to_string()),
                        track.album.as_deref(),
                        track.album_artist.as_deref().or(track.artist.as_deref()),
                    ),
                    replaygain: Some(ReplayGainInfo {
                        track_gain_db: rg.track_gain_db,
                        track_peak: rg.track_peak,
                        album_gain_db: rg.album_gain_db,
                        album_peak: rg.album_peak,
                    }),
                };
                let now = NowPlaying {
                    uid: item.uid,
                    track: item.track.clone(),
                    path: Some(track.path.clone()),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration_ms: track.duration_ms,
                    artwork: artwork.map(ArtworkRef::File),
                    format: None,
                    link: None,
                };
                Ok((play, now))
            }
            TrackRef::File(path) => {
                if !path.is_file() {
                    return Err(CoreError::Other(format!("{} is missing", path.display())));
                }
                // Tags are read from the file itself (headers only, no
                // artwork); a file without readable tags still plays.
                let tags = jf_library::read_file_tags(path, false).unwrap_or_default();
                let rg = &tags.replaygain;
                let play = PlayItem {
                    id: item.uid,
                    source: MediaSource::File(path.clone()),
                    // Files: the same album tag in the same folder.
                    album: album_key(
                        path.parent().map(|d| d.display().to_string()),
                        tags.album.as_deref(),
                        tags.album_artist.as_deref().or(tags.artist.as_deref()),
                    ),
                    replaygain: Some(ReplayGainInfo {
                        track_gain_db: rg.track_gain_db,
                        track_peak: rg.track_peak,
                        album_gain_db: rg.album_gain_db,
                        album_peak: rg.album_peak,
                    }),
                };
                let title = tags.title.clone().unwrap_or_else(|| {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                });
                let now = NowPlaying {
                    uid: item.uid,
                    track: item.track.clone(),
                    path: Some(path.clone()),
                    title,
                    artist: tags.artist,
                    album: tags.album,
                    duration_ms: tags.duration_ms,
                    artwork: theme::folder_artwork(path).map(ArtworkRef::File),
                    format: None,
                    link: None,
                };
                Ok((play, now))
            }
            TrackRef::Source(_) => Err(CoreError::Other("a source track is not a local file".into())),
        }
    }

    // ---- helper results -------------------------------------------------

    fn on_library(&mut self, msg: LibraryMsg) {
        match msg {
            LibraryMsg::ScanStarted => self.shared.update(|s| s.scanning = true),
            LibraryMsg::Progress(p) => self.shared.emit(Event::Scan(ScanStatus::Progress(p))),
            LibraryMsg::ScanFinished(result) => {
                self.shared.update(|s| s.scanning = false);
                self.shared.emit(Event::Scan(match result {
                    Ok(report) => ScanStatus::Finished(report),
                    Err(e) => ScanStatus::Failed(e),
                }));
            }
            LibraryMsg::Failed(id, error) => self.shared.emit(Event::CommandFailed { id, error }),
            LibraryMsg::Changed(revision) => {
                self.shared.update(|s| s.library_revision = revision);
                self.shared.emit(Event::LibraryChanged { revision });
            }
        }
    }

    fn on_media(&mut self, msg: MediaMsg) {
        let current = self.shared.read_queue().current_uid();
        match msg {
            MediaMsg::Lyrics(l) if Some(l.uid) == current => {
                let l: Arc<LyricsSnapshot> = Arc::from(l);
                self.shared.update(|s| s.lyrics = Some(l.clone()));
                self.shared.emit(Event::Lyrics(l));
            }
            MediaMsg::Theme(t) if t.uid == current => {
                self.artwork_theme = Some(Arc::from(t));
                self.publish_theme(false);
            }
            _ => {} // for a track that is no longer current
        }
    }

    fn on_sources(&mut self, msg: SourcesMsg) {
        match msg {
            SourcesMsg::Done { id, result } => {
                self.sources.job_done();
                let declined_changed = match result {
                    Ok(Done::Installed(plugin)) => self.sources.installed(&plugin, true),
                    Ok(Done::Uninstalled(plugin)) => self.sources.uninstalled(&plugin),
                    Ok(Done::LoggedOut { note }) => {
                        if let Some(message) = note {
                            self.shared.emit(Event::Error { message });
                        }
                        false
                    }
                    Err(error) => {
                        self.shared.emit(Event::CommandFailed { id, error });
                        false
                    }
                };
                if declined_changed {
                    self.save_setting(DECLINED_BUNDLED_KEY, self.sources.declined());
                }
                self.publish_sources(true);
            }
            SourcesMsg::Bundled(outcome) => {
                if let Bundled::Failed { path, error } = &outcome {
                    let name = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    tracing::warn!(path = %path.display(), %error, "a bundled plugin was not installed");
                    self.shared.emit(Event::Error {
                        message: format!("The bundled plugin {name} could not be installed: {error}"),
                    });
                }
                if self.sources.bundled_outcome(&outcome).is_some() {
                    self.publish_sources(true);
                }
            }
            SourcesMsg::Refresh | SourcesMsg::SecretsChecked => self.publish_sources(false),
            SourcesMsg::LoginEnded { flow } => {
                self.sources.login_ended(flow);
                self.publish_sources(false);
            }
            SourcesMsg::Resolved { uid, result } => self.on_resolved(uid, result),
            SourcesMsg::Collection {
                id,
                plugin,
                then,
                result,
            } => {
                if let Err(e) = self.queue_collection(&plugin, then, result) {
                    self.shared.emit(Event::CommandFailed {
                        id,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    /// A source track's stream URL arrived (or will not).
    fn on_resolved(&mut self, uid: u64, result: Result<HttpSource, String>) {
        self.sources.resolved(uid, &result);
        let current = self.shared.read_queue().current_uid();
        if self.pending_start == Some(uid) && current == Some(uid) {
            if let Err(error) = result {
                // Like a file that cannot be opened: say why, move on.
                self.pending_start = None;
                let what = self
                    .shared
                    .read_queue()
                    .current()
                    .map(|i| describe(&i.track))
                    .unwrap_or_default();
                self.shared.emit(Event::Error {
                    message: format!("skipping {what}: {error}"),
                });
                self.failures += 1;
                if self.failures > self.shared.read_queue().len() {
                    self.stop_playback(Status::Stopped);
                    return;
                }
                let next = self.shared.write_queue().next(Advance::Manual);
                self.queue_changed();
                if next.is_none() {
                    self.end_of_queue();
                    return;
                }
            } else if self.status != Status::Playing {
                // Paused meanwhile: it starts on Play.
                return;
            }
            if let Err(e) = self.start_current() {
                self.shared.emit(Event::Error {
                    message: e.to_string(),
                });
                self.stop_playback(Status::Stopped);
            }
        } else if result.is_ok()
            && self.shared.read_queue().peek_next(Advance::Auto).map(|i| i.uid) == Some(uid)
        {
            // The entry after the current one: arm it for a gapless join.
            self.rearm();
        }
    }

    /// Queues a fetched source collection.
    fn queue_collection(
        &mut self,
        plugin: &str,
        then: Then,
        result: Result<Vec<Item>, String>,
    ) -> Result<(), CoreError> {
        let items = result.map_err(CoreError::Other)?;
        match then {
            Then::Play { start, shuffle } => {
                let (tracks, start) = sources::collection_queue(plugin, &items, start)?;
                {
                    let mut q = self.shared.write_queue();
                    q.set_shuffle(shuffle);
                    q.set_items(tracks, Some(start))?;
                }
                self.resume_at = None;
                self.failures = 0;
                self.queue_changed();
                self.start_current()?;
            }
            Then::Enqueue { play_next } => {
                let (tracks, _) = sources::collection_queue(plugin, &items, 0)?;
                {
                    let mut q = self.shared.write_queue();
                    if play_next {
                        q.insert_next(tracks);
                    } else {
                        q.append(tracks);
                    }
                }
                self.queue_changed();
                self.rearm();
            }
        }
        Ok(())
    }

    // ---- publishing and persistence ----------------------------------

    fn publish_sources(&mut self, force: bool) {
        self.sources
            .publish(self.settings.allow_community_plugins, &self.shared, force);
    }

    fn publish_playback(&mut self) {
        self.playback_version += 1;
        if let Some(engine) = &self.engine
            && self.awaiting_start.is_none()
        {
            self.anchor = engine.anchor();
        }
        if !self.is_active() {
            self.anchor.rate = 0.0;
        }
        let snap = PlaybackSnapshot {
            version: self.playback_version,
            status: self.status,
            anchor: self.anchor,
            volume: self.volume,
            eq: self.eq.clone(),
            replaygain_mode: self.rg_mode,
            preamp_db: self.preamp_db,
        };
        self.shared.update(|s| s.playback = snap.clone());
        self.shared.emit(Event::Playback(snap));
    }

    fn publish_queue(&mut self) {
        self.queue_version += 1;
        let snap = {
            let q = self.shared.read_queue();
            QueueSnapshot {
                version: self.queue_version,
                len: q.len(),
                position: q.position(),
                current: q.current(),
                shuffle: q.is_shuffled(),
                repeat: q.repeat(),
                up_next: q.up_next(UP_NEXT_WINDOW),
            }
        };
        self.shared.update(|s| s.queue = snap.clone());
        self.shared.emit(Event::Queue(snap));
    }

    fn queue_changed(&mut self) {
        self.publish_queue();
        if self.config.persist_session && self.save_due.is_none() {
            self.save_due = Some(Instant::now() + SAVE_DEBOUNCE);
        }
    }

    fn set_now_playing(&mut self, now: Option<NowPlaying>) {
        let now = now.map(Arc::new);
        if now.is_none() {
            self.media_uid = None;
        }
        let cleared = now.is_none();
        if cleared {
            self.artwork_theme = None;
        }
        self.shared.update(|s| {
            s.now_playing = now.clone();
            if cleared {
                s.lyrics = None;
                s.theme = None;
            }
        });
        self.shared.emit(Event::NowPlaying(now));
        if cleared {
            // With a fixed hue the colours stay; with the artwork's, shells
            // fall back to the default theme on NowPlaying(None).
            self.publish_theme(false);
        }
    }

    /// The theme to show: the artwork's, with the hue setting applied. With a
    /// fixed hue there is one even when nothing plays.
    fn current_theme(&self) -> Option<ThemeSnapshot> {
        let fixed = match self.settings.hue_source {
            HueSource::Fixed { hue } if hue.is_finite() => Some(hue.rem_euclid(360.0)),
            _ => None,
        };
        match (&self.artwork_theme, fixed) {
            (Some(t), None) => Some((**t).clone()),
            (Some(t), Some(hue)) => Some(ThemeSnapshot {
                hue,
                from_artwork: false,
                light: jf_palette::resolve_theme(hue, jf_palette::Appearance::Light),
                dark: jf_palette::resolve_theme(hue, jf_palette::Appearance::Dark),
                ..(**t).clone()
            }),
            (None, Some(hue)) => Some(idle_theme(hue)),
            (None, None) => None,
        }
    }

    /// Publishes the current theme. `revert` also publishes the default theme
    /// when there is none (the hue setting went back to the artwork while
    /// nothing plays), so shells hear of the change.
    fn publish_theme(&mut self, revert: bool) {
        let theme = self
            .current_theme()
            .or_else(|| revert.then(|| idle_theme(jf_tokens::DEFAULT_HUE)))
            .map(Arc::new);
        if let Some(theme) = theme {
            self.shared.update(|s| s.theme = Some(theme.clone()));
            self.shared.emit(Event::Theme(theme));
        }
    }

    fn save_queue(&mut self) {
        self.save_due = None;
        if !self.config.persist_session {
            return;
        }
        if let Some(store) = &mut self.store {
            let q = self.shared.read_queue();
            if let Err(e) = store.save_queue(&q) {
                tracing::warn!("could not save the queue: {e}");
            }
        }
    }

    fn save_playback(&mut self) {
        if !self.config.persist_session {
            return;
        }
        let uid = self.shared.read_queue().current_uid();
        let position_ms = match self.resume_at {
            // Not played yet in this run: keep the restored position.
            Some((resume_uid, ms)) if !self.is_active() && Some(resume_uid) == uid => ms,
            _ => u64::try_from(self.position_us() / 1_000).unwrap_or(0),
        };
        if let Some(store) = &self.store {
            let state = PlaybackState {
                uid,
                position_ms,
                was_playing: self.status == Status::Playing,
            };
            if let Err(e) = store.save_playback(&state) {
                tracing::warn!("could not save the playback position: {e}");
            }
        }
    }

    fn save_setting<T: serde::Serialize>(&self, key: &str, value: &T) {
        if !self.config.persist_session {
            return;
        }
        if let Some(store) = &self.store
            && let Err(e) = store.set_setting(key, value)
        {
            tracing::warn!("could not save setting {key}: {e}");
        }
    }
}

impl Drop for Actor {
    fn drop(&mut self) {
        // Also on a panic: the worker holds a sender of its own queue (for the
        // folder watcher), so it stops only when told to.
        let _ = self.library_jobs.send(LibraryJob::Shutdown);
    }
}

/// The key the engine compares to join tracks of one album gaplessly while
/// crossfading others: `scope` (a library album id, or a folder) and the
/// album's names. No album tag, no key.
fn album_key(scope: Option<String>, album: Option<&str>, artist: Option<&str>) -> Option<String> {
    let album = album.map(str::trim).filter(|a| !a.is_empty())?;
    Some(format!(
        "{}\u{1f}{}\u{1f}{}",
        scope.unwrap_or_default(),
        album.to_lowercase(),
        artist.unwrap_or_default().trim().to_lowercase()
    ))
}

/// The theme of a fixed hue with nothing playing (no artwork, no backdrops).
fn idle_theme(hue: f32) -> ThemeSnapshot {
    ThemeSnapshot {
        uid: None,
        hue,
        from_artwork: false,
        light: jf_palette::resolve_theme(hue, jf_palette::Appearance::Light),
        dark: jf_palette::resolve_theme(hue, jf_palette::Appearance::Dark),
        backdrop_light: None,
        backdrop_dark: None,
    }
}

fn describe(track: &TrackRef) -> String {
    match track {
        TrackRef::Library(id) => format!("library track {id}"),
        TrackRef::File(path) => path.display().to_string(),
        TrackRef::Source(t) => format!("{} ({})", t.title, t.plugin),
    }
}

/// How far a queue entry is from playing.
enum Resolution {
    /// The engine can play it now.
    Ready(PlayItem),
    /// A source track whose stream URL its plugin is still resolving.
    Pending,
}

/// The engine's item for a source track: its stream, and an album key for
/// gapless joins inside one album of one source.
fn source_play_item(uid: u64, track: &SourceTrack, source: HttpSource) -> PlayItem {
    PlayItem {
        id: uid,
        source: MediaSource::Http(source),
        album: album_key(
            Some(track.plugin.clone()),
            track.album.as_deref(),
            track.artist.as_deref(),
        ),
        replaygain: None,
    }
}

/// What to show for a source track: the metadata it was listed with.
fn source_now_playing(item: &QueueItem, track: &SourceTrack, link: Option<String>) -> NowPlaying {
    NowPlaying {
        uid: item.uid,
        track: item.track.clone(),
        path: None,
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
        artwork: track.artwork_url.clone().map(ArtworkRef::Url),
        format: None,
        link,
    }
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ---- helper threads ----------------------------------------------------

fn spawn_library_worker(
    db: PathBuf,
    cache: PathBuf,
    watch: bool,
    tx: Sender<Msg>,
) -> Result<Sender<LibraryJob>, CoreError> {
    let (jobs, rx) = unbounded::<LibraryJob>();
    let own_jobs = jobs.clone();
    thread::Builder::new().name("jf-library".into()).spawn(move || {
        let send = |msg| {
            let _ = tx.send(Msg::Library(msg));
        };
        let library = match Library::open(&db, &cache) {
            Ok(l) => l,
            Err(e) => {
                for job in rx.iter() {
                    match job {
                        LibraryJob::AddRoot(id, _) | LibraryJob::RemoveRoot(id, _) | LibraryJob::Scan(id) => {
                            send(LibraryMsg::Failed(id, e.to_string()));
                        }
                        LibraryJob::Shutdown => break,
                        LibraryJob::ScanPaths(_) | LibraryJob::Watch(_) => {}
                    }
                }
                return;
            }
        };
        let mut worker = LibraryWorker {
            library,
            tx: tx.clone(),
            jobs: own_jobs,
            watcher: None,
        };
        if watch {
            worker.watch(true);
        }
        // A job taken off the queue while gathering watcher reports.
        let mut pending: Option<LibraryJob> = None;
        loop {
            let job = match pending.take() {
                Some(job) => job,
                None => match rx.recv() {
                    Ok(job) => job,
                    Err(_) => break,
                },
            };
            match job {
                LibraryJob::AddRoot(id, path) => match worker.library.add_root(&path) {
                    Ok(_) => {
                        send(LibraryMsg::Changed(worker.library.revision().unwrap_or(0)));
                        worker.rewatch();
                        worker.scan(Some(id), Some(&[path]));
                    }
                    Err(e) => send(LibraryMsg::Failed(id, e.to_string())),
                },
                LibraryJob::RemoveRoot(id, path) => match worker.library.remove_root(&path) {
                    Ok(()) => {
                        send(LibraryMsg::Changed(worker.library.revision().unwrap_or(0)));
                        worker.rewatch();
                    }
                    Err(e) => send(LibraryMsg::Failed(id, e.to_string())),
                },
                LibraryJob::Scan(id) => {
                    // A full scan also brings back watches of folders that
                    // were missing (a drive mounted again).
                    worker.rewatch();
                    worker.scan(Some(id), None);
                }
                LibraryJob::ScanPaths(mut paths) => {
                    // Reports that piled up during a long scan go in one pass.
                    while let Ok(next) = rx.try_recv() {
                        match next {
                            LibraryJob::ScanPaths(more) => paths.extend(more),
                            other => {
                                pending = Some(other);
                                break;
                            }
                        }
                    }
                    paths.sort();
                    paths.dedup();
                    worker.scan(None, Some(&paths));
                }
                LibraryJob::Watch(on) => worker.watch(on),
                LibraryJob::Shutdown => break,
            }
        }
        // Dropping the worker stops the watcher thread.
    })?;
    Ok(jobs)
}

/// The library worker's state: the writing connection and the folder watcher.
struct LibraryWorker {
    library: Library,
    tx: Sender<Msg>,
    /// The worker's own queue, for the watcher's reports.
    jobs: Sender<LibraryJob>,
    watcher: Option<LibraryWatcher>,
}

impl LibraryWorker {
    fn send(&self, msg: LibraryMsg) {
        let _ = self.tx.send(Msg::Library(msg));
    }

    /// Scans every root (`paths` `None`) or the given paths, reporting
    /// progress, the result and the new revision.
    fn scan(&self, id: Option<CommandId>, paths: Option<&[PathBuf]>) {
        self.send(LibraryMsg::ScanStarted);
        let mut last = Instant::now();
        let mut progress = |p| {
            if last.elapsed() >= PROGRESS_INTERVAL {
                last = Instant::now();
                let _ = self.tx.send(Msg::Library(LibraryMsg::Progress(p)));
            }
        };
        let result = match paths {
            Some(paths) => self.library.scan_paths(paths, &mut progress),
            None => self.library.scan(&mut progress),
        };
        match result {
            Ok(report) => {
                self.send(LibraryMsg::ScanFinished(Ok(report)));
                self.send(LibraryMsg::Changed(self.library.revision().unwrap_or(0)));
            }
            Err(e) => {
                if let Some(id) = id {
                    self.send(LibraryMsg::Failed(id, e.to_string()));
                }
                self.send(LibraryMsg::ScanFinished(Err(e.to_string())));
            }
        }
    }

    /// Starts or stops watching the roots.
    fn watch(&mut self, on: bool) {
        if !on {
            self.watcher = None;
            return;
        }
        if self.watcher.is_some() {
            return;
        }
        let roots = self.library.roots().unwrap_or_default();
        let jobs = self.jobs.clone();
        match LibraryWatcher::new(roots, WATCH_DEBOUNCE, move |paths| {
            let _ = jobs.send(LibraryJob::ScanPaths(paths));
        }) {
            Ok(watcher) => {
                for (root, kind) in watcher.unwatched() {
                    tracing::warn!("not watching {}: {kind:?}", root.display());
                }
                self.watcher = Some(watcher);
            }
            Err(e) => tracing::warn!("folder watching is off: {e}"),
        }
    }

    /// Watches the current roots, again.
    fn rewatch(&mut self) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        if let Some(e) = watcher.take_error() {
            tracing::warn!("folder watching: {e}");
        }
        let roots = self.library.roots().unwrap_or_default();
        if let Err(e) = watcher.set_roots(roots) {
            tracing::warn!("folder watching: {e}");
        }
    }
}

fn spawn_media_worker(
    db: PathBuf,
    cache: PathBuf,
    sources: Option<Arc<Ctx>>,
    tx: Sender<Msg>,
) -> Result<Sender<MediaJob>, CoreError> {
    let (jobs, rx) = unbounded::<MediaJob>();
    thread::Builder::new().name("jf-media".into()).spawn(move || {
        let library = Library::open(&db, &cache).ok();
        for mut job in rx.iter() {
            // Only the newest request matters when several are queued.
            while let Ok(newer) = rx.try_recv() {
                job = newer;
            }
            let uid = job.uid();
            let bytes = match job {
                MediaJob::Local {
                    uid,
                    audio,
                    library_id,
                    artwork,
                } => {
                    // Files outside the library: their own tags, with the cover.
                    let file_tags = if library_id.is_none() {
                        jf_library::read_file_tags(&audio, true).ok()
                    } else {
                        None
                    };
                    let found = lyrics::find_lyrics(&audio, || match library_id {
                        Some(id) => library.as_ref()?.embedded_lyrics(id).ok().flatten(),
                        None => file_tags.as_ref()?.lyrics.clone(),
                    });
                    let (doc, source) = match found {
                        Some((doc, source)) => (Some(doc), Some(source)),
                        None => (None, None),
                    };
                    let timeline = doc.as_ref().map(|d| d.timeline());
                    let lyrics = LyricsSnapshot {
                        uid,
                        doc,
                        timeline,
                        source,
                    };
                    let _ = tx.send(Msg::Media(MediaMsg::Lyrics(Box::new(lyrics))));
                    file_tags
                        .and_then(|t| t.cover)
                        .or_else(|| artwork.as_deref().and_then(|p| std::fs::read(p).ok()))
                }
                MediaJob::Source {
                    uid,
                    plugin,
                    item,
                    listed,
                } => {
                    let found = sources.as_ref().and_then(|ctx| ctx.lyrics(&plugin, &item));
                    let lyrics = match found {
                        Some((doc, synced)) => LyricsSnapshot {
                            uid,
                            // Unsynced (content policy): no timeline to follow.
                            timeline: synced.then(|| doc.timeline()),
                            doc: Some(doc),
                            source: Some(LyricsSource::Plugin(plugin.clone())),
                        },
                        None => LyricsSnapshot {
                            uid,
                            doc: None,
                            timeline: None,
                            source: None,
                        },
                    };
                    let _ = tx.send(Msg::Media(MediaMsg::Lyrics(Box::new(lyrics))));
                    if !rx.is_empty() {
                        continue; // another track already: its artwork matters
                    }
                    sources
                        .as_ref()
                        .and_then(|ctx| ctx.artwork(&plugin, &item, listed))
                }
            };
            let t = theme::theme_for_artwork(bytes.as_deref());
            let (backdrop_light, backdrop_dark) = match &bytes {
                Some(b) if !b.is_empty() => match theme::backdrops(b, &theme::artwork_key(b), &cache) {
                    Ok((l, d)) => (Some(l), Some(d)),
                    Err(e) => {
                        tracing::warn!("could not bake a backdrop: {e}");
                        (None, None)
                    }
                },
                _ => (None, None),
            };
            let snapshot = ThemeSnapshot {
                uid: Some(uid),
                hue: t.pick.hue,
                from_artwork: t.pick.from_artwork,
                light: t.light,
                dark: t.dark,
                backdrop_light,
                backdrop_dark,
            };
            let _ = tx.send(Msg::Media(MediaMsg::Theme(Box::new(snapshot))));
        }
    })?;
    Ok(jobs)
}

/// Used by tests and the CLI: true when `path` looks like audio the engine
/// can open.
pub(crate) fn is_audio_path(path: &Path) -> bool {
    const EXTS: [&str; 13] = [
        "mp3", "flac", "ogg", "oga", "m4a", "mp4", "aac", "wav", "aif", "aiff", "aifc", "caf", "mka",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}
