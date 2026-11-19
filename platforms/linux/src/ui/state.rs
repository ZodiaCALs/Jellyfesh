//! What the app mirrors from the core, and who is told when it changes.
//!
//! The app state is a plain copy of the core's slices, replaced whole when
//! an event brings a new version (a UI never edits a mirror on its own,
//! ADR-0001). Views register listeners; bursts of events are applied
//! together and each changed slice is announced once.
//!
//! While the `Remote` slice has a source (ADR-0018), the player shows the
//! remote session instead of the engine: [`App::is_playing`], the drawn
//! position and the shuffle and repeat states follow the session, and the
//! transport commands go to the core as usual (it routes them).

use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{
    AiLyricsSnapshot, AuthUpdate, BrowseUpdate, Command, CommandId, DevicesSnapshot, DisplayClock, Event,
    LookSnapshot, LyricsSnapshot, NowPlaying,
    PlaybackSnapshot, ProSnapshot, QueueSnapshot, RemoteSnapshot, RepeatMode, ScanStatus, SearchUpdate,
    Settings, Snapshot, SourcesSnapshot, Status, ThemeSnapshot, UpdateSnapshot, monotonic_now_ns,
};

use crate::logic::remote::{self, RemoteView};
use crate::ui::bridge::Bridge;
use crate::ui::clock::Scheduler;
use crate::ui::thumbs::Thumbs;

/// What changed, for listeners.
#[derive(Debug)]
pub enum Change<'a> {
    Playback,
    Queue,
    NowPlaying,
    Lyrics,
    Theme,
    Settings,
    Devices,
    Sources,
    /// The remote session changed (its track, state, device or devices).
    Remote,
    Library,
    Muted,
    Scan(&'a ScanStatus),
    Search(&'a Arc<SearchUpdate>),
    Browse(&'a Arc<BrowseUpdate>),
    Auth(&'a AuthUpdate),
    Buffering,
    /// A playlist was made by the command with this id.
    PlaylistCreated(CommandId, i64),
    Failed(CommandId, &'a str),
    Error,
    /// The AI Lyrics slice changed.
    AiLyrics,
    /// The look changed.
    Look,
    /// Jellyfesh Pro changed.
    Pro,
    /// The update check changed.
    Update,
}

type Listener = Rc<dyn Fn(&Change)>;

/// The app's mirror of the core and its shared services.
pub struct App {
    bridge: RefCell<Option<Bridge>>,
    pub thumbs: Thumbs,
    pub scheduler: Scheduler,
    /// The engine's clock, fed with the playback slice's anchors.
    pub clock: RefCell<DisplayClock>,
    /// The remote session's clock, fed with the `Remote` slice's anchors.
    pub remote_clock: RefCell<DisplayClock>,
    pub playback: RefCell<PlaybackSnapshot>,
    pub queue: RefCell<QueueSnapshot>,
    pub now: RefCell<Option<Arc<NowPlaying>>>,
    pub lyrics: RefCell<Option<Arc<LyricsSnapshot>>>,
    pub theme: RefCell<Option<Arc<ThemeSnapshot>>>,
    pub settings: RefCell<Arc<Settings>>,
    pub devices: RefCell<Arc<DevicesSnapshot>>,
    pub sources: RefCell<Arc<SourcesSnapshot>>,
    pub remote: RefCell<Arc<RemoteSnapshot>>,
    pub ai_lyrics: RefCell<Arc<AiLyricsSnapshot>>,
    pub look: RefCell<Arc<LookSnapshot>>,
    pub pro: RefCell<Arc<ProSnapshot>>,
    pub update: RefCell<Arc<UpdateSnapshot>>,
    pub muted: Cell<bool>,
    pub library_revision: Cell<u64>,
    pub scanning: Cell<bool>,
    /// How the last scan ended (finished or failed) in this run, for
    /// Settings › Library's Last Scan.
    pub last_scan: RefCell<Option<ScanStatus>>,
    pub buffering: Cell<bool>,
    /// The main window, for toasts and dialogs.
    pub window: RefCell<glib::WeakRef<adw::ApplicationWindow>>,
    pub toasts: RefCell<glib::WeakRef<adw::ToastOverlay>>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_listener: Cell<u64>,
    /// Commands whose failure a view reports itself (no toast).
    quiet: RefCell<Vec<CommandId>>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The running app (main thread only).
pub fn app() -> Rc<App> {
    APP.with(|a| a.borrow().clone())
        .expect("the app state is created at startup")
}

/// The running app, or `None` before startup and after shutdown.
pub fn try_app() -> Option<Rc<App>> {
    APP.with(|a| a.borrow().clone())
}

#[derive(Default)]
struct Dirty {
    playback: bool,
    queue: bool,
    now: bool,
    lyrics: bool,
    theme: bool,
    settings: bool,
    devices: bool,
    sources: bool,
    remote: bool,
    library: bool,
    muted: bool,
    ai_lyrics: bool,
    look: bool,
    pro: bool,
    update: bool,
}

impl App {
    /// Creates the app state from the core's snapshot and installs it.
    pub fn install(bridge: Bridge, thumbs: Thumbs, snapshot: Snapshot) -> Rc<App> {
        let mut clock = DisplayClock::new();
        clock.update(snapshot.playback.anchor, monotonic_now_ns());
        let mut remote_clock = DisplayClock::new();
        remote_clock.update(snapshot.remote.anchor, monotonic_now_ns());
        let app = Rc::new(App {
            bridge: RefCell::new(Some(bridge)),
            thumbs,
            scheduler: Scheduler::default(),
            clock: RefCell::new(clock),
            remote_clock: RefCell::new(remote_clock),
            playback: RefCell::new(snapshot.playback),
            queue: RefCell::new(snapshot.queue),
            now: RefCell::new(snapshot.now_playing),
            lyrics: RefCell::new(snapshot.lyrics),
            theme: RefCell::new(snapshot.theme),
            settings: RefCell::new(snapshot.settings),
            devices: RefCell::new(snapshot.devices),
            sources: RefCell::new(snapshot.sources),
            remote: RefCell::new(snapshot.remote),
            ai_lyrics: RefCell::new(snapshot.ai_lyrics),
            look: RefCell::new(snapshot.look),
            pro: RefCell::new(snapshot.pro),
            update: RefCell::new(snapshot.update),
            muted: Cell::new(snapshot.muted),
            library_revision: Cell::new(snapshot.library_revision),
            scanning: Cell::new(snapshot.scanning),
            last_scan: RefCell::new(None),
            buffering: Cell::new(false),
            window: RefCell::new(glib::WeakRef::new()),
            toasts: RefCell::new(glib::WeakRef::new()),
            listeners: RefCell::new(Vec::new()),
            next_listener: Cell::new(1),
            quiet: RefCell::new(Vec::new()),
        });
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));
        app
    }

    /// Removes the app state and returns the bridge for shutdown.
    pub fn uninstall() -> Option<Bridge> {
        let app = APP.with(|a| a.borrow_mut().take())?;
        app.listeners.borrow_mut().clear();
        app.thumbs.stop();
        app.bridge.borrow_mut().take()
    }

    /// Sends a command to the core.
    pub fn send(&self, command: Command) -> CommandId {
        match self.bridge.borrow().as_ref() {
            Some(b) => b.send(command),
            None => CommandId(0),
        }
    }

    /// Sends a command whose failure the caller shows itself.
    pub fn send_quiet(&self, command: Command) -> CommandId {
        let id = self.send(command);
        self.quiet.borrow_mut().push(id);
        id
    }

    /// Runs a query on the query thread; `None` once the app is shutting down.
    pub async fn query<R: Send + 'static>(
        &self,
        f: impl FnOnce(&jf_core::Core) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = async_channel::bounded(1);
        {
            let bridge = self.bridge.borrow();
            bridge.as_ref()?.run(move |core| {
                let _ = tx.send_blocking(f(core));
            });
        }
        rx.recv().await.ok()
    }

    /// Calls `f` on every change until the returned id is removed.
    pub fn listen(&self, f: impl Fn(&Change) + 'static) -> u64 {
        let id = self.next_listener.get();
        self.next_listener.set(id + 1);
        self.listeners.borrow_mut().push((id, Rc::new(f)));
        id
    }

    pub fn unlisten(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    /// Calls `f` on every change while `widget` exists.
    pub fn listen_for(&self, widget: &impl IsA<gtk::Widget>, f: impl Fn(&Change) + 'static) {
        let id = self.listen(f);
        widget.as_ref().connect_destroy(move |_| {
            if let Some(app) = try_app() {
                app.unlisten(id);
            }
        });
    }

    fn emit(&self, change: &Change) {
        let listeners: Vec<Listener> = self.listeners.borrow().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l(change);
        }
    }

    /// Applies a burst of events, then announces each changed slice once.
    pub fn apply_events(&self, events: impl IntoIterator<Item = Event>) {
        let mut dirty = Dirty::default();
        for event in events {
            self.apply(event, &mut dirty);
        }
        let Dirty {
            playback,
            queue,
            now,
            lyrics,
            theme,
            settings,
            devices,
            sources,
            remote,
            library,
            muted,
            ai_lyrics,
            look,
            pro,
            update,
        } = dirty;
        if settings {
            self.emit(&Change::Settings);
        }
        if pro {
            self.emit(&Change::Pro);
        }
        if look {
            self.emit(&Change::Look);
        }
        if theme {
            self.emit(&Change::Theme);
        }
        if now {
            self.emit(&Change::NowPlaying);
        }
        if lyrics {
            self.emit(&Change::Lyrics);
        }
        if queue {
            self.emit(&Change::Queue);
        }
        if playback {
            self.emit(&Change::Playback);
        }
        if muted {
            self.emit(&Change::Muted);
        }
        if devices {
            self.emit(&Change::Devices);
        }
        if sources {
            self.emit(&Change::Sources);
        }
        if remote {
            self.emit(&Change::Remote);
        }
        if library {
            self.emit(&Change::Library);
        }
        if ai_lyrics {
            self.emit(&Change::AiLyrics);
        }
        if update {
            self.emit(&Change::Update);
        }
        if playback || now || lyrics || remote {
            self.scheduler.refresh();
        }
    }

    fn apply(&self, event: Event, dirty: &mut Dirty) {
        match event {
            Event::Playback(p) => {
                self.clock.borrow_mut().update(p.anchor, monotonic_now_ns());
                *self.playback.borrow_mut() = p;
                dirty.playback = true;
            }
            Event::Queue(q) => {
                *self.queue.borrow_mut() = q;
                dirty.queue = true;
            }
            Event::NowPlaying(n) => {
                *self.now.borrow_mut() = n;
                dirty.now = true;
            }
            Event::Lyrics(l) => {
                *self.lyrics.borrow_mut() = Some(l);
                dirty.lyrics = true;
            }
            Event::Theme(t) => {
                *self.theme.borrow_mut() = Some(t);
                dirty.theme = true;
            }
            Event::Settings(s) => {
                *self.settings.borrow_mut() = s;
                dirty.settings = true;
            }
            Event::Devices(d) => {
                *self.devices.borrow_mut() = d;
                dirty.devices = true;
            }
            Event::Sources(s) => {
                *self.sources.borrow_mut() = s;
                dirty.sources = true;
            }
            Event::Remote(r) => {
                self.remote_clock
                    .borrow_mut()
                    .update(r.anchor, monotonic_now_ns());
                let before = self.remote.replace(r.clone());
                // A session that ends with an error says so once.
                if let Some(message) = remote::ended_error(&before, &r) {
                    self.toast(&message);
                }
                dirty.remote = true;
            }
            Event::LibraryChanged { revision } => {
                self.library_revision.set(revision);
                dirty.library = true;
            }
            Event::Scan(status) => {
                self.scanning.set(matches!(status, ScanStatus::Progress(_)));
                if !matches!(status, ScanStatus::Progress(_)) {
                    *self.last_scan.borrow_mut() = Some(status.clone());
                }
                self.emit(&Change::Scan(&status));
            }
            Event::Auth(update) => self.emit(&Change::Auth(&update)),
            Event::SearchResults(update) => self.emit(&Change::Search(&update)),
            Event::BrowsePage(update) => self.emit(&Change::Browse(&update)),
            Event::PlaylistCreated { request, id } => self.emit(&Change::PlaylistCreated(request, id)),
            Event::Buffering { buffering, .. } => {
                self.buffering.set(buffering);
                self.emit(&Change::Buffering);
            }
            Event::CommandFailed { id, error } => {
                let quiet = {
                    let mut q = self.quiet.borrow_mut();
                    let found = q.iter().position(|x| *x == id);
                    found.map(|i| q.remove(i)).is_some()
                };
                self.emit(&Change::Failed(id, &error));
                if !quiet {
                    self.toast(&error);
                }
            }
            Event::Error { message } => {
                self.emit(&Change::Error);
                self.toast(&message);
            }
            Event::AiLyrics(a) => {
                *self.ai_lyrics.borrow_mut() = a;
                dirty.ai_lyrics = true;
            }
            Event::Look(l) => {
                *self.look.borrow_mut() = l;
                dirty.look = true;
            }
            Event::Pro(p) => {
                *self.pro.borrow_mut() = p;
                dirty.pro = true;
            }
            Event::Update(u) => {
                *self.update.borrow_mut() = u;
                dirty.update = true;
            }
            // Discover, details and comments: not shown yet.
            Event::Discover(_) | Event::Details(_) | Event::Comments(_) => {}
            // A source's picture is in the cache: the views waiting load it.
            Event::Artwork(ready) => self.thumbs.artwork_ready(&ready),
        }
        // Muted is part of the snapshot; it only changes with SetMuted, which
        // republishes the playback slice.
        if dirty.playback {
            let muted = self.bridge.borrow().as_ref().map(|b| b.core().snapshot().muted);
            if let Some(m) = muted
                && m != self.muted.get()
            {
                self.muted.set(m);
                dirty.muted = true;
            }
        }
    }

    /// Shows a short message at the bottom of the window.
    pub fn toast(&self, message: &str) {
        if let Some(overlay) = self.toasts.borrow().upgrade() {
            let toast = adw::Toast::new(message);
            toast.set_timeout(4);
            overlay.add_toast(toast);
        } else {
            tracing::warn!("{message}");
        }
    }

    /// The transport state shown: the remote session's while one runs,
    /// otherwise the engine's.
    pub fn status(&self) -> Status {
        let remote = self.remote.borrow();
        if remote.source.is_some() {
            remote.status
        } else {
            self.playback.borrow().status
        }
    }

    pub fn is_playing(&self) -> bool {
        self.status() == Status::Playing
    }

    /// Whether a remote session runs (the `Remote` slice has a source).
    pub fn is_remote(&self) -> bool {
        self.remote.borrow().source.is_some()
    }

    /// The remote session as the player shows it, while one runs.
    pub fn remote_view(&self) -> Option<RemoteView> {
        remote::view(&self.remote.borrow())
    }

    /// The clock the player draws from: the remote session's while one
    /// runs, otherwise the engine's.
    pub fn display_clock(&self) -> RefMut<'_, DisplayClock> {
        if self.is_remote() {
            self.remote_clock.borrow_mut()
        } else {
            self.clock.borrow_mut()
        }
    }

    /// The track shown as playing: the remote session's while one runs,
    /// otherwise the engine's (rows mark it).
    pub fn shown_track(&self) -> Option<jf_core::TrackRef> {
        let remote = self.remote.borrow();
        if remote.source.is_some() {
            remote.item.as_ref().and_then(|i| i.track.clone())
        } else {
            self.now.borrow().as_ref().map(|n| n.track.clone())
        }
    }

    /// The shuffle state shown (the remote session's while one runs).
    pub fn shown_shuffle(&self) -> bool {
        let remote = self.remote.borrow();
        if remote.source.is_some() {
            remote.shuffle
        } else {
            self.queue.borrow().shuffle
        }
    }

    /// The repeat mode shown (the remote session's while one runs).
    pub fn shown_repeat(&self) -> RepeatMode {
        let remote = self.remote.borrow();
        if remote.source.is_some() {
            remote.repeat
        } else {
            self.queue.borrow().repeat
        }
    }

    /// The name of a source (its plugin's name), for "Open in {Service}".
    pub fn source_name(&self, id: &str) -> String {
        self.sources
            .borrow()
            .plugins
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_owned(), |p| p.name.clone())
    }

    /// Whether a source plays on the person's own devices (ADR-0018).
    pub fn is_remote_source(&self, id: &str) -> bool {
        self.sources
            .borrow()
            .plugins
            .iter()
            .any(|p| p.id == id && p.remote_playback)
    }

    /// Duration of the shown track in µs, from the anchor or the metadata.
    pub fn duration_us(&self) -> Option<i64> {
        let from_anchor = self.display_clock().anchor().and_then(|a| a.duration_us);
        let from_metadata = || {
            let ms = match self.remote_view() {
                Some(view) => view.duration_ms,
                None => self.now.borrow().as_ref()?.duration_ms,
            };
            ms.map(|d| i64::try_from(d).unwrap_or(0) * 1_000)
        };
        from_anchor.filter(|d| *d > 0).or_else(from_metadata)
    }

    /// The drawn playback position now, in µs.
    pub fn position_us(&self) -> i64 {
        self.display_clock().position_us(monotonic_now_ns())
    }

    /// Changes one setting: sends back a modified copy of the slice.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut settings = (**self.settings.borrow()).clone();
        f(&mut settings);
        if settings != **self.settings.borrow() {
            self.send(Command::SetSettings { settings });
        }
    }
}

/// Spawns a future on the main loop.
pub fn spawn<F: std::future::Future<Output = ()> + 'static>(f: F) {
    glib::spawn_future_local(f);
}
