//! Remote sessions (ADR-0018): Jellyfesh as the remote control of a
//! service's own player.
//!
//! Playing a track of a remote source starts a session: a worker thread
//! that sends the service the person's commands, asks it what it is doing
//! (every second while it plays, every five while it is paused, every
//! fifteen while nothing plays, less often after errors) and publishes the
//! `Remote` slice. The actor routes the transport commands to the session
//! while it lives. A session ends when the actor drops it (local playback
//! started, [`crate::RemoteAction::End`], another session started), after
//! ten minutes without playing, or when the service wants a new sign-in.
//!
//! What the worker talks to is a [`Backend`]: a plugin's `remote` export
//! here, a player an app provides through the FFI later (ADR-0018 item 5).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use jf_engine::{PlaybackAnchor, monotonic_now_ns};
use jf_plugin_host::links::Links;
use jf_plugin_host::{
    CallError, Item, PluginError, PluginHost, RemoteDevice as HostDevice, RemoteDeviceKind as HostKind,
    RemoteRepeat, RemoteState as HostState,
};
use jf_session::RepeatMode;

use crate::actor::Shared;
use crate::sources::source_item;
use crate::types::{
    Event, ExternalItem, ExternalPlayer, ExternalState, ItemKind, RemoteDevice, RemoteDeviceKind,
    RemoteSnapshot, Status,
};

/// Longest wait for one call to the service.
const CALL_WAIT: Duration = Duration::from_secs(15);
/// How often the state is asked for while the service plays.
const PLAYING_POLL: Duration = Duration::from_secs(1);
/// … while it is paused.
const PAUSED_POLL: Duration = Duration::from_secs(5);
/// … while nothing plays anywhere.
const IDLE_POLL: Duration = Duration::from_secs(15);
/// The state is asked for this soon after a command (services apply them
/// with a little delay).
const AFTER_COMMAND: Duration = Duration::from_millis(600);
/// The longest wait after repeated errors.
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A session ends after this long without playing.
const IDLE_END: Duration = Duration::from_secs(600);
/// A reported position this close to the predicted one keeps the anchor,
/// so the slice is not published every second for nothing.
const DRIFT_MS: i64 = 1_500;
/// Previous restarts the track after this much of it, like the engine's.
const RESTART_MS: i64 = 3_000;

/// Why a call to the service failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteError {
    pub message: String,
    /// The service wants the person to sign in again: the session ends.
    pub sign_in: bool,
}

impl From<CallError> for RemoteError {
    fn from(error: CallError) -> Self {
        RemoteError {
            sign_in: matches!(error, CallError::Plugin(PluginError::AuthRequired)),
            message: error.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, RemoteError>;

/// What a session controls: the shape of the WIT `remote` interface.
pub(crate) trait Backend: Send + 'static {
    fn devices(&self) -> Result<Vec<HostDevice>>;
    fn state(&self) -> Result<Option<HostState>>;
    fn play(
        &self,
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()>;
    fn resume(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn next(&self) -> Result<()>;
    fn previous(&self) -> Result<()>;
    fn seek(&self, position_ms: u32) -> Result<()>;
    fn set_volume(&self, percent: u32) -> Result<()>;
    fn set_shuffle(&self, on: bool) -> Result<()>;
    fn set_repeat(&self, mode: RemoteRepeat) -> Result<()>;
    fn transfer(&self, device: String, play: bool) -> Result<()>;
}

/// A plugin's `remote` export, through the plugin host.
pub(crate) struct PluginBackend {
    pub host: PluginHost,
    pub plugin: String,
}

impl PluginBackend {
    fn wait<T>(&self, reply: &jf_plugin_host::Reply<T>) -> Result<T> {
        match reply.recv_timeout(CALL_WAIT) {
            Ok(result) => result.map_err(RemoteError::from),
            Err(_) => Err(RemoteError {
                message: "the service did not answer in time".into(),
                sign_in: false,
            }),
        }
    }
}

impl Backend for PluginBackend {
    fn devices(&self) -> Result<Vec<HostDevice>> {
        self.wait(&self.host.remote_devices(&self.plugin))
    }
    fn state(&self) -> Result<Option<HostState>> {
        self.wait(&self.host.remote_state(&self.plugin))
    }
    fn play(
        &self,
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()> {
        self.wait(
            &self
                .host
                .remote_play(&self.plugin, device, context, tracks, start),
        )
    }
    fn resume(&self) -> Result<()> {
        self.wait(&self.host.remote_resume(&self.plugin))
    }
    fn pause(&self) -> Result<()> {
        self.wait(&self.host.remote_pause(&self.plugin))
    }
    fn next(&self) -> Result<()> {
        self.wait(&self.host.remote_next(&self.plugin))
    }
    fn previous(&self) -> Result<()> {
        self.wait(&self.host.remote_previous(&self.plugin))
    }
    fn seek(&self, position_ms: u32) -> Result<()> {
        self.wait(&self.host.remote_seek(&self.plugin, position_ms))
    }
    fn set_volume(&self, percent: u32) -> Result<()> {
        self.wait(&self.host.remote_set_volume(&self.plugin, percent))
    }
    fn set_shuffle(&self, on: bool) -> Result<()> {
        self.wait(&self.host.remote_set_shuffle(&self.plugin, on))
    }
    fn set_repeat(&self, mode: RemoteRepeat) -> Result<()> {
        self.wait(&self.host.remote_set_repeat(&self.plugin, mode))
    }
    fn transfer(&self, device: String, play: bool) -> Result<()> {
        self.wait(&self.host.remote_transfer(&self.plugin, device, play))
    }
}

/// A player an app registered ([`ExternalPlayer`]).
pub(crate) struct ExternalBackend(pub Arc<dyn ExternalPlayer>);

fn external_error(message: String) -> RemoteError {
    RemoteError {
        message,
        sign_in: false,
    }
}

impl Backend for ExternalBackend {
    fn devices(&self) -> Result<Vec<HostDevice>> {
        let devices = self.0.devices().map_err(external_error)?;
        Ok(jf_plugin_host::sanitize::remote_devices(
            devices.into_iter().map(host_device).collect(),
        ))
    }
    fn state(&self) -> Result<Option<HostState>> {
        let state = self.0.state().map_err(external_error)?;
        Ok(state.map(host_state))
    }
    fn play(
        &self,
        _device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()> {
        self.0
            .play(
                context.as_ref().map(external_item),
                tracks.iter().map(external_item).collect(),
                start,
            )
            .map_err(external_error)
    }
    fn resume(&self) -> Result<()> {
        self.0.resume().map_err(external_error)
    }
    fn pause(&self) -> Result<()> {
        self.0.pause().map_err(external_error)
    }
    fn next(&self) -> Result<()> {
        self.0.next().map_err(external_error)
    }
    fn previous(&self) -> Result<()> {
        self.0.previous().map_err(external_error)
    }
    fn seek(&self, position_ms: u32) -> Result<()> {
        self.0.seek(u64::from(position_ms)).map_err(external_error)
    }
    fn set_volume(&self, percent: u32) -> Result<()> {
        self.0.set_volume(percent).map_err(external_error)
    }
    fn set_shuffle(&self, on: bool) -> Result<()> {
        self.0.set_shuffle(on).map_err(external_error)
    }
    fn set_repeat(&self, mode: RemoteRepeat) -> Result<()> {
        self.0.set_repeat(repeat_from_host(mode)).map_err(external_error)
    }
    fn transfer(&self, device: String, _play: bool) -> Result<()> {
        self.0.transfer(device).map_err(external_error)
    }
}

pub(crate) fn host_kind(kind: ItemKind) -> jf_plugin_host::ItemKind {
    match kind {
        ItemKind::Track => jf_plugin_host::ItemKind::Track,
        ItemKind::Album => jf_plugin_host::ItemKind::Album,
        ItemKind::Artist => jf_plugin_host::ItemKind::Artist,
        ItemKind::Playlist => jf_plugin_host::ItemKind::Playlist,
        ItemKind::Folder => jf_plugin_host::ItemKind::Folder,
    }
}

fn external_item(item: &Item) -> ExternalItem {
    ExternalItem {
        kind: crate::sources::item_kind(item.kind),
        id: item.id.clone(),
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms.map(u64::from),
        artwork_url: item.artwork.as_ref().map(|a| a.url.clone()),
    }
}

fn host_item(item: ExternalItem) -> Item {
    let mut out = crate::sources::bare_item(&item.id, host_kind(item.kind));
    out.title = item.title;
    out.artist = item.artist;
    out.album = item.album;
    out.duration_ms = item.duration_ms.map(|d| u32::try_from(d).unwrap_or(u32::MAX));
    out.artwork = item.artwork_url.map(|url| jf_plugin_host::ImageRef {
        url,
        headers: Vec::new(),
        width: None,
        height: None,
    });
    out
}

fn host_device(d: RemoteDevice) -> HostDevice {
    HostDevice {
        id: d.id,
        name: d.name,
        kind: match d.kind {
            RemoteDeviceKind::Computer => HostKind::Computer,
            RemoteDeviceKind::Smartphone => HostKind::Smartphone,
            RemoteDeviceKind::Speaker => HostKind::Speaker,
            RemoteDeviceKind::Tv => HostKind::Tv,
            RemoteDeviceKind::Car => HostKind::Car,
            RemoteDeviceKind::Other => HostKind::Other,
        },
        active: d.active,
        supports_volume: d.volume_percent.is_some(),
        volume_percent: d.volume_percent,
    }
}

fn host_state(s: ExternalState) -> HostState {
    // The app's answer is checked like a plugin's (titles, ids, positions).
    let allow_none = |_: &url::Url| true;
    let checker = jf_plugin_host::sanitize::Checker::new("external", &allow_none);
    jf_plugin_host::sanitize::remote_state(
        HostState {
            device: s.device.map(host_device),
            item: s.item.map(host_item),
            context: s.context.map(host_item),
            playing: s.playing,
            position_ms: u32::try_from(s.position_ms).unwrap_or(u32::MAX),
            shuffle: s.shuffle,
            repeat: repeat_to_host(s.repeat),
        },
        &checker,
    )
}

/// What the actor asks of a session.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Op {
    /// Tracks from `start`, or the container `context` from its track
    /// `start`; shuffle set first when given.
    Play {
        context: Option<Box<Item>>,
        tracks: Vec<Item>,
        start: u32,
        shuffle: Option<bool>,
    },
    Resume,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek(u64),
    Shuffle(bool),
    Repeat(RepeatMode),
    Volume(u32),
    Transfer(String),
    Refresh,
    /// Pause the service's player, then end the session.
    Stop,
}

/// The actor's handle on a session. Dropping it ends the session.
pub(crate) struct Session {
    pub(crate) source: String,
    id: u64,
    tx: Sender<Op>,
    alive: Arc<AtomicBool>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("source", &self.source)
            .field("id", &self.id)
            .field("alive", &self.is_alive())
            .finish_non_exhaustive()
    }
}

/// Who the slice is about: the source, its name and links.
#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub id: String,
    pub name: String,
    pub links: Links,
}

impl Session {
    /// Starts a session on its own thread; `first` is usually [`Op::Play`].
    /// From now on only this session's updates reach the `Remote` slice.
    pub(crate) fn start(
        backend: Box<dyn Backend>,
        source: Source,
        shared: Arc<Shared>,
        first: Op,
    ) -> std::io::Result<Session> {
        let id = shared.remote_session.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = unbounded();
        let alive = Arc::new(AtomicBool::new(true));
        let worker = Worker::new(backend, source.clone(), shared, id, alive.clone());
        std::thread::Builder::new()
            .name("jf-remote".into())
            .spawn(move || worker.run(&rx, first))?;
        Ok(Session {
            source: source.id,
            id,
            tx,
            alive,
        })
    }

    /// Whether the worker still runs (it ends by itself when idle or
    /// signed out).
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Hands the worker a command; false when it has ended.
    pub(crate) fn send(&self, op: Op) -> bool {
        self.is_alive() && self.tx.send(op).is_ok()
    }
}

/// Ends whatever session runs and publishes the empty slice, unless a
/// newer session took over in between.
pub(crate) fn end(shared: &Shared, session: Option<Session>) {
    let Some(session) = session else {
        return;
    };
    let id = session.id;
    drop(session);
    // A session that already ended said so itself.
    let current = shared.remote_session.load(Ordering::SeqCst);
    if current == id {
        publish(shared, id, |version| ended(version, None));
    }
}

/// The slice without a session.
pub(crate) fn ended(version: u64, error: Option<String>) -> RemoteSnapshot {
    RemoteSnapshot {
        version,
        source: None,
        source_name: None,
        status: Status::Stopped,
        item: None,
        context: None,
        anchor: PlaybackAnchor {
            track_generation: 0,
            position_us: 0,
            anchor_mono_ns: monotonic_now_ns(),
            rate: 0.0,
            duration_us: None,
        },
        shuffle: false,
        repeat: RepeatMode::Off,
        device: None,
        devices: Vec::new(),
        error,
        quality_note: None,
    }
}

/// Replaces the slice and tells subscribers, if `session` is still the
/// current one (decided under the subscribers' lock, so a stale update can
/// never overtake a newer one).
fn publish(shared: &Shared, session: u64, build: impl FnOnce(u64) -> RemoteSnapshot) -> bool {
    let mut subscribers = shared.subscribers.lock().unwrap_or_else(|e| e.into_inner());
    if shared.remote_session.load(Ordering::SeqCst) != session {
        return false;
    }
    let mut snapshot = shared.snapshot.write().unwrap_or_else(|e| e.into_inner());
    let slice = Arc::new(build(snapshot.remote.version + 1));
    snapshot.remote = slice.clone();
    drop(snapshot);
    let event = Event::Remote(slice);
    subscribers.retain(|s| s.send(event.clone()).is_ok());
    true
}

struct Worker {
    backend: Box<dyn Backend>,
    source: Source,
    shared: Arc<Shared>,
    id: u64,
    alive: Arc<AtomicBool>,
    model: Model,
}

/// What the worker knows about the service's player.
#[derive(Debug, Clone, PartialEq)]
struct Model {
    state: Option<HostState>,
    /// When `state.position_ms` was true, on the monotonic clock.
    measured_ns: u64,
    /// Changes with the track and on seeks.
    generation: u64,
    devices: Vec<HostDevice>,
    error: Option<String>,
    /// Errors in a row.
    errors: u32,
}

impl Model {
    fn new() -> Model {
        Model {
            state: None,
            measured_ns: monotonic_now_ns(),
            generation: 1,
            devices: Vec::new(),
            error: None,
            errors: 0,
        }
    }

    fn playing(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.playing)
    }

    /// The position now, extrapolated from the last report.
    fn position_ms(&self, now_ns: u64) -> i64 {
        let Some(state) = &self.state else {
            return 0;
        };
        let mut position = i64::from(state.position_ms);
        if state.playing {
            let elapsed =
                i64::try_from(now_ns.saturating_sub(self.measured_ns) / 1_000_000).unwrap_or(i64::MAX);
            position = position.saturating_add(elapsed);
        }
        match state.item.as_ref().and_then(|i| i.duration_ms) {
            Some(length) if length > 0 => position.min(i64::from(length)),
            _ => position,
        }
    }

    /// Takes in what the service reported at `now_ns`. Returns whether
    /// anything changed beyond the position moving on as predicted.
    fn report(&mut self, reported: Option<HostState>, now_ns: u64) -> bool {
        let predicted = self.position_ms(now_ns);
        let same_track = self.state.as_ref().and_then(|s| s.item.as_ref()).map(|i| &i.id)
            == reported.as_ref().and_then(|s| s.item.as_ref()).map(|i| &i.id);
        let on_course = match (&self.state, &reported) {
            (Some(old), Some(new)) => {
                same_track
                    && old.playing == new.playing
                    && old.shuffle == new.shuffle
                    && old.repeat == new.repeat
                    && old.device == new.device
                    && old.context == new.context
                    && (i64::from(new.position_ms) - predicted).abs() <= DRIFT_MS
            }
            (None, None) => true,
            _ => false,
        };
        if on_course {
            // Keep the anchor; take in what the position does not affect.
            if let (Some(old), Some(new)) = (&mut self.state, reported) {
                old.item = new.item;
            }
            return false;
        }
        if !same_track {
            self.generation += 1;
        }
        if let Some(device) = reported.as_ref().and_then(|s| s.device.as_ref()) {
            // The playing device is the active one in the list too.
            for d in &mut self.devices {
                d.active = d.id == device.id;
                if d.id == device.id {
                    d.volume_percent = device.volume_percent;
                }
            }
        }
        self.state = reported;
        self.measured_ns = now_ns;
        true
    }

    /// After a command the service accepted: what it will do, until the
    /// next report says otherwise.
    fn assume(&mut self, op: &Op, now_ns: u64) {
        let position = u32::try_from(self.position_ms(now_ns).max(0)).unwrap_or(u32::MAX);
        match op {
            Op::Play {
                tracks,
                start,
                shuffle,
                context,
            } => {
                self.generation += 1;
                let item = tracks.get(*start as usize).cloned();
                let state = self.state.get_or_insert(HostState {
                    device: None,
                    item: None,
                    context: None,
                    playing: true,
                    position_ms: 0,
                    shuffle: false,
                    repeat: RemoteRepeat::Off,
                });
                state.item = item;
                state.context = context.as_deref().cloned();
                state.playing = true;
                state.position_ms = 0;
                if let Some(on) = shuffle {
                    state.shuffle = *on;
                }
            }
            Op::Resume | Op::Pause | Op::Toggle => {
                if let Some(state) = &mut self.state {
                    state.playing = match op {
                        Op::Resume => true,
                        Op::Pause => false,
                        _ => !state.playing,
                    };
                    state.position_ms = position;
                }
            }
            Op::Seek(to) => {
                self.generation += 1;
                if let Some(state) = &mut self.state {
                    state.position_ms = u32::try_from(*to).unwrap_or(u32::MAX);
                }
            }
            Op::Next | Op::Previous => {
                self.generation += 1;
                if let Some(state) = &mut self.state {
                    state.position_ms = 0;
                }
            }
            Op::Shuffle(on) => {
                if let Some(state) = &mut self.state {
                    state.shuffle = *on;
                }
            }
            Op::Repeat(mode) => {
                if let Some(state) = &mut self.state {
                    state.repeat = repeat_to_host(*mode);
                }
            }
            Op::Volume(percent) => {
                let active = self
                    .state
                    .as_ref()
                    .and_then(|s| s.device.as_ref())
                    .map(|d| d.id.clone());
                if let Some(device) = self.state.as_mut().and_then(|s| s.device.as_mut()) {
                    device.volume_percent = Some(*percent);
                }
                for d in &mut self.devices {
                    if Some(&d.id) == active.as_ref() || (active.is_none() && d.active) {
                        d.volume_percent = Some(*percent);
                    }
                }
            }
            Op::Transfer(device) => {
                for d in &mut self.devices {
                    d.active = d.id == *device;
                }
                let picked = self.devices.iter().find(|d| d.id == *device).cloned();
                if let Some(state) = &mut self.state {
                    state.device = picked.or(state.device.take());
                    state.playing = true;
                    state.position_ms = position;
                }
            }
            Op::Refresh | Op::Stop => {}
        }
        self.measured_ns = now_ns;
    }

    /// How long to wait before asking again.
    fn interval(&self) -> Duration {
        let base = match &self.state {
            Some(state) if state.playing => PLAYING_POLL,
            Some(_) => PAUSED_POLL,
            None => IDLE_POLL,
        };
        if self.errors == 0 {
            return base;
        }
        let factor = 1u32 << self.errors.min(6);
        (base.max(Duration::from_secs(2)) * factor).min(MAX_BACKOFF)
    }

    fn failed(&mut self, error: &RemoteError) {
        self.errors = self.errors.saturating_add(1);
        self.error = Some(error.message.clone());
    }

    fn succeeded(&mut self) {
        self.errors = 0;
        self.error = None;
    }
}

impl Worker {
    fn new(
        backend: Box<dyn Backend>,
        source: Source,
        shared: Arc<Shared>,
        id: u64,
        alive: Arc<AtomicBool>,
    ) -> Self {
        Worker {
            backend,
            source,
            shared,
            id,
            alive,
            model: Model::new(),
        }
    }

    fn run(mut self, rx: &Receiver<Op>, first: Op) {
        let mut idle_since = Instant::now();
        let mut ending: Option<String> = None;
        self.refresh_devices();
        let mut wait = match self.apply(first) {
            Ok(()) => AFTER_COMMAND,
            Err(error) if error.sign_in => {
                ending = Some(self.sign_in_text());
                Duration::ZERO
            }
            Err(_) => AFTER_COMMAND,
        };
        self.publish();
        while ending.is_none() {
            match rx.recv_timeout(wait) {
                Ok(Op::Stop) => {
                    if let Err(error) = self.backend.pause() {
                        tracing::debug!(source = %self.source.id, error = %error.message, "remote pause on stop");
                    }
                    break;
                }
                Ok(op) => {
                    if let Err(error) = self.apply(op)
                        && error.sign_in
                    {
                        ending = Some(self.sign_in_text());
                        break;
                    }
                    self.publish();
                    wait = AFTER_COMMAND;
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            match self.backend.state() {
                Ok(state) => {
                    let errors = self.model.errors;
                    self.model.succeeded();
                    if self.model.report(state, monotonic_now_ns()) || errors > 0 {
                        self.publish();
                    }
                }
                Err(error) if error.sign_in => {
                    ending = Some(self.sign_in_text());
                    break;
                }
                Err(error) => {
                    let first = self.model.error.is_none();
                    self.model.failed(&error);
                    if first {
                        self.publish();
                    }
                }
            }
            if self.model.playing() {
                idle_since = Instant::now();
            } else if idle_since.elapsed() >= IDLE_END {
                break;
            }
            wait = self.model.interval();
        }
        self.alive.store(false, Ordering::SeqCst);
        publish(&self.shared, self.id, |version| ended(version, ending));
    }

    fn sign_in_text(&self) -> String {
        format!("Sign in to {} again.", self.source.name)
    }

    fn refresh_devices(&mut self) {
        match self.backend.devices() {
            Ok(devices) => self.model.devices = devices,
            Err(error) => self.model.failed(&error),
        }
    }

    /// Sends one command; on success the model assumes its effect.
    fn apply(&mut self, op: Op) -> Result<()> {
        let now = monotonic_now_ns();
        let result = match &op {
            Op::Play {
                context,
                tracks,
                start,
                shuffle,
            } => {
                if let Some(on) = shuffle
                    && let Err(error) = self.backend.set_shuffle(*on)
                {
                    tracing::debug!(source = %self.source.id, error = %error.message, "remote shuffle before play");
                }
                self.play_on_a_device(context.as_deref().cloned(), tracks.clone(), *start)
            }
            Op::Resume => self.backend.resume(),
            Op::Pause => self.backend.pause(),
            Op::Toggle if self.model.playing() => self.backend.pause(),
            Op::Toggle => self.backend.resume(),
            Op::Next => self.backend.next(),
            Op::Previous if self.model.position_ms(now) > RESTART_MS => self.backend.seek(0),
            Op::Previous => self.backend.previous(),
            Op::Seek(to) => self.backend.seek(u32::try_from(*to).unwrap_or(u32::MAX)),
            Op::Shuffle(on) => self.backend.set_shuffle(*on),
            Op::Repeat(mode) => self.backend.set_repeat(repeat_to_host(*mode)),
            Op::Volume(percent) => self.backend.set_volume((*percent).min(100)),
            Op::Transfer(device) => self.backend.transfer(device.clone(), true),
            Op::Refresh => {
                self.refresh_devices();
                self.backend.state().map(|state| {
                    self.model.report(state, monotonic_now_ns());
                })
            }
            Op::Stop => Ok(()),
        };
        match result {
            Ok(()) => {
                self.model.succeeded();
                let op = match op {
                    // A Previous that restarted the track is a seek to 0.
                    Op::Previous if self.model.position_ms(now) > RESTART_MS => Op::Seek(0),
                    op => op,
                };
                if op != Op::Refresh {
                    self.model.assume(&op, now);
                }
                if let Op::Transfer(device) = &op {
                    remember_device(&self.shared, &self.source.id, device);
                    self.refresh_devices();
                }
                Ok(())
            }
            Err(error) => {
                self.model.failed(&error);
                Err(error)
            }
        }
    }

    fn publish(&self) {
        let model = &self.model;
        let source = &self.source;
        publish(&self.shared, self.id, |version| snapshot(version, source, model));
    }

    /// The device to play on: the active one, else the one chosen last (in
    /// the device picker, or played on) while it is listed, else the only
    /// one; `None` (several idle devices, none chosen) leaves it to the
    /// service.
    fn play_device(&self) -> Option<String> {
        let devices = &self.model.devices;
        if let Some(active) = devices.iter().find(|d| d.active) {
            return Some(active.id.clone());
        }
        let chosen = chosen_device(&self.shared, &self.source.id);
        if let Some(chosen) = chosen.filter(|id| devices.iter().any(|d| &d.id == id)) {
            return Some(chosen);
        }
        match devices.as_slice() {
            [only] => Some(only.id.clone()),
            _ => None,
        }
    }

    /// Plays on [`Worker::play_device`], so a song started in Jellyfesh goes
    /// to the device chosen there even when no device is active (Spotify
    /// answers NO_ACTIVE_DEVICE to a play for no device then). When an idle
    /// device does not take the play, playback moves to it first and plays
    /// there. With several idle devices and none chosen, the service's answer
    /// stands: it asks the person to choose one, rather than music starting
    /// on a device they did not pick.
    fn play_on_a_device(&mut self, context: Option<Item>, tracks: Vec<Item>, start: u32) -> Result<()> {
        if self.model.devices.is_empty() {
            self.refresh_devices();
        }
        let idle = !self.model.devices.iter().any(|d| d.active);
        let device = self.play_device();
        match self
            .backend
            .play(device.clone(), context.clone(), tracks.clone(), start)
        {
            Ok(()) => {
                if let Some(device) = &device {
                    remember_device(&self.shared, &self.source.id, device);
                }
                Ok(())
            }
            Err(error) if error.sign_in || !idle => Err(error),
            Err(error) => {
                let Some(target) = device else {
                    return Err(error);
                };
                tracing::debug!(source = %self.source.id, error = %error.message, %target, "moving playback to the device, then playing");
                // A device that cannot be moved to keeps the play's own answer.
                self.backend.transfer(target.clone(), false).map_err(|_| error)?;
                self.backend.play(Some(target.clone()), context, tracks, start)?;
                remember_device(&self.shared, &self.source.id, &target);
                self.refresh_devices();
                Ok(())
            }
        }
    }
}

/// Remembers the device a source last played on (chosen in the device
/// picker, or played on), for the next session with no active device.
fn remember_device(shared: &Shared, source: &str, device: &str) {
    shared
        .remote_devices
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(source.to_owned(), device.to_owned());
}

/// The device a source last played on in this run.
fn chosen_device(shared: &Shared, source: &str) -> Option<String> {
    shared
        .remote_devices
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(source)
        .cloned()
}

/// The slice for `model`.
fn snapshot(version: u64, source: &Source, model: &Model) -> RemoteSnapshot {
    let state = model.state.as_ref();
    let item = state.and_then(|s| s.item.as_ref());
    RemoteSnapshot {
        version,
        source: Some(source.id.clone()),
        source_name: Some(source.name.clone()),
        status: match state {
            Some(s) if s.playing => Status::Playing,
            Some(_) => Status::Paused,
            None => Status::Stopped,
        },
        item: item.map(|i| source_item(&source.id, i, &source.links)),
        context: state
            .and_then(|s| s.context.as_ref())
            .map(|c| source_item(&source.id, c, &source.links)),
        anchor: PlaybackAnchor {
            track_generation: model.generation,
            position_us: state.map_or(0, |s| i64::from(s.position_ms) * 1_000),
            anchor_mono_ns: model.measured_ns,
            rate: if model.playing() { 1.0 } else { 0.0 },
            duration_us: item.and_then(|i| i.duration_ms).map(|d| i64::from(d) * 1_000),
        },
        shuffle: state.is_some_and(|s| s.shuffle),
        repeat: state.map_or(RepeatMode::Off, |s| repeat_from_host(s.repeat)),
        device: state.and_then(|s| s.device.clone()).map(device),
        devices: model.devices.iter().cloned().map(device).collect(),
        error: model.error.clone(),
        quality_note: Some(crate::types::QualityNote::SetInService {
            service: source.name.clone(),
        }),
    }
}

fn device(d: HostDevice) -> RemoteDevice {
    RemoteDevice {
        id: d.id,
        name: d.name,
        kind: match d.kind {
            HostKind::Computer => RemoteDeviceKind::Computer,
            HostKind::Smartphone => RemoteDeviceKind::Smartphone,
            HostKind::Speaker => RemoteDeviceKind::Speaker,
            HostKind::Tv => RemoteDeviceKind::Tv,
            HostKind::Car => RemoteDeviceKind::Car,
            HostKind::Other => RemoteDeviceKind::Other,
        },
        active: d.active,
        volume_percent: d.volume_percent.filter(|_| d.supports_volume),
    }
}

fn repeat_to_host(mode: RepeatMode) -> RemoteRepeat {
    match mode {
        RepeatMode::Off => RemoteRepeat::Off,
        RepeatMode::All => RemoteRepeat::All,
        RepeatMode::One => RemoteRepeat::One,
    }
}

fn repeat_from_host(mode: RemoteRepeat) -> RepeatMode {
    match mode {
        RemoteRepeat::Off => RepeatMode::Off,
        RemoteRepeat::All => RepeatMode::All,
        RemoteRepeat::One => RepeatMode::One,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_host::ItemKind;

    fn track(id: &str, length: u32) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: format!("Sample {id}"),
            artist: None,
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(length),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    fn state(id: &str, playing: bool, position_ms: u32) -> HostState {
        HostState {
            device: None,
            item: Some(track(id, 100_000)),
            context: None,
            playing,
            position_ms,
            shuffle: false,
            repeat: RemoteRepeat::Off,
        }
    }

    const SECOND: u64 = 1_000_000_000;

    #[test]
    fn a_report_on_course_keeps_the_anchor() {
        let mut model = Model::new();
        assert!(model.report(Some(state("a", true, 10_000)), 100 * SECOND));
        let generation = model.generation;
        // One second later the service says 11.2 s: as predicted.
        assert!(!model.report(Some(state("a", true, 11_200)), 101 * SECOND));
        assert_eq!(model.measured_ns, 100 * SECOND, "the anchor stays");
        assert_eq!(model.position_ms(102 * SECOND), 12_000);
        // A jump (someone seeked on the phone) moves it.
        assert!(model.report(Some(state("a", true, 50_000)), 103 * SECOND));
        assert_eq!(model.position_ms(103 * SECOND), 50_000);
        assert_eq!(model.generation, generation, "same track");
        // Another track is a new generation.
        assert!(model.report(Some(state("b", true, 0)), 104 * SECOND));
        assert_eq!(model.generation, generation + 1);
        // Nothing playing anywhere.
        assert!(model.report(None, 105 * SECOND));
        assert!(!model.report(None, 106 * SECOND));
    }

    #[test]
    fn the_position_stops_at_the_end_of_the_track() {
        let mut model = Model::new();
        model.report(Some(state("a", true, 99_000)), 10 * SECOND);
        assert_eq!(model.position_ms(20 * SECOND), 100_000);
        model.report(Some(state("a", false, 40_000)), 30 * SECOND);
        assert_eq!(model.position_ms(90 * SECOND), 40_000, "paused stays put");
    }

    #[test]
    fn commands_are_assumed_until_the_next_report() {
        let mut model = Model::new();
        model.devices = vec![
            HostDevice {
                id: "desk".into(),
                name: "Desk".into(),
                kind: HostKind::Speaker,
                active: true,
                supports_volume: true,
                volume_percent: Some(30),
            },
            HostDevice {
                id: "phone".into(),
                name: "Phone".into(),
                kind: HostKind::Smartphone,
                active: false,
                supports_volume: false,
                volume_percent: None,
            },
        ];
        model.assume(
            &Op::Play {
                context: None,
                tracks: vec![track("a", 1000), track("b", 1000)],
                start: 1,
                shuffle: Some(true),
            },
            5 * SECOND,
        );
        let s = model.state.clone().unwrap();
        assert_eq!(s.item.map(|i| i.id).as_deref(), Some("b"));
        assert!(s.playing && s.shuffle);

        model.assume(&Op::Toggle, 6 * SECOND);
        assert!(!model.playing());
        assert_eq!(
            model.state.as_ref().unwrap().position_ms,
            1000,
            "paused where it was"
        );
        model.assume(&Op::Volume(70), 7 * SECOND);
        assert_eq!(model.devices[0].volume_percent, Some(70));
        model.assume(&Op::Transfer("phone".into()), 8 * SECOND);
        assert!(!model.devices[0].active && model.devices[1].active);
        assert_eq!(
            model
                .state
                .as_ref()
                .unwrap()
                .device
                .as_ref()
                .map(|d| d.name.as_str()),
            Some("Phone")
        );
        assert!(model.playing());
        model.assume(&Op::Repeat(RepeatMode::One), 9 * SECOND);
        assert_eq!(model.state.as_ref().unwrap().repeat, RemoteRepeat::One);
    }

    #[test]
    fn polling_slows_down_when_paused_idle_or_failing() {
        let mut model = Model::new();
        assert_eq!(model.interval(), IDLE_POLL);
        model.report(Some(state("a", true, 0)), SECOND);
        assert_eq!(model.interval(), PLAYING_POLL);
        model.report(Some(state("a", false, 0)), 2 * SECOND);
        assert_eq!(model.interval(), PAUSED_POLL);
        let error = RemoteError {
            message: "the service asked to slow down".into(),
            sign_in: false,
        };
        model.failed(&error);
        assert_eq!(model.interval(), Duration::from_secs(10));
        for _ in 0..10 {
            model.failed(&error);
        }
        assert_eq!(model.interval(), MAX_BACKOFF);
        model.succeeded();
        assert_eq!(model.interval(), PAUSED_POLL);
        assert_eq!(model.error, None);
    }

    fn speaker(id: &str, active: bool) -> HostDevice {
        HostDevice {
            id: id.into(),
            name: id.into(),
            kind: HostKind::Speaker,
            active,
            supports_volume: true,
            volume_percent: None,
        }
    }

    /// A service with Spotify's rule: a play for no device fails while no
    /// device is active, and a `sleepy` idle device takes a play only once
    /// playback moved to it. It notes what it was asked.
    struct Fake {
        devices: Vec<HostDevice>,
        sleepy: bool,
        moved_to: std::sync::Mutex<Option<String>>,
        calls: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Fake {
        fn note(&self, call: String) {
            self.calls.lock().unwrap().push(call);
        }
    }

    fn no_device() -> RemoteError {
        RemoteError {
            message: "No device is active.".into(),
            sign_in: false,
        }
    }

    impl Backend for Fake {
        fn devices(&self) -> Result<Vec<HostDevice>> {
            Ok(self.devices.clone())
        }
        fn state(&self) -> Result<Option<HostState>> {
            Ok(None)
        }
        fn play(&self, device: Option<String>, _: Option<Item>, _: Vec<Item>, start: u32) -> Result<()> {
            self.note(format!("play {} {start}", device.as_deref().unwrap_or("-")));
            let moved = self.moved_to.lock().unwrap().clone();
            let active = self.devices.iter().any(|d| d.active) || moved.is_some();
            match device {
                None if !active => Err(no_device()),
                Some(device) if self.sleepy && !active && moved.as_deref() != Some(device.as_str()) => {
                    Err(no_device())
                }
                _ => Ok(()),
            }
        }
        fn resume(&self) -> Result<()> {
            Ok(())
        }
        fn pause(&self) -> Result<()> {
            Ok(())
        }
        fn next(&self) -> Result<()> {
            Ok(())
        }
        fn previous(&self) -> Result<()> {
            Ok(())
        }
        fn seek(&self, _: u32) -> Result<()> {
            Ok(())
        }
        fn set_volume(&self, _: u32) -> Result<()> {
            Ok(())
        }
        fn set_shuffle(&self, _: bool) -> Result<()> {
            Ok(())
        }
        fn set_repeat(&self, _: RemoteRepeat) -> Result<()> {
            Ok(())
        }
        fn transfer(&self, device: String, _: bool) -> Result<()> {
            self.note(format!("transfer {device}"));
            *self.moved_to.lock().unwrap() = Some(device);
            Ok(())
        }
    }

    /// A worker for `devices`, with the calls it makes.
    fn worker(
        shared: &Arc<Shared>,
        devices: Vec<HostDevice>,
        sleepy: bool,
    ) -> (Worker, Arc<std::sync::Mutex<Vec<String>>>) {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let fake = Fake {
            devices,
            sleepy,
            moved_to: std::sync::Mutex::new(None),
            calls: calls.clone(),
        };
        let source = Source {
            id: "org.example.remote".into(),
            name: "Example".into(),
            links: Links::default(),
        };
        let worker = Worker::new(
            Box::new(fake),
            source,
            shared.clone(),
            1,
            Arc::new(AtomicBool::new(true)),
        );
        (worker, calls)
    }

    fn play_op() -> Op {
        Op::Play {
            context: None,
            tracks: vec![track("a", 1000)],
            start: 0,
            shuffle: None,
        }
    }

    #[test]
    fn songs_play_on_the_device_chosen_when_none_is_active() {
        let shared = Arc::new(Shared::new(jf_session::Queue::new()));
        // The only device, idle: it is asked by name.
        let (mut w, calls) = worker(&shared, vec![speaker("desk", false)], false);
        w.apply(play_op()).unwrap();
        assert_eq!(*calls.lock().unwrap(), ["play desk 0"]);

        // Several idle devices and none chosen: the service's answer stands.
        let (mut w, calls) = worker(
            &shared,
            vec![speaker("tv", false), speaker("phone", false)],
            false,
        );
        assert_eq!(w.apply(play_op()), Err(no_device()));
        assert_eq!(
            *calls.lock().unwrap(),
            ["play - 0"],
            "music never starts on a device not picked"
        );

        // Choosing one in the picker sends later songs there.
        w.apply(Op::Transfer("phone".into())).unwrap();
        let (mut w, calls) = worker(
            &shared,
            vec![speaker("tv", false), speaker("phone", false)],
            false,
        );
        w.apply(play_op()).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["play phone 0"],
            "remembered across sessions"
        );

        // A device that sleeps through the play is moved to first.
        let (mut w, calls) = worker(&shared, vec![speaker("tv", false), speaker("phone", false)], true);
        w.apply(play_op()).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["play phone 0", "transfer phone", "play phone 0"]
        );

        // The active device wins over the one chosen.
        let (mut w, calls) = worker(&shared, vec![speaker("tv", true), speaker("phone", false)], true);
        w.apply(play_op()).unwrap();
        assert_eq!(*calls.lock().unwrap(), ["play tv 0"]);
        assert_eq!(
            chosen_device(&shared, "org.example.remote").as_deref(),
            Some("tv")
        );
    }

    #[test]
    fn devices_map_without_volume_when_it_cannot_change() {
        let d = device(HostDevice {
            id: "tv".into(),
            name: "Living Room TV".into(),
            kind: HostKind::Tv,
            active: false,
            supports_volume: false,
            volume_percent: Some(20),
        });
        assert_eq!(d.kind, RemoteDeviceKind::Tv);
        assert_eq!(d.volume_percent, None);
        assert_eq!(repeat_from_host(repeat_to_host(RepeatMode::All)), RepeatMode::All);
    }
}
