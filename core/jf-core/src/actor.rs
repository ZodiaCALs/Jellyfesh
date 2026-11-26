//! The core actor: one thread owns all state and applies commands in order.
//!
//! Helpers run on their own threads so the actor never blocks on slow I/O:
//! the library worker (the single SQLite writer: roots and scans), the
//! media worker (lyrics files and tags, artwork, backdrop bakes; for source
//! tracks, lyrics and artwork through their plugin) and the helpers of the
//! online sources ([`crate::sources`]: plugin calls, stream URLs, logins).
//! All of them park on a channel when idle, so an idle core uses no CPU and
//! no timers tick.

use std::collections::HashMap;
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
    ArtworkRef, Collection, Command, CommandId, Config, DevicesSnapshot, DownloadsPaused, DownloadsSnapshot,
    Event, ExternalPlayer, HueSource, LyricsSnapshot, LyricsSource, NowPlaying, PlaybackSnapshot,
    QualityNote, QueueSnapshot, RemoteAction, ScanStatus, SearchScope, SearchUpdate, Settings, Snapshot,
    SourceLibrary, SourceLibrarySnapshot, SourcesSnapshot, Status, StreamingQuality, ThemeSnapshot,
    UP_NEXT_WINDOW,
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
/// The settings key of the Jellyfesh Pro licence key the person entered.
const LICENCE_KEY: &str = "pro_licence_key";
/// The plugins' own quality settings were carried over (ADR-0026 §2).
const QUALITY_CARRIED_KEY: &str = "quality_carried_over";
/// How long the appearance plugin may take to give its look.
const LOOK_WAIT: Duration = Duration::from_secs(25);
/// Longest the core waits at shutdown for installs, uninstalls and logouts in flight (a logout
/// may take 45 s when the service is slow to revoke its tokens).
const JOB_DRAIN: Duration = Duration::from_secs(60);

pub(crate) enum Msg {
    Command(CommandId, Command),
    Library(LibraryMsg),
    Media(MediaMsg),
    Sources(SourcesMsg),
    /// What the app's store says about Jellyfesh Pro.
    StoreEntitlement(crate::types::StoreEntitlement),
    /// AI Lyrics helpers: plans, job stages and ends, model downloads.
    AiLyrics(crate::ai_lyrics::AiMsg),
    /// The appearance plugin's look, for the request `generation`.
    Look {
        generation: u64,
        plugin: String,
        result: Result<jf_plugin_host::Look, String>,
    },
    /// A source picture is ready, or cannot be had.
    Artwork(crate::types::ArtworkReady),
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
        /// Now Playing's picture URL, which the fetched picture is kept under.
        artwork_url: Option<String>,
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
    /// A source track's picture, fetched through its plugin and kept as a file.
    NowPlayingArtwork {
        uid: u64,
        path: PathBuf,
    },
}

/// State shared with [`crate::Core`] handles on other threads.
pub(crate) struct Shared {
    pub(crate) snapshot: RwLock<Snapshot>,
    pub(crate) subscribers: Mutex<Vec<Sender<Event>>>,
    pub(crate) queue: RwLock<Queue>,
    /// The remote session whose updates the `Remote` slice takes (0: none).
    pub(crate) remote_session: AtomicU64,
    /// Players the app registered, by source id.
    pub(crate) external: RwLock<HashMap<String, External>>,
    /// The plugin host, for queries the actor need not answer.
    pub(crate) host: RwLock<Option<jf_plugin_host::PluginHost>>,
    /// Source pictures fetched through the core (set once at start).
    pub(crate) artwork: std::sync::OnceLock<crate::artwork::Artwork>,
    /// The device each remote source last played on in this run (the next
    /// session plays there when no device is active).
    pub(crate) remote_devices: Mutex<HashMap<String, String>>,
}

/// A player the app registered, with the name its source shows.
pub(crate) type External = (String, Arc<dyn ExternalPlayer>);

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
    /// Jellyfesh Pro: where it comes from (ADR-0021).
    pro: crate::pro::Pro,
    /// The appearance plugin the `Look` slice comes from.
    look: LookState,
    /// AI Lyrics: jobs, consents and speech models (ADR-0019).
    ai: crate::ai_lyrics::AiLyrics,
    /// The update thread (it publishes the `Update` slice itself).
    update: Option<Sender<crate::update::UpdateMsg>>,
    /// The queue entry automatic AI Lyrics last considered.
    auto_lyrics: Option<u64>,
    /// For helper threads the actor starts (the look).
    tx: Sender<Msg>,
    /// The network as the app last reported it (`SetNetworkState`).
    network: Network,
    /// Versions of the v0.0.8 slices.
    source_library_version: u64,
    downloads_version: u64,
    /// A source collection whose first pages play while the rest arrives:
    /// its request and its queue entries so far. It stops growing once none
    /// of them is queued (the queue was replaced).
    growing: Option<(CommandId, Vec<u64>)>,
    /// The playing source song reopened at a new streaming level: its start
    /// is the same listen going on.
    reopening: Option<u64>,
}

/// The network the platform reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Network {
    online: bool,
    metered: bool,
}

/// What the `Look` slice is made from.
#[derive(Debug, Default)]
struct LookState {
    version: u64,
    /// The appearance plugin and its version, while Pro is active.
    plugin: Option<(String, String)>,
    generation: u64,
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
                    can_install_plugins: true,
                }),
                remote: Arc::new(remote::ended(0, None)),
                muted: false,
                ai_lyrics: Arc::new(crate::types::AiLyricsSnapshot {
                    version: 0,
                    available: false,
                    plugin: None,
                    plan: None,
                    need: crate::types::AiLyricsNeed::Unavailable {
                        reason: "no AI Lyrics plugin is installed".into(),
                    },
                    jobs: Vec::new(),
                    models: Vec::new(),
                    consents: Vec::new(),
                }),
                look: Arc::new(crate::types::LookSnapshot::defaults(0)),
                pro: Arc::new(crate::types::ProSnapshot {
                    version: 0,
                    active: false,
                    source: crate::types::ProSource::None,
                    preview: false,
                    licence_error: None,
                    stores: Vec::new(),
                    licence_keys: true,
                }),
                update: Arc::new(crate::types::UpdateSnapshot {
                    version: 0,
                    supported: false,
                    checking: false,
                    available: None,
                    last_checked_ms: None,
                    auto: false,
                    error: None,
                }),
                source_library: Arc::new(SourceLibrarySnapshot {
                    version: 0,
                    sources: Vec::new(),
                }),
                downloads: Arc::new(DownloadsSnapshot {
                    version: 0,
                    folder: PathBuf::new(),
                    items: Vec::new(),
                    bytes_used: 0,
                    paused: None,
                }),
            }),
            subscribers: Mutex::new(Vec::new()),
            queue: RwLock::new(queue),
            remote_session: AtomicU64::new(0),
            external: RwLock::new(HashMap::new()),
            host: RwLock::new(None),
            artwork: std::sync::OnceLock::new(),
            remote_devices: Mutex::new(HashMap::new()),
        }
    }

    /// The plugin host, when sources run.
    pub(crate) fn host(&self) -> Option<jf_plugin_host::PluginHost> {
        self.host.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// A cached source picture, or `None` while it is fetched (see
    /// [`crate::Core::source_artwork`]).
    pub(crate) fn source_artwork(&self, url: &str, size_px: u32) -> Option<PathBuf> {
        self.artwork.get()?.get(url, size_px)
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
        let mut settings = settings.unwrap_or_else(|| Settings {
            eq: eq.clone(),
            replaygain_mode: rg.0,
            preamp_db: rg.1,
            ..Settings::default()
        });
        if config.distribution.store().is_some() {
            // Store builds keep community plugins off (ADR-0021 §4).
            settings.allow_community_plugins = false;
        }
        // ADR-0026 §2: the plugins' own quality settings went in v0.0.8. Once,
        // the level they agreed on becomes the app's, read before the bundled
        // plugins' new versions drop them from their stored settings.
        if config.restore_session
            && config.persist_session
            && let Some(store) = &store
            && !store
                .setting::<bool>(QUALITY_CARRIED_KEY)
                .ok()
                .flatten()
                .unwrap_or(false)
        {
            if settings.streaming_quality == StreamingQuality::Auto
                && let Some(level) = crate::quality::carried_over(&config.data_dir.join("plugins"))
            {
                tracing::info!(?level, "the plugins' streaming quality is now the app's");
                settings.streaming_quality = level;
                if let Err(e) = store.set_setting("settings", &settings) {
                    tracing::warn!("could not save the carried-over streaming quality: {e}");
                }
            }
            if let Err(e) = store.set_setting(QUALITY_CARRIED_KEY, &true) {
                tracing::warn!("could not note the carried-over streaming quality: {e}");
            }
        }
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
        *shared.host.write().unwrap_or_else(|e| e.into_inner()) = sources.ctx().map(|ctx| ctx.host.clone());

        // Jellyfesh Pro: a licence key kept from an earlier run is checked again.
        let licence: Option<String> = store
            .as_ref()
            .and_then(|s| s.setting::<String>(LICENCE_KEY).ok().flatten());
        let pro = crate::pro::Pro::new(&config.pro, config.distribution, licence.as_deref());
        if let Some(host) = shared.host() {
            host.set_pro(pro.active());
        }
        let _ = shared.artwork.set(crate::artwork::Artwork::new(
            &config.cache_dir,
            sources.ctx(),
            tx.clone(),
        ));
        let speech_store = config.speech_store.clone().unwrap_or_else(|| {
            let dir = config.data_dir.join("speech-models");
            match &config.speech_model_url {
                Some(url) => jf_speech::ModelStore::with_base_url(dir, url),
                None => jf_speech::ModelStore::new(dir),
            }
        });
        let ai = crate::ai_lyrics::AiLyrics::new(
            tx.clone(),
            &config.data_dir,
            speech_store,
            config.speech.clone(),
            shared.host(),
        );
        let update = crate::update::spawn(
            shared.clone(),
            config.update_url.clone(),
            &config.data_dir,
            config.distribution == crate::types::Distribution::Direct,
            settings.check_for_updates,
        )
        .map_err(|e| tracing::warn!("could not start the update check: {e}"))
        .ok();
        let actor_tx = tx.clone();
        let media_jobs = spawn_media_worker(
            db,
            config.cache_dir.clone(),
            sources.ctx(),
            ai.made().clone(),
            shared.artwork.get().map(|a| a.dir().to_owned()),
            tx,
        )?;

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
            pro,
            look: LookState::default(),
            ai,
            update,
            auto_lyrics: None,
            tx: actor_tx,
            network: Network {
                online: true,
                metered: false,
            },
            source_library_version: 0,
            downloads_version: 0,
            growing: None,
            reopening: None,
        };
        actor.sources.set_level(actor.streaming_level());
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
        actor.publish_pro();
        actor.publish_sources(true);
        actor.ai.publish(&actor.shared);
        actor.publish_downloads();
        Ok((actor, shared))
    }

    pub(crate) fn run(mut self, rx: Receiver<Msg>) {
        self.resume_on_launch();
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
                    Ok(Msg::StoreEntitlement(entitlement)) => self.on_store_entitlement(entitlement),
                    Ok(Msg::AiLyrics(msg)) => self.on_ai_lyrics(msg),
                    Ok(Msg::Look { generation, plugin, result }) => self.on_look(generation, &plugin, result),
                    Ok(Msg::Artwork(ready)) => self.shared.emit(Event::Artwork(Arc::new(ready))),
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
                collection:
                    Collection::Source {
                        plugin,
                        id: node,
                        item_kind,
                    },
                start,
                shuffle,
            } => {
                if self.is_remote(&plugin) {
                    let context = if self.external(&plugin).is_some() {
                        sources::bare_item(&node, crate::remote::host_kind(item_kind))
                    } else {
                        self.sources.remote_node(&plugin, &node, item_kind)?
                    };
                    // Shuffle with no song chosen starts anywhere: services start at the
                    // offset given (Spotify even with shuffle on), so the core picks one.
                    let start = if shuffle && start == 0 {
                        random_below(context.child_count.map_or(0, |n| n as usize))
                    } else {
                        start
                    };
                    let op = Op::Play {
                        context: Some(Box::new(context)),
                        tracks: Vec::new(),
                        start: u32::try_from(start).unwrap_or(u32::MAX),
                        shuffle: Some(shuffle),
                    };
                    return self.start_remote(&plugin, op);
                }
                self.sources
                    .fetch_collection(id, plugin, node, item_kind, Then::Play { start, shuffle })?;
            }
            Command::EnqueueCollection {
                collection:
                    Collection::Source {
                        plugin,
                        id: node,
                        item_kind,
                    },
                play_next,
            } => {
                if self.is_remote(&plugin) {
                    return Err(self.remote_queue_refusal(&plugin));
                }
                self.sources
                    .fetch_collection(id, plugin, node, item_kind, Then::Enqueue { play_next })?;
            }
            Command::PlayCollection {
                collection,
                start,
                shuffle,
            } => {
                let requested_start = start;
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
                    if shuffle && requested_start == 0 {
                        // Shuffle with no song chosen: any song first.
                        q.set_items_from_any(tracks);
                    } else {
                        q.set_items(tracks, Some(start))?;
                    }
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
                self.plugin_settings_changed(&plugin);
            }
            Command::AcknowledgeUnofficial { id: plugin } => {
                if self.sources.store_build() {
                    return Err(CoreError::Other(
                        "community plugins are not available in this edition of Jellyfesh".into(),
                    ));
                }
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
            Command::ResetPluginSettings { plugin } => {
                self.sources.reset_settings(&plugin)?;
                self.publish_sources(true);
                self.plugin_settings_changed(&plugin);
            }
            Command::MakeLyrics { song } => {
                let song = self.lyrics_song(&song)?;
                self.ai.make(song).map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::CancelLyrics { job } => {
                self.ai.cancel(job).map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::DeleteMadeLyrics { song } => {
                let song = self.lyrics_song(&song)?;
                let outcome = self.ai.delete_made(&song.identity).map_err(CoreError::Other)?;
                self.made_lyrics_changed(outcome);
            }
            Command::AllowAudioUpload { plugin, origin } => {
                self.ai
                    .set_consent(&plugin, &origin, true)
                    .map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::RevokeAudioUpload { plugin, origin } => {
                self.ai
                    .set_consent(&plugin, &origin, false)
                    .map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::DownloadSpeechModel { id: model } => {
                self.ai.download(&model).map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::CancelSpeechModelDownload { id: model } => {
                self.ai.cancel_download(&model).map_err(CoreError::Other)?;
            }
            Command::DeleteSpeechModel { id: model } => {
                self.ai.delete_model(&model).map_err(CoreError::Other)?;
                self.ai.publish(&self.shared);
            }
            Command::ActivateLicence { key } => self.activate_licence(&key)?,
            Command::RemoveLicence => self.remove_licence(),
            Command::CheckForUpdates => self.check_for_updates()?,
            Command::Discover { source } => self.discover(id, source)?,
            Command::Details { item } => self.details(id, item)?,
            Command::Comments { item, page } => self.comments(id, item, page)?,
            Command::SetNetworkState { online, metered } => {
                self.network = Network { online, metered };
                self.publish_downloads();
                // Auto follows the network from the next song.
                self.update_streaming_level(false);
            }
            Command::SearchMore { plugin, kind, cursor } => {
                self.sources.search_more(id, plugin, kind, cursor)?;
            }
            // M1 stub (v0.0.8 contract): the slice lists the sources with empty lists until the
            // refresh, cache and paging land in M2.
            Command::RefreshSourceLibrary { plugin } => self.refresh_source_library(plugin.as_deref())?,
            // M1 stubs (v0.0.8 contract): answered "not available yet" until M2.
            Command::SourceLibraryMore { .. } | Command::SavePlaylistCopy { .. } => {
                return Err(CoreError::Other(
                    "sources' likes and playlists are not available in this build yet".into(),
                ));
            }
            Command::Download { .. }
            | Command::CancelDownload { .. }
            | Command::RetryDownload { .. }
            | Command::RemoveDownload { .. }
            | Command::RemoveAllDownloads => {
                return Err(CoreError::Other(
                    "downloads are not available in this build yet".into(),
                ));
            }
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

    /// The player an app registered under `source`, if any.
    fn external(&self, source: &str) -> Option<External> {
        self.shared
            .external
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(source)
            .cloned()
    }

    /// Whether `source` plays elsewhere: a remote plugin or an app's player.
    fn is_remote(&self, source: &str) -> bool {
        self.external(source).is_some() || self.sources.is_remote(source)
    }

    /// The remote source whose track `start` is, if it is one.
    fn remote_plugin(&self, tracks: &[TrackRef], start: usize) -> Option<String> {
        match tracks.get(start)? {
            TrackRef::Source(track) if self.is_remote(&track.plugin) => Some(track.plugin.clone()),
            _ => None,
        }
    }

    /// Remote sources' tracks never join the queue (`mixed_queue` is theirs
    /// to decide, and the engine cannot play them anyway).
    fn refuse_remote_tracks(&self, tracks: &[TrackRef]) -> Result<(), CoreError> {
        match tracks.iter().find_map(|t| match t {
            TrackRef::Source(track) if self.is_remote(&track.plugin) => Some(track.plugin.clone()),
            _ => None,
        }) {
            Some(plugin) => Err(self.remote_queue_refusal(&plugin)),
            None => Ok(()),
        }
    }

    fn remote_queue_refusal(&self, plugin: &str) -> CoreError {
        let name = match self.external(plugin) {
            Some((name, _)) => name,
            None => self.sources.display_name(plugin),
        };
        CoreError::Other(format!(
            "{name} plays on your own devices; its songs cannot join the queue"
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
                items.push(if self.external(plugin).is_some() {
                    sources::track_item(track)
                } else {
                    self.sources.remote_track(track)?
                });
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

    // ---- streaming quality (ADR-0026) ------------------------------------

    /// The level streams are asked for: the setting, Auto resolved for the
    /// network.
    fn streaming_level(&self) -> StreamingQuality {
        crate::quality::streaming_level(self.settings.streaming_quality, self.network.metered)
    }

    /// Passes the level on to the sources: the next song is asked for at it
    /// (resolved ahead again for a gapless join). When the person changed the
    /// setting (`reopen`), the playing source song also reopens where it is.
    fn update_streaming_level(&mut self, reopen: bool) {
        if !self.sources.set_level(self.streaming_level()) {
            return;
        }
        self.rearm();
        if reopen {
            self.reopen_for_level();
        }
    }

    /// Reopens the playing source song where it is, so a new level applies
    /// at once (ADR-0026 §1); the listen goes on (no new play or history
    /// entry). A song still waiting for its stream is asked for again when
    /// that arrives; a paused one keeps its stream until the next song.
    fn reopen_for_level(&mut self) {
        if self.remote.is_some() || self.status != Status::Playing || self.pending_start.is_some() {
            return;
        }
        let Some(item) = self.shared.read_queue().current() else {
            return;
        };
        if !matches!(item.track, TrackRef::Source(_)) {
            return;
        }
        let position_ms = u64::try_from(self.position_us() / 1_000).unwrap_or(0);
        let play = self.play.take();
        self.resume_at = Some((item.uid, position_ms));
        self.reopening = Some(item.uid);
        let started = self.start_current();
        self.play = play;
        if let Err(e) = started {
            self.shared.emit(Event::Error {
                message: e.to_string(),
            });
            self.stop_playback(Status::Stopped);
        }
    }

    /// Pauses local playback and starts a session with `first`.
    fn start_remote(&mut self, plugin: &str, first: Op) -> Result<(), CoreError> {
        let (backend, source): (Box<dyn remote::Backend>, remote::Source) = match self.external(plugin) {
            Some((name, player)) => (
                Box::new(remote::ExternalBackend(player)),
                remote::Source {
                    id: plugin.to_owned(),
                    name,
                    links: jf_plugin_host::links::Links::default(),
                },
            ),
            None => {
                let (backend, source) = self.sources.remote_backend(plugin)?;
                (Box::new(backend), source)
            }
        };
        if self.status == Status::Playing {
            self.pause();
        }
        // The new session takes the slice over first, so the old one ends quietly.
        let session = remote::Session::start(backend, source, self.shared.clone(), first)?;
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
            Collection::Songs { sort, descending } => library(jf_library::TrackQuery {
                sort: *sort,
                descending: *descending,
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
    fn apply_settings(&mut self, mut settings: Settings) {
        if self.config.distribution.store().is_some() {
            // Store builds keep community plugins off (ADR-0021 §4).
            settings.allow_community_plugins = false;
        }
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
        settings.downloads.at_once = settings.downloads.at_once.clamp(1, 4);
        let downloads_changed = settings.downloads != self.settings.downloads;
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
        let updates_changed = settings.check_for_updates != self.settings.check_for_updates;
        let quality_changed = settings.streaming_quality != self.settings.streaming_quality;
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
        if quality_changed {
            self.update_streaming_level(true);
        }
        if hue_changed {
            self.publish_theme(true);
        }
        if updates_changed && let Some(update) = &self.update {
            let _ = update.send(crate::update::UpdateMsg::Auto(self.settings.check_for_updates));
        }
        if community_changed {
            self.sources
                .set_allow_community(self.settings.allow_community_plugins);
            if self.settings.allow_community_plugins {
                // Bundled community plugins kept back until now (ADR-0012).
                self.sources.install_deferred();
            }
            self.publish_sources(true);
        }
        if downloads_changed {
            self.publish_downloads();
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
            SearchScope::All => self.sources.search_plugins(),
            SearchScope::Plugin { id: plugin } => {
                self.sources.check_search(plugin)?;
                vec![(plugin.clone(), self.sources.merges(plugin))]
            }
        };
        self.sources.begin_search(id, &query);
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
                merged: true,
                groups: sources::search_groups(&answer.rows),
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
        // Reopened at a new streaming level: the same listen goes on.
        let reopened = self.reopening.take() == Some(id);

        let Some(item) = self.shared.read_queue().current() else {
            return;
        };
        if !reopened {
            // The history is part of the queue state and is saved with it.
            self.shared
                .write_queue()
                .record_played(item.track.clone(), unix_ms());
            self.queue_changed();
        }

        let mut now = match self.now_playing(&item) {
            Ok(meta) => meta,
            Err(e) => {
                self.shared.emit(Event::Error {
                    message: e.to_string(),
                });
                return;
            }
        };
        let decoded = crate::details::engine_format(&info);
        now.quality = Some(match self.sources.facts(item.uid) {
            // A source's codec and bitrate hints fill in what the decoder does not say.
            Some(facts) if matches!(item.track, TrackRef::Source(_)) => {
                crate::details::with_hints(decoded, facts.codec.as_deref(), facts.bitrate_kbps)
            }
            _ => decoded,
        });
        now.format = Some(info);
        let previous = self
            .shared
            .snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .now_playing
            .clone()
            .filter(|p| p.uid == item.uid);
        match previous {
            // The cover found for it stays (the play record, too).
            Some(previous) if reopened => now.artwork = previous.artwork.clone(),
            _ => {
                self.play = Some(PlayRecord {
                    track: item.track.clone(),
                    started_ms: unix_ms(),
                    duration_ms: now.duration_ms,
                });
            }
        }
        let changed = self.media_uid != Some(item.uid);
        let job = self.media_job(&item, &now);
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

    /// The lyrics, artwork and theme to find for a queue entry.
    fn media_job(&self, item: &QueueItem, now: &NowPlaying) -> Option<MediaJob> {
        match &item.track {
            TrackRef::Source(track) => self.sources.ctx().map(|ctx| {
                let item_record = ctx.track_item(track);
                let listed = item_record.artwork.clone();
                MediaJob::Source {
                    uid: item.uid,
                    plugin: track.plugin.clone(),
                    item: Box::new(item_record),
                    listed,
                    artwork_url: track.artwork_url.clone(),
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
        }
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
                let mut now = source_now_playing(item, track, link);
                let (qualities, note) = self.sources.offered_qualities(&track.plugin);
                now.qualities = qualities;
                now.quality_note = note;
                // The level its stream was asked for, else the one it would be asked for now.
                now.quality_level = Some(
                    self.sources
                        .facts(item.uid)
                        .map_or_else(|| self.sources.level_for(&track.plugin), |facts| facts.level),
                );
                Ok(now)
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
                    quality: None,
                    qualities: Vec::new(),
                    quality_note: Some(QualityNote::FilesPlayAsTheyAre),
                    quality_level: None,
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
                    quality: None,
                    qualities: Vec::new(),
                    quality_note: Some(QualityNote::FilesPlayAsTheyAre),
                    quality_level: None,
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

    fn on_store_entitlement(&mut self, entitlement: crate::types::StoreEntitlement) {
        tracing::debug!(?entitlement.store, entitlement.active, "store entitlement");
        if self.pro.set_entitlement(&entitlement) {
            self.pro_changed();
        }
    }

    fn on_media(&mut self, msg: MediaMsg) {
        let current = self.shared.read_queue().current_uid();
        match msg {
            // Only for the entry they were asked for, while it is still the one playing: after
            // the queue ends (and rewinds) a late answer must not bring back its lyrics or theme.
            MediaMsg::Lyrics(l) if Some(l.uid) == current && Some(l.uid) == self.media_uid => {
                let l: Arc<LyricsSnapshot> = Arc::from(l);
                self.shared.update(|s| s.lyrics = Some(l.clone()));
                self.shared.emit(Event::Lyrics(l.clone()));
                self.consider_automatic_lyrics(&l);
            }
            MediaMsg::NowPlayingArtwork { uid, path }
                if Some(uid) == current && Some(uid) == self.media_uid =>
            {
                let now = self.shared.snapshot().now_playing.clone();
                if let Some(now) = now.filter(|n| n.uid == uid) {
                    let mut now = (*now).clone();
                    now.artwork = Some(ArtworkRef::File(path));
                    self.set_now_playing(Some(now));
                }
            }
            MediaMsg::Theme(t) if t.uid == current && t.uid == self.media_uid => {
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
            SourcesMsg::Resolved { uid, asked, result } => self.on_resolved(uid, asked, result),
            SourcesMsg::Collection {
                id,
                plugin,
                then,
                result,
                more,
            } => match self.queue_collection(&plugin, then, result) {
                Ok(uids) => {
                    if more && let Some(uids) = uids {
                        self.growing = Some((id, uids));
                    }
                }
                Err(e) => self.shared.emit(Event::CommandFailed {
                    id,
                    error: e.to_string(),
                }),
            },
            SourcesMsg::CollectionMore {
                id,
                plugin,
                items,
                done,
                error,
            } => self.collection_more(id, &plugin, &items, done, error),
        }
    }

    /// A source track's stream URL arrived (or will not).
    fn on_resolved(
        &mut self,
        uid: u64,
        asked: StreamingQuality,
        result: Result<(HttpSource, crate::sources::StreamFacts), String>,
    ) {
        let current = self.shared.read_queue().current_uid();
        if asked != self.sources.level() {
            // Asked for at a level that changed meanwhile: ask again at the new one.
            self.sources.drop_resolving(uid);
            if self.pending_start != Some(uid) || current != Some(uid) {
                self.rearm();
            } else if self.status == Status::Playing
                && let Err(e) = self.start_current()
            {
                self.shared.emit(Event::Error {
                    message: e.to_string(),
                });
                self.stop_playback(Status::Stopped);
            }
            return;
        }
        let result = self.sources.resolved(uid, result);
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

    /// Queues a fetched source collection (to play, perhaps its first pages);
    /// returns its queue entries when it plays.
    fn queue_collection(
        &mut self,
        plugin: &str,
        then: Then,
        result: Result<Vec<Item>, String>,
    ) -> Result<Option<Vec<u64>>, CoreError> {
        let items = result.map_err(CoreError::Other)?;
        match then {
            Then::Play {
                start: requested,
                shuffle,
            } => {
                let (tracks, start) = sources::collection_queue(plugin, &items, requested)?;
                let uids = {
                    let mut q = self.shared.write_queue();
                    q.set_shuffle(shuffle);
                    if shuffle && requested == 0 {
                        // Shuffle with no song chosen: any song first.
                        q.set_items_from_any(tracks)
                    } else {
                        q.set_items(tracks, Some(start))?
                    }
                };
                self.resume_at = None;
                self.failures = 0;
                self.queue_changed();
                self.start_current()?;
                return Ok(Some(uids));
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
        Ok(None)
    }

    /// More of a source collection that started playing joins its queue,
    /// unless the queue was replaced meanwhile. A failed page ends it; what
    /// came stays queued.
    fn collection_more(
        &mut self,
        id: CommandId,
        plugin: &str,
        items: &[Item],
        done: bool,
        error: Option<String>,
    ) {
        let Some((request, mut uids)) = self.growing.take() else {
            return;
        };
        if request != id {
            // An earlier collection's page: the one growing now goes on.
            self.growing = Some((request, uids));
            return;
        }
        let queued = {
            let q = self.shared.read_queue();
            uids.iter().any(|uid| q.get(*uid).is_some())
        };
        if !queued {
            return; // the queue was replaced: the rest is not wanted
        }
        let tracks = sources::collection_more(plugin, items);
        if !tracks.is_empty() {
            let added = self.shared.write_queue().extend_arriving(tracks);
            uids.extend(added);
            self.queue_changed();
            self.rearm();
        }
        if let Some(error) = error {
            self.shared.emit(Event::Error {
                message: format!(
                    "Only part of the list could be loaded from {}: {error}",
                    self.sources.display_name(plugin)
                ),
            });
        }
        if !done {
            self.growing = Some((request, uids));
        }
    }

    // ---- publishing and persistence ----------------------------------

    fn publish_sources(&mut self, force: bool) {
        self.sources
            .publish(self.settings.allow_community_plugins, &self.shared, force);
        // Plugins came, went, were turned on or off, signed in or out.
        if self.ai.plugins_changed(self.pro.active(), false) {
            self.ai.publish(&self.shared);
        }
        self.refresh_look(false);
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

// ---- v0.0.8: source libraries and downloads -----------------------------

impl Actor {
    /// The folder downloads go to: the one the person chose, else the app's
    /// default, else one the core works out.
    fn downloads_folder(&self) -> PathBuf {
        self.settings
            .downloads
            .folder
            .clone()
            .or_else(|| self.config.downloads_dir.clone())
            .unwrap_or_else(|| default_downloads_dir(&self.config.data_dir))
    }

    /// Publishes the `Downloads` slice.
    ///
    /// M1 stub (v0.0.8 contract): no downloads yet; the folder and the
    /// network's effect are real.
    fn publish_downloads(&mut self) {
        self.downloads_version += 1;
        let paused = if !self.network.online {
            Some(DownloadsPaused::Offline)
        } else if self.network.metered && self.settings.downloads.wifi_only {
            Some(DownloadsPaused::Metered)
        } else {
            None
        };
        let snapshot = Arc::new(DownloadsSnapshot {
            version: self.downloads_version,
            folder: self.downloads_folder(),
            items: Vec::new(),
            bytes_used: 0,
            paused,
        });
        if self
            .shared
            .snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .downloads
            .as_ref()
            == snapshot.as_ref()
        {
            return;
        }
        self.shared.update(|s| s.downloads = snapshot.clone());
        self.shared.emit(Event::Downloads(snapshot));
    }

    /// `RefreshSourceLibrary`.
    ///
    /// M1 stub (v0.0.8 contract): lists the sources that join the slice, with
    /// no lists yet; M2 loads, caches and pages them.
    fn refresh_source_library(&mut self, plugin: Option<&str>) -> Result<(), CoreError> {
        let sources: Vec<SourceLibrary> = self
            .sources
            .library_sources()
            .into_iter()
            .map(|(plugin, name, attribution)| SourceLibrary {
                plugin,
                name,
                attribution,
                lists: Vec::new(),
            })
            .collect();
        if let Some(plugin) = plugin
            && !sources.iter().any(|s| s.plugin == plugin)
        {
            return Err(CoreError::Other(format!(
                "{} has no likes or playlists to show (it is off, signed out, or does not list them)",
                self.sources.display_name(plugin)
            )));
        }
        self.source_library_version += 1;
        let snapshot = Arc::new(SourceLibrarySnapshot {
            version: self.source_library_version,
            sources,
        });
        self.shared.update(|s| s.source_library = snapshot.clone());
        self.shared.emit(Event::SourceLibrary(snapshot));
        Ok(())
    }
}

/// A random number below `n` (0 when `n` is 0 or 1).
fn random_below(n: usize) -> usize {
    use std::hash::{BuildHasher, RandomState};
    if n < 2 {
        return 0;
    }
    let random = RandomState::new().hash_one(std::time::SystemTime::now());
    usize::try_from(random % n as u64).unwrap_or(0)
}

/// The downloads folder when the app passes none: `Jellyfesh` in
/// `$XDG_MUSIC_DIR`, else in `Music` in the home folder, else
/// `<data_dir>/Downloads`.
fn default_downloads_dir(data_dir: &Path) -> PathBuf {
    let music = std::env::var_os("XDG_MUSIC_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|home| home.join("Music"))
        });
    match music {
        Some(music) => music.join("Jellyfesh"),
        None => data_dir.join("Downloads"),
    }
}

// ---- v0.0.7: Pro, the look, AI Lyrics, updates, Discover and details ---

impl Actor {
    fn publish_pro(&mut self) {
        let snapshot = Arc::new(self.pro.snapshot());
        self.shared.update(|s| s.pro = snapshot.clone());
        self.shared.emit(Event::Pro(snapshot));
    }

    /// Pro started or stopped: the plugin host, AI Lyrics and the look follow.
    fn pro_changed(&mut self) {
        let active = self.pro.active();
        if let Some(host) = self.shared.host() {
            host.set_pro(active);
        }
        self.publish_pro();
        if self.ai.plugins_changed(active, false) {
            self.ai.publish(&self.shared);
        }
        self.refresh_look(false);
    }

    fn activate_licence(&mut self, key: &str) -> Result<(), CoreError> {
        if !self.pro.licence_keys() {
            return Err(CoreError::Other(
                "licence keys are not used in this edition of Jellyfesh: Jellyfesh Pro comes from its store"
                    .into(),
            ));
        }
        let result = self.pro.activate(key);
        if let Ok(text) = &result {
            self.save_setting(LICENCE_KEY, text);
        }
        self.pro_changed();
        result
            .map(|_| ())
            .map_err(|e| CoreError::Other(crate::pro::licence_error_text(e).into()))
    }

    fn remove_licence(&mut self) {
        self.pro.remove_licence();
        if self.config.persist_session
            && let Some(store) = &self.store
            && let Err(e) = store.remove_setting(LICENCE_KEY)
        {
            tracing::warn!("could not forget the licence key: {e}");
        }
        self.pro_changed();
    }

    fn check_for_updates(&mut self) -> Result<(), CoreError> {
        if self.config.distribution != crate::types::Distribution::Direct {
            return Err(CoreError::Other(
                "this edition of Jellyfesh is updated by its store or package manager".into(),
            ));
        }
        match &self.update {
            Some(update) => {
                let _ = update.send(crate::update::UpdateMsg::CheckNow);
                Ok(())
            }
            None => Err(CoreError::Other("the update check is not running".into())),
        }
    }

    /// Asks the appearance plugin for its look when it, its version or Pro
    /// changed (`force`: its settings changed); without one, or without Pro,
    /// the look goes back to the defaults.
    fn refresh_look(&mut self, force: bool) {
        let host = self.shared.host();
        let found = if self.pro.active() {
            host.as_ref().and_then(|host| {
                host.plugins()
                    .into_iter()
                    .find(|p| p.enabled && p.capabilities.contains(&jf_plugin_host::Capability::Appearance))
                    .map(|p| (p.id, p.version))
            })
        } else {
            None
        };
        if !force && found == self.look.plugin {
            return;
        }
        self.look.plugin.clone_from(&found);
        self.look.generation += 1;
        let (Some((plugin, _)), Some(host)) = (found, host) else {
            self.publish_look(None, None);
            return;
        };
        let (generation, tx) = (self.look.generation, self.tx.clone());
        let spawned = thread::Builder::new().name("jf-look".into()).spawn(move || {
            let result = match host.appearance_look(&plugin).recv_timeout(LOOK_WAIT) {
                Ok(Ok(look)) => Ok(look),
                Ok(Err(e)) => Err(e.to_string()),
                Err(_) => Err("the plugin did not answer in time".into()),
            };
            let _ = tx.send(Msg::Look {
                generation,
                plugin,
                result,
            });
        });
        if spawned.is_err() {
            self.publish_look(None, None);
        }
    }

    fn on_look(&mut self, generation: u64, plugin: &str, result: Result<jf_plugin_host::Look, String>) {
        if generation != self.look.generation {
            return;
        }
        match result {
            Ok(look) => self.publish_look(Some(&look), Some(plugin)),
            Err(error) => {
                tracing::warn!(%plugin, %error, "no look from the appearance plugin");
                self.publish_look(None, None);
            }
        }
    }

    fn publish_look(&mut self, look: Option<&jf_plugin_host::Look>, plugin: Option<&str>) {
        self.look.version += 1;
        let snapshot = Arc::new(crate::look::resolve(look, plugin, self.look.version));
        self.shared.update(|s| s.look = snapshot.clone());
        self.shared.emit(Event::Look(snapshot));
    }

    /// A plugin's settings changed: AI Lyrics asks for its plan again, the
    /// look for the look.
    fn plugin_settings_changed(&mut self, plugin: &str) {
        if self.ai.plugin() == Some(plugin) {
            self.ai.refresh_plan();
            self.ai.publish(&self.shared);
        }
        if self.look.plugin.as_ref().is_some_and(|(id, _)| id == plugin) {
            self.refresh_look(true);
        }
    }

    fn on_ai_lyrics(&mut self, msg: crate::ai_lyrics::AiMsg) {
        let outcome = self.ai.on_message(msg);
        self.ai.publish(&self.shared);
        self.made_lyrics_changed(outcome);
    }

    /// Made lyrics came or went for a song: when it plays, its lyrics are
    /// looked up again.
    fn made_lyrics_changed(&mut self, outcome: crate::ai_lyrics::Outcome) {
        let Some(identity) = outcome.made else {
            return;
        };
        let Some(item) = self.shared.read_queue().current() else {
            return;
        };
        if self.song_identity(&item.track).as_deref() != Some(identity.as_str()) {
            return;
        }
        let now = self.shared.snapshot().now_playing.clone();
        if let Some(now) = now.filter(|n| n.uid == item.uid)
            && let Some(job) = self.media_job(&item, &now)
        {
            self.media_uid = Some(item.uid);
            let _ = self.media_jobs.send(job);
        }
    }

    /// What identifies a song's made lyrics.
    fn song_identity(&self, track: &TrackRef) -> Option<String> {
        let path = match track {
            TrackRef::Library(id) => self.library.track(*id).ok().flatten().map(|t| t.path),
            TrackRef::File(_) | TrackRef::Source(_) => None,
        };
        crate::made::identity(track, path.as_deref())
    }

    /// A song to make lyrics for: the one playing, or a library song.
    fn lyrics_song(&self, song: &crate::types::SongRef) -> Result<crate::ai_lyrics::Song, CoreError> {
        use crate::ai_lyrics::{Song, SongAudio};
        let other = |text: &str| CoreError::Other(text.into());
        let (uid, track, text) = match song {
            crate::types::SongRef::Current => {
                if self.remote.is_some() {
                    return Err(other(
                        "AI Lyrics cannot make lyrics for a song playing on another device",
                    ));
                }
                let item = self
                    .shared
                    .read_queue()
                    .current()
                    .ok_or_else(|| other("nothing is playing"))?;
                // Plain lyrics the song has are the plugin's to time.
                let text = self.shared.snapshot().lyrics.as_ref().and_then(|l| {
                    let doc = l.doc.as_ref().filter(|_| l.uid == item.uid)?;
                    let made = matches!(l.source, Some(LyricsSource::Made { .. }));
                    (doc.timing == jf_lyrics::Timing::Unsynced && !made).then(|| {
                        doc.lines
                            .iter()
                            .map(|line| line.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                });
                (Some(item.uid), item.track.clone(), text)
            }
            crate::types::SongRef::Library { id } => (None, TrackRef::Library(*id), None),
        };
        let song = match &track {
            TrackRef::Library(id) => {
                let found = self
                    .library
                    .track(*id)?
                    .ok_or_else(|| other("that song is not in the library"))?;
                Song {
                    identity: crate::made::identity(&track, Some(&found.path)).unwrap_or_default(),
                    title: found.title,
                    artist: found.artist,
                    album: found.album,
                    duration_ms: found.duration_ms,
                    text,
                    language: None,
                    audio: SongAudio::File(found.path),
                    uid,
                    library_id: Some(*id),
                }
            }
            TrackRef::File(path) => {
                let tags = jf_library::read_file_tags(path, false).ok();
                Song {
                    identity: crate::made::identity(&track, None).unwrap_or_default(),
                    title: tags
                        .as_ref()
                        .and_then(|t| t.title.clone())
                        .unwrap_or_else(|| items::file_title(path)),
                    artist: tags.as_ref().and_then(|t| t.artist.clone()),
                    album: tags.as_ref().and_then(|t| t.album.clone()),
                    duration_ms: tags.as_ref().and_then(|t| t.duration_ms),
                    text,
                    language: None,
                    audio: SongAudio::File(path.clone()),
                    uid,
                    library_id: None,
                }
            }
            TrackRef::Source(source) => {
                let ctx = self
                    .sources
                    .ctx()
                    .ok_or_else(|| other("online sources are off"))?;
                let info = ctx
                    .host
                    .plugin(&source.plugin)
                    .ok_or_else(|| other("the song's source is not installed"))?;
                if info.playback_mode != jf_plugin_host::PlaybackMode::Stream
                    || !info
                        .capabilities
                        .contains(&jf_plugin_host::Capability::ResolveStream)
                {
                    return Err(CoreError::Other(format!(
                        "AI Lyrics cannot read the songs {} plays in its own app",
                        info.name
                    )));
                }
                Song {
                    identity: crate::made::identity(&track, None).unwrap_or_default(),
                    title: source.title.clone(),
                    artist: source.artist.clone(),
                    album: source.album.clone(),
                    duration_ms: source.duration_ms,
                    text,
                    language: None,
                    audio: SongAudio::Source {
                        plugin: source.plugin.clone(),
                        item: Box::new(ctx.track_item(source)),
                    },
                    uid,
                    library_id: None,
                }
            }
        };
        Ok(song)
    }

    /// When the AI Lyrics plugin makes lyrics by itself: a song that started
    /// without synced lyrics (and without made ones) gets a job, once.
    fn consider_automatic_lyrics(&mut self, lyrics: &LyricsSnapshot) {
        if self.auto_lyrics == Some(lyrics.uid) || !self.ai.automatic() {
            return;
        }
        let synced = lyrics
            .doc
            .as_ref()
            .is_some_and(|d| d.timing != jf_lyrics::Timing::Unsynced);
        let made = matches!(lyrics.source, Some(LyricsSource::Made { .. }));
        if synced || made {
            return;
        }
        self.auto_lyrics = Some(lyrics.uid);
        let Ok(song) = self.lyrics_song(&crate::types::SongRef::Current) else {
            return;
        };
        if self.ai.made().has(&song.identity) || self.ai.busy_with(&song.identity) {
            return;
        }
        match self.ai.make(song) {
            Ok(_) => self.ai.publish(&self.shared),
            Err(error) => tracing::debug!(%error, "no automatic lyrics"),
        }
    }

    /// Starts the restored queue where it was, when the person asked for that
    /// (Settings: resume on launch) and the session was restored.
    fn resume_on_launch(&mut self) {
        if !(self.config.restore_session && self.config.resume_on_start && self.settings.resume_on_launch)
            || self.resume_at.is_none()
        {
            return;
        }
        let Some(item) = self.shared.read_queue().current() else {
            return;
        };
        // A service's own player is not started by itself.
        if let TrackRef::Source(track) = &item.track
            && self.sources.is_remote(&track.plugin)
        {
            return;
        }
        if let Err(e) = self.play() {
            tracing::warn!("could not resume playback: {e}");
        }
    }

    fn discover(&mut self, id: CommandId, source: String) -> Result<(), CoreError> {
        if source == sources::LIBRARY {
            let page = crate::discover::library_page(&self.library, id, unix_ms());
            self.shared.emit(Event::Discover(Arc::new(page)));
            return Ok(());
        }
        let ctx = self
            .sources
            .ctx()
            .ok_or_else(|| CoreError::Other("online sources are off".into()))?;
        if ctx.host.plugin(&source).is_none() {
            return Err(CoreError::Other(format!("the source {source} is not installed")));
        }
        let shared = self.shared.clone();
        thread::Builder::new().name("jf-discover".into()).spawn(move || {
            let page = crate::discover::source_page(&ctx, &source, id);
            shared.emit(Event::Discover(Arc::new(page)));
        })?;
        Ok(())
    }

    fn details(&mut self, id: CommandId, item: crate::types::ItemRef) -> Result<(), CoreError> {
        use crate::details::{LocalSong, SourceRequest};
        use crate::types::{DetailsPage, Fact, FactKey, ItemRef};
        let not_found = || CoreError::Other("that is not in the library".into());
        let large = |hash: Option<&str>| {
            hash.and_then(|h| self.library.artwork_thumbnail(h, 1024).ok().flatten())
                .map(ArtworkRef::File)
        };
        match &item {
            ItemRef::Track { id: track } => {
                let found = self.library.track(*track)?.ok_or_else(not_found)?;
                let image = large(found.artwork_hash.as_deref()).and_then(|a| match a {
                    ArtworkRef::File(path) => Some(path),
                    ArtworkRef::Url(_) => None,
                });
                crate::details::local_song(
                    self.shared.clone(),
                    id,
                    LocalSong {
                        item: item.clone(),
                        path: found.path,
                        title: Some(found.title),
                        artist: found.artist,
                        image,
                    },
                )?;
            }
            ItemRef::File { path } => crate::details::local_song(
                self.shared.clone(),
                id,
                LocalSong {
                    item: item.clone(),
                    path: path.clone(),
                    title: None,
                    artist: None,
                    image: None,
                },
            )?,
            ItemRef::Album { id: album } => {
                let found = self.library.album(*album)?.ok_or_else(not_found)?;
                let tracks = self.library.album_tracks(*album)?;
                let genre = tracks.iter().find_map(|t| t.genre.clone());
                let mut facts = Vec::new();
                if let Some(year) = found.year {
                    facts.push(Fact {
                        key: FactKey::Released,
                        value: year.to_string(),
                    });
                }
                if let Some(genre) = genre {
                    facts.push(Fact {
                        key: FactKey::Genre,
                        value: genre,
                    });
                }
                let page = DetailsPage {
                    request: id,
                    item: item.clone(),
                    title: found.title,
                    subtitle: found.album_artist,
                    description: None,
                    image: large(found.artwork_hash.as_deref()),
                    facts,
                    technical: None,
                    comment_count: None,
                    attribution: None,
                    error: None,
                };
                self.shared.emit(Event::Details(Arc::new(page)));
            }
            ItemRef::Artist { id: artist } => {
                let found = self.library.artist(*artist)?.ok_or_else(not_found)?;
                let page = DetailsPage {
                    request: id,
                    item: item.clone(),
                    title: found.name,
                    subtitle: None,
                    description: None,
                    image: None,
                    facts: Vec::new(),
                    technical: None,
                    comment_count: None,
                    attribution: None,
                    error: None,
                };
                self.shared.emit(Event::Details(Arc::new(page)));
            }
            ItemRef::Source {
                plugin,
                item_kind,
                id: item_id,
            } => {
                let ctx = self
                    .sources
                    .ctx()
                    .ok_or_else(|| CoreError::Other("online sources are off".into()))?;
                let host_item = ctx.item(plugin, item_id, crate::remote::host_kind(*item_kind));
                // The song playing: what the engine decoded.
                let now = self.shared.snapshot().now_playing.clone();
                let technical = now.as_ref().and_then(|now| match &now.track {
                    TrackRef::Source(t) if &t.plugin == plugin && &t.id == item_id => {
                        now.quality.clone().map(|format| crate::types::TechnicalInfo {
                            format,
                            container: None,
                            file_size_bytes: None,
                            duration_ms: now.duration_ms,
                            replaygain: None,
                            path: None,
                        })
                    }
                    _ => None,
                });
                let attribution = ctx.host.plugin(plugin).and_then(|i| i.content_policy.attribution);
                crate::details::source_details(
                    self.shared.clone(),
                    ctx.host.clone(),
                    id,
                    SourceRequest {
                        item: item.clone(),
                        plugin: plugin.clone(),
                        host_item,
                        attribution,
                        technical,
                    },
                )?;
            }
        }
        Ok(())
    }

    fn comments(
        &mut self,
        id: CommandId,
        item: crate::types::ItemRef,
        page: Option<String>,
    ) -> Result<(), CoreError> {
        let crate::types::ItemRef::Source {
            plugin,
            item_kind,
            id: item_id,
        } = &item
        else {
            let page = crate::details::no_comments(id, item);
            self.shared.emit(Event::Comments(Arc::new(page)));
            return Ok(());
        };
        let ctx = self
            .sources
            .ctx()
            .ok_or_else(|| CoreError::Other("online sources are off".into()))?;
        let host_item = ctx.item(plugin, item_id, crate::remote::host_kind(*item_kind));
        let attribution = ctx.host.plugin(plugin).and_then(|i| i.content_policy.attribution);
        crate::details::source_comments(
            self.shared.clone(),
            ctx.host.clone(),
            id,
            item.clone(),
            plugin.clone(),
            host_item,
            page,
            attribution,
        )?;
        Ok(())
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
        artwork: items::stored_artwork(track),
        format: None,
        link,
        quality: None,
        qualities: Vec::new(),
        quality_note: None,
        quality_level: None,
    }
}

pub(crate) fn unix_ms() -> i64 {
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
    made: crate::made::MadeLyrics,
    artwork_dir: Option<PathBuf>,
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
                    let (mut doc, mut source) = match found {
                        Some((doc, source)) => (Some(doc), Some(source)),
                        None => (None, None),
                    };
                    // No synced lyrics of its own: the ones AI Lyrics made, if any.
                    if doc
                        .as_ref()
                        .is_none_or(|d| d.timing == jf_lyrics::Timing::Unsynced)
                        && let Some((made_doc, meta)) = made.get(&crate::made::file_identity(&audio))
                    {
                        doc = Some(made_doc);
                        source = Some(LyricsSource::Made {
                            plugin: meta.plugin,
                            engine: meta.engine,
                        });
                    }
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
                    artwork_url,
                } => {
                    let found = sources.as_ref().and_then(|ctx| ctx.lyrics(&plugin, &item));
                    // Made lyrics show synced only where the source's content policy allows it.
                    let sync_allowed = sources
                        .as_ref()
                        .and_then(|ctx| ctx.host.plugin(&plugin))
                        .is_none_or(|info| info.content_policy.lyrics_sync);
                    let own_synced = found
                        .as_ref()
                        .is_some_and(|(doc, synced)| *synced && doc.timing != jf_lyrics::Timing::Unsynced);
                    let made_lyrics = (sync_allowed && !own_synced)
                        .then(|| made.get(&crate::made::source_identity(&plugin, &item.id)))
                        .flatten();
                    let lyrics = match (found, made_lyrics) {
                        (_, Some((doc, meta))) => LyricsSnapshot {
                            uid,
                            timeline: Some(doc.timeline()),
                            doc: Some(doc),
                            source: Some(LyricsSource::Made {
                                plugin: meta.plugin,
                                engine: meta.engine,
                            }),
                        },
                        (Some((doc, synced)), None) => LyricsSnapshot {
                            uid,
                            // Unsynced (content policy): no timeline to follow.
                            timeline: synced.then(|| doc.timeline()),
                            doc: Some(doc),
                            source: Some(LyricsSource::Plugin(plugin.clone())),
                        },
                        (None, None) => LyricsSnapshot {
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
                    let bytes = sources
                        .as_ref()
                        .and_then(|ctx| ctx.artwork(&plugin, &item, listed));
                    // Now Playing shows the picture from the cache, fetched with the plugin's headers,
                    // under the name lists give the song, so Recently Played finds it there.
                    if let (Some(bytes), Some(dir)) = (&bytes, &artwork_dir) {
                        let key = crate::artwork::stored_picture(
                            &plugin,
                            jf_plugin_host::ItemKind::Track,
                            &item.id,
                            artwork_url.as_deref(),
                        )
                        .unwrap_or_else(|| format!("jellyfesh-track:{plugin}:{}", item.id));
                        match crate::artwork::store_now_playing(dir, &key, bytes) {
                            Ok(path) => {
                                let _ = tx.send(Msg::Media(MediaMsg::NowPlayingArtwork { uid, path }));
                            }
                            Err(error) => tracing::info!(%error, "could not keep the Now Playing picture"),
                        }
                    }
                    bytes
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
