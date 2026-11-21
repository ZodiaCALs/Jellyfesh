//! Now Playing as a stage (Luminous, docs/ui.md): the backdrop behind
//! everything, the artwork (380 px, `shadow-3`) and titles on the left,
//! lyrics, the queue or track info on the right (one column when the window
//! is narrow), and every control in one floating Veil dock: shuffle,
//! previous, play, next, repeat, the seek bar with its times, the volume,
//! the device button and More (a rounded card in narrow windows). The
//! backdrop is the core's baked artwork backdrop, crossfaded on track change
//! (style Artwork), or the Misty field at the artwork hue (style Misty). The
//! artwork scales to 0.88 while paused and springs back when playing; play
//! and pause morph into each other.
//!
//! While a remote session runs (ADR-0018) the stage shows its track with
//! "Playing on {Device}" (and its error in a quieter line); the dock's
//! controls send the usual commands, the device button opens Devices, More
//! holds Open in {Service} and Stop Controlling, and the Lyrics and Queue
//! panes say that the service has them.

mod info;
mod lyrics;
mod queue;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{BackdropStyle, Command, ThemeSnapshot};

use crate::logic::format;
use crate::logic::lyrics as lyrics_logic;
use crate::logic::motion::{self, artwork_plan, artwork_scale};
use crate::logic::remote::{self, RemoteView};
use crate::logic::seek::Times;
use crate::ui::actions;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::outputs::DeviceButton;
use crate::ui::state::{Change, app, spawn, try_app};
use crate::ui::style::{self, FLUID};
use crate::ui::thumbs::ArtKey;
use crate::ui::transport::{self, Volume, set_on};
use crate::ui::widgets::art::Art;
use crate::ui::widgets::fade::FadeBin;
use crate::ui::widgets::glyph::{self, Glyph};
use crate::ui::widgets::motion::MotionBin;
use crate::ui::widgets::playpause::PlayPause;
use crate::ui::widgets::seekbar::SeekBar;
use crate::ui::window;

/// The right-hand panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Lyrics,
    Queue,
    Info,
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::Lyrics => "lyrics",
            Tab::Queue => "queue",
            Tab::Info => "info",
        }
    }
}

/// The queue entry (uid) whose album More knows, and that album.
type AlbumOf = Option<(u64, Option<i64>)>;

struct Inner {
    root: adw::BreakpointBin,
    backdrop: Backdrop,
    art: Art,
    /// The artwork and its frame, scaled while paused.
    art_bin: MotionBin,
    title: gtk::Label,
    subtitle: gtk::Label,
    /// "Playing on {Device}" while a remote session runs, with the
    /// device's glyph.
    device_line: gtk::Box,
    device_glyph: Glyph,
    device_label: gtk::Label,
    /// The remote session's last error, in a quieter line.
    error_line: gtk::Label,
    heart: gtk::Button,
    favorite: Cell<bool>,
    seek: SeekBar,
    play: gtk::Button,
    play_glyph: PlayPause,
    shuffle: gtk::Button,
    repeat: gtk::Button,
    volume: Volume,
    /// The dock and its parts, laid out in a row (wide) or a card (narrow).
    dock: gtk::Box,
    dock_transport: gtk::Box,
    dock_extras: gtk::Box,
    /// Kept for as long as the view, for its listener.
    _devices: Rc<DeviceButton>,
    more: gtk::MenuButton,
    /// What More's menu was built for, so an open menu is not replaced by
    /// an identical one (the remote session reports every second).
    more_key: RefCell<Option<String>>,
    /// The album of the library song playing, for Go to Album in More.
    album_of: Rc<Cell<AlbumOf>>,
    tabs: Vec<(Tab, gtk::ToggleButton)>,
    tab_guard: Cell<bool>,
    panel: gtk::Stack,
    player: gtk::Box,
    narrow: Cell<bool>,
    /// The view's size, for fitting the artwork.
    view_size: Cell<(i32, i32)>,
    tab: Cell<Tab>,
    show_panel_narrow: Cell<bool>,
    lyrics: Rc<lyrics::Lyrics>,
    queue: Rc<queue::Queue>,
    info: info::Info,
}

/// The Now Playing view.
#[derive(Clone)]
pub struct NowPlaying(Rc<Inner>);

impl std::fmt::Debug for NowPlaying {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NowPlaying").finish_non_exhaustive()
    }
}

impl NowPlaying {
    pub fn new() -> NowPlaying {
        let backdrop = Backdrop::new();

        // Top: back to the Library and the panel switcher, each a glass
        // capsule floating on the stage.
        let back = gtk::Button::builder().build();
        let back_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        back_box.append(&Glyph::new("back", 20));
        let back_label = gtk::Label::new(Some(&tr("Library")));
        back_box.append(&back_label);
        back.set_child(Some(&back_box));
        back.add_css_class("np-back");
        back.add_css_class("np-capsule");
        back.set_tooltip_text(Some(&tr("Back to Library")));
        back.update_property(&[gtk::accessible::Property::Label(&tr("Back to Library"))]);
        back.connect_clicked(|_| window::close_now_playing());
        let segmented = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segmented.add_css_class("segmented");
        segmented.add_css_class("np-capsule");
        segmented.update_property(&[gtk::accessible::Property::Label(&tr("Now Playing views"))]);
        let mut tabs = Vec::new();
        for (tab, label) in [
            (Tab::Lyrics, tr("Lyrics")),
            (Tab::Queue, tr("Queue")),
            (Tab::Info, tr("Info")),
        ] {
            let b = gtk::ToggleButton::with_label(&label);
            b.add_css_class("segment");
            segmented.append(&b);
            tabs.push((tab, b));
        }
        let topbar = gtk::CenterBox::new();
        topbar.add_css_class("np-topbar");
        topbar.set_start_widget(Some(&back));
        topbar.set_center_widget(Some(&segmented));

        // The player column: the artwork, then the titles.
        let art = Art::new(ART_MAX, jf_tokens::artwork::RADIUS_HERO);
        art.add_css_class("np-art");
        art.set_halign(gtk::Align::Center);
        art.set_fixed(true);
        let art_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        art_frame.add_css_class("np-art-frame");
        // The Misty glass frame hugs the artwork.
        art_frame.set_halign(gtk::Align::Center);
        art_frame.set_valign(gtk::Align::Center);
        art_frame.append(&art);
        // Paused, the artwork (with its frame) rests at 0.88 of its size.
        let art_bin = MotionBin::new(&art_frame);
        art_bin.add_css_class("np-art-bin");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        title.add_css_class("np-title");
        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        subtitle.add_css_class("np-subtitle");
        let device_glyph = Glyph::new("speaker", 16);
        let device_label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let device_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        device_line.add_css_class("np-device-line");
        device_line.append(&device_glyph);
        device_line.append(&device_label);
        device_line.set_visible(false);
        let error_line = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .visible(false)
            .build();
        error_line.add_css_class("np-error-line");
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        titles.set_hexpand(true);
        titles.append(&title);
        titles.append(&subtitle);
        titles.append(&device_line);
        titles.append(&error_line);
        let heart = glyph::button("heart", 22, &tr("Add to Favourites"));
        heart.add_css_class("np-toggle");
        heart.set_valign(gtk::Align::Start);
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        title_row.add_css_class("np-titles");
        title_row.append(&titles);
        title_row.append(&heart);

        let player = gtk::Box::new(gtk::Orientation::Vertical, 28);
        player.add_css_class("np-player");
        player.set_valign(gtk::Align::Center);
        player.append(&art_bin);
        player.append(&title_row);
        let player_clamp = adw::Clamp::builder().child(&player).maximum_size(ART_MAX).build();
        player_clamp.set_hexpand(false);

        // The dock: every control in one floating Veil capsule (Luminous).
        let shuffle = transport::shuffle_button(20);
        shuffle.add_css_class("np-toggle");
        let previous = transport::skip_button(false, 24);
        previous.add_css_class("np-skip");
        let (play, play_glyph) = transport::play_button(26);
        play.add_css_class("np-play");
        let next = transport::skip_button(true, 24);
        next.add_css_class("np-skip");
        let repeat = transport::repeat_button(20);
        repeat.add_css_class("np-toggle");
        let dock_transport = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        dock_transport.add_css_class("np-dock-transport");
        for b in [&shuffle, &previous, &play, &next, &repeat] {
            b.set_valign(gtk::Align::Center);
            dock_transport.append(b);
        }
        let seek = SeekBar::new(Times::Beside);
        seek.add_css_class("np-seek");
        seek.set_valign(gtk::Align::Center);
        seek.update_property(&[gtk::accessible::Property::Label(&tr("Position"))]);
        seek.connect_seek(|ms| {
            app().send(Command::Seek { position_ms: ms });
        });
        let volume = Volume::new(18, true);
        volume.row.add_css_class("np-volume");
        volume.row.set_valign(gtk::Align::Center);
        volume.bar.set_natural_width(VOLUME_WIDTH);
        volume.bar.set_hexpand(false);
        let devices = DeviceButton::new(false);
        devices.button.add_css_class("np-dock-button");
        devices.button.set_valign(gtk::Align::Center);
        let more = gtk::MenuButton::builder()
            .child(&Glyph::new("more", 20))
            .tooltip_text(tr("More"))
            .valign(gtk::Align::Center)
            .build();
        more.add_css_class("np-dock-button");
        more.update_property(&[gtk::accessible::Property::Label(&tr("More"))]);
        let dock_extras = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        dock_extras.add_css_class("np-dock-extras");
        dock_extras.append(&volume.row);
        dock_extras.append(&devices.button);
        dock_extras.append(&more);
        let dock = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        dock.add_css_class("np-dock");
        dock.add_css_class("veil");
        dock.update_property(&[gtk::accessible::Property::Label(&tr("Player"))]);
        dock.append(&dock_transport);
        dock.append(&seek);
        dock.append(&dock_extras);
        let dock_clamp = adw::Clamp::builder()
            .child(&dock)
            .maximum_size(DOCK_MAX)
            .tightening_threshold(DOCK_MAX)
            .build();
        dock_clamp.add_css_class("np-dock-holder");

        // The panels, fading out at their ends.
        let lyrics = lyrics::Lyrics::new();
        let queue_panel = queue::Queue::new();
        let info = info::Info::new();
        let panel = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(style::duration(jf_tokens::motion::DUR_2_MS))
            .vexpand(true)
            .build();
        // The lyrics fade out at both ends; the queue and info, whose
        // headings stay at the top, at the bottom only.
        for (pane, tab, top) in [
            (
                lyrics.root.clone().upcast::<gtk::Widget>(),
                Tab::Lyrics,
                PANEL_FADE_TOP,
            ),
            (queue_panel.root.clone().upcast(), Tab::Queue, 0.0),
            (info.root.clone().upcast(), Tab::Info, 0.0),
        ] {
            let faded = FadeBin::new(&pane);
            faded.set_fades(top, PANEL_FADE_BOTTOM);
            panel.add_named(&faded, Some(tab.name()));
        }
        panel.add_css_class("np-panel");
        let panel_clamp = adw::Clamp::builder()
            .child(&panel)
            .maximum_size(lyrics_logic::FULL_SIZE_WIDTH as i32)
            .build();
        panel_clamp.set_hexpand(true);

        let main = gtk::Box::new(gtk::Orientation::Horizontal, 64);
        main.add_css_class("np-main");
        main.set_vexpand(true);
        main.append(&player_clamp);
        main.append(&panel_clamp);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&topbar);
        column.append(&main);
        column.append(&dock_clamp);
        // An empty drawing area reports the view's size, so the artwork can
        // shrink to keep the whole player visible in short windows.
        let sizer = gtk::DrawingArea::new();
        sizer.set_can_target(false);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&backdrop.root));
        overlay.add_overlay(&column);
        overlay.add_overlay(&sizer);
        let root = adw::BreakpointBin::builder()
            .child(&overlay)
            .width_request(360)
            .height_request(400)
            .build();
        root.add_css_class("now-playing");
        let narrow =
            adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 880sp").expect("condition"));
        root.add_breakpoint(narrow);
        // Phone widths (also narrow): the back button keeps its glyph and
        // drops the word, which the tooltip and accessible name still say.
        let compact =
            adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 540sp").expect("condition"));
        compact.add_setters(&[(&back_label, "visible", false)]);
        root.add_breakpoint(compact);

        let inner = Rc::new(Inner {
            root,
            backdrop,
            art,
            art_bin,
            title,
            subtitle,
            device_line,
            device_glyph,
            device_label,
            error_line,
            heart,
            favorite: Cell::new(false),
            seek,
            play,
            play_glyph,
            shuffle,
            repeat,
            volume,
            dock,
            dock_transport,
            dock_extras,
            _devices: devices,
            more,
            more_key: RefCell::new(None),
            album_of: Rc::new(Cell::new(None)),
            tabs,
            tab_guard: Cell::new(false),
            panel,
            player: player.clone(),
            narrow: Cell::new(false),
            view_size: Cell::new((0, 0)),
            tab: Cell::new(Tab::Lyrics),
            show_panel_narrow: Cell::new(false),
            lyrics,
            queue: queue_panel,
            info,
        });
        let weak = Rc::downgrade(&inner);
        // The size arrives during layout; the artwork is fitted right after
        // it, so a frame never lays itself out again and again.
        sizer.connect_resize(move |_, w, h| {
            let Some(inner) = weak.upgrade() else { return };
            if inner.view_size.replace((w, h)) == (w, h) {
                return;
            }
            let weak = Rc::downgrade(&inner);
            glib::idle_add_local_once(move || {
                if let Some(inner) = weak.upgrade() {
                    fit_art(&inner);
                }
            });
        });
        let np = NowPlaying(inner);
        np.wire(&player_clamp, &panel_clamp);
        np
    }

    pub fn widget(&self) -> gtk::Widget {
        self.0.root.clone().upcast()
    }

    fn wire(&self, player_clamp: &adw::Clamp, panel_clamp: &adw::Clamp) {
        let inner = &self.0;
        // The favourite heart sends the opposite of what the core said.
        let weak = Rc::downgrade(&self.0);
        inner.heart.connect_clicked(move |_| {
            let Some(inner) = weak.upgrade() else { return };
            let a = app();
            let track = a.now.borrow().as_ref().map(|n| n.track.clone());
            if let Some(track) = track {
                a.send(Command::SetFavorite {
                    track,
                    favorite: !inner.favorite.get(),
                });
            }
        });
        // Panel switching.
        for (tab, button) in &inner.tabs {
            let weak = Rc::downgrade(&self.0);
            let tab = *tab;
            button.connect_toggled(move |b| {
                let Some(inner) = weak.upgrade() else { return };
                if inner.tab_guard.get() {
                    return;
                }
                let np = NowPlaying(inner);
                if b.is_active() {
                    np.select(Some(tab));
                } else if np.0.narrow.get() {
                    np.select(None);
                } else {
                    np.select(Some(tab));
                }
            });
        }
        // Narrow windows show the player or one panel, and the dock as a
        // card.
        let weak = Rc::downgrade(&self.0);
        let (pc, nc) = (player_clamp.clone(), panel_clamp.clone());
        inner.root.connect_current_breakpoint_notify(move |bin| {
            // Breakpoints change during allocation: re-lay out after it.
            let narrow = bin.current_breakpoint().is_some();
            let (weak, pc, nc) = (weak.clone(), pc.clone(), nc.clone());
            glib::idle_add_local_once(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.narrow.set(narrow);
                    if narrow {
                        inner.root.add_css_class("narrow");
                    } else {
                        inner.root.remove_css_class("narrow");
                    }
                    lay_out_dock(&inner, narrow);
                    fit_art(&inner);
                    NowPlaying(inner).layout(&pc, &nc);
                }
            });
        });
        let weak = Rc::downgrade(&self.0);
        let (pc, nc) = (player_clamp.clone(), panel_clamp.clone());
        inner.panel.connect_visible_child_notify(move |_| {
            if let Some(inner) = weak.upgrade() {
                NowPlaying(inner).layout(&pc, &nc);
            }
        });
        // The backdrop's light or dark bake follows the colour scheme, which
        // changes after the settings slice is announced (the window applies
        // it), or with the system's.
        let weak = Rc::downgrade(&self.0);
        let manager = adw::StyleManager::default();
        let id = manager.connect_dark_notify(move |_| {
            if let Some(inner) = weak.upgrade() {
                update_backdrop(&inner);
            }
        });
        let manager = manager.downgrade();
        let id = RefCell::new(Some(id));
        inner.root.connect_destroy(move |_| {
            if let (Some(m), Some(id)) = (manager.upgrade(), id.borrow_mut().take()) {
                m.disconnect(id);
            }
        });

        // Time-driven parts: the seek bar and its times, and the lyrics
        // (which a remote session does not show).
        let a = app();
        a.scheduler.add(
            &inner.seek,
            |bar, pos, running| bar.set_position(pos, app().duration_us(), running),
            |bar, pos| bar.next_change(pos),
        );
        let lyrics = Rc::downgrade(&inner.lyrics);
        let lyrics_next = lyrics.clone();
        a.scheduler.add(
            &inner.lyrics.root,
            move |_, pos, running| {
                if let Some(l) = lyrics.upgrade()
                    && !app().is_remote()
                {
                    l.update(pos, running);
                }
            },
            move |_, pos| {
                if app().is_remote() {
                    return None;
                }
                lyrics_next.upgrade().and_then(|l| l.next_change(pos))
            },
        );
        // Follow the core.
        let weak = Rc::downgrade(&self.0);
        a.listen_for(&inner.root, move |change| {
            let Some(inner) = weak.upgrade() else { return };
            match change {
                Change::NowPlaying => update_track(&inner),
                Change::Remote => {
                    update_track(&inner);
                    update_transport(&inner);
                    transport::update_toggles(&inner.shuffle, &inner.repeat);
                }
                Change::Library => update_favorite(&inner),
                Change::Playback | Change::Muted => update_transport(&inner),
                Change::Queue => transport::update_toggles(&inner.shuffle, &inner.repeat),
                Change::Lyrics => {
                    inner.lyrics.set_lyrics(app().lyrics.borrow().clone());
                    if !app().is_remote() {
                        inner
                            .info
                            .update(app().now.borrow().as_ref(), app().lyrics.borrow().as_ref());
                    }
                }
                Change::Details(page) => inner.info.details_arrived(page),
                Change::Sources => update_more(&inner),
                Change::Theme | Change::Settings => {
                    update_backdrop(&inner);
                    if matches!(change, Change::Settings) {
                        update_more(&inner);
                        // The Misty frame takes room from the artwork.
                        fit_art(&inner);
                        inner.lyrics.apply_settings();
                        update_art_scale(&inner);
                    }
                }
                _ => {}
            }
        });
        update_track(&self.0);
        update_transport(&self.0);
        transport::update_toggles(&inner.shuffle, &inner.repeat);
        update_backdrop(&self.0);
        inner.lyrics.set_lyrics(a.lyrics.borrow().clone());
        lay_out_dock(&self.0, false);
        self.select(Some(Tab::Lyrics));
    }

    /// Wide windows show the player and the panel; narrow ones the player
    /// or the panel. The highlighted tab is the panel on screen, if any.
    fn layout(&self, player: &adw::Clamp, panel: &adw::Clamp) {
        let inner = &self.0;
        let narrow = inner.narrow.get();
        let show_panel = !narrow || inner.show_panel_narrow.get();
        player.set_visible(!(narrow && show_panel));
        // Alone in a narrow view, the player takes the width and centres itself.
        player.set_hexpand(narrow);
        panel.set_visible(show_panel);
        inner.tab_guard.set(true);
        let tab = inner.tab.get();
        for (t, b) in &inner.tabs {
            b.set_active(show_panel && *t == tab);
        }
        inner.tab_guard.set(false);
    }

    /// Shows a panel (`None`: the player alone, in narrow windows).
    pub fn select(&self, tab: Option<Tab>) {
        let inner = &self.0;
        if let Some(t) = tab {
            inner.tab.set(t);
            inner.panel.set_visible_child_name(t.name());
        }
        inner.show_panel_narrow.set(tab.is_some());
        if let (Some(pc), Some(nc)) = (
            inner.player.parent().and_downcast::<adw::Clamp>(),
            panel_clamp_of(&inner.panel),
        ) {
            self.layout(&pc, &nc);
        }
    }

    /// Prepares the view for opening on `tab`. Without one, narrow windows
    /// open on the player and wide ones on the last panel.
    pub fn prepare(&self, tab: Option<Tab>) {
        let inner = &self.0;
        match tab {
            Some(t) => self.select(Some(t)),
            None if inner.narrow.get() => self.select(None),
            None => {
                self.select(Some(inner.tab.get()));
                // Should the view turn out narrow once laid out (it is not
                // measured while hidden), it opens on the player.
                inner.show_panel_narrow.set(false);
            }
        }
        if let Some(a) = try_app() {
            a.scheduler.refresh();
        }
    }

    /// The panel on screen (`None`: the player alone, in narrow windows).
    pub fn current_tab(&self) -> Option<Tab> {
        let inner = &self.0;
        (!inner.narrow.get() || inner.show_panel_narrow.get()).then(|| inner.tab.get())
    }

    /// Puts the keyboard focus on the play button, as Now Playing opens.
    pub fn focus_default(&self) {
        self.0.play.grab_focus();
    }
}

/// The clamp around the panels.
fn panel_clamp_of(panel: &gtk::Stack) -> Option<adw::Clamp> {
    panel.parent().and_downcast::<adw::Clamp>()
}

/// Gives More the menu `build` makes, unless it shows the menu for `key`
/// already (replacing the model would close it while open).
fn set_more(inner: &Inner, key: String, build: impl FnOnce() -> Option<gtk::gio::Menu>) {
    if inner.more_key.borrow().as_deref() == Some(key.as_str()) {
        return;
    }
    let menu = build();
    inner.more.set_sensitive(menu.is_some());
    inner.more.set_menu_model(menu.as_ref());
    if let Some(p) = inner.more.popover() {
        style::animate_popup(&p);
    }
    *inner.more_key.borrow_mut() = Some(key);
}

/// The dock in a row (wide windows: transport, seek bar with its times
/// beside it, then volume, device and More) or as a card (narrow: the seek
/// bar with its times under it, the transport, then the rest).
fn lay_out_dock(inner: &Inner, narrow: bool) {
    let dock = &inner.dock;
    if narrow {
        dock.set_orientation(gtk::Orientation::Vertical);
        dock.set_spacing(10);
        dock.reorder_child_after(&inner.seek, None::<&gtk::Widget>);
        dock.reorder_child_after(&inner.dock_transport, Some(&inner.seek));
        inner.seek.set_times(Times::Below);
        inner.dock_transport.set_halign(gtk::Align::Center);
        inner.dock_extras.set_halign(gtk::Align::Center);
        inner.volume.bar.set_hexpand(true);
        dock.add_css_class("dock-card");
    } else {
        dock.set_orientation(gtk::Orientation::Horizontal);
        dock.set_spacing(20);
        dock.reorder_child_after(&inner.dock_transport, None::<&gtk::Widget>);
        dock.reorder_child_after(&inner.seek, Some(&inner.dock_transport));
        inner.seek.set_times(Times::Beside);
        inner.dock_transport.set_halign(gtk::Align::Fill);
        inner.dock_extras.set_halign(gtk::Align::Fill);
        inner.volume.bar.set_hexpand(false);
        dock.remove_css_class("dock-card");
    }
}

/// The widest the dock gets (the stage's width less its side margins).
const DOCK_MAX: i32 = 1088;
/// The volume line's natural width in the dock.
const VOLUME_WIDTH: i32 = 112;
/// The panels fade out over these bands at their top and bottom.
const PANEL_FADE_TOP: f64 = 40.0;
const PANEL_FADE_BOTTOM: f64 = 56.0;

/// What the stage needs besides the artwork: the top bar, the titles, the
/// dock and the gaps between them (the approved mock-up: 380 px artwork in
/// an 800 px view).
const PLAYER_CHROME: i32 = 420;
/// The same in narrow windows, where the dock is a taller card.
const PLAYER_CHROME_NARROW: i32 = 440;
/// The player's side gutters in narrow windows (`space.s4` each, in CSS).
const NARROW_GUTTERS: i32 = 2 * 16;
/// The Misty glass frame around the artwork (12 px each side, in CSS).
const MISTY_FRAME: i32 = 2 * 12;
const ART_MIN: i32 = 160;
/// Luminous: the artwork at 380 px.
const ART_MAX: i32 = 380;

/// The artwork's side in a view of `width` × `height`: as large as the
/// rest of the stage leaves room for, between 160 and 380 px with its
/// frame, and never wider than a narrow window's gutters allow.
fn art_side(width: i32, height: i32, narrow: bool, misty: bool) -> i32 {
    let frame = if misty { MISTY_FRAME } else { 0 };
    let chrome = if narrow {
        PLAYER_CHROME_NARROW
    } else {
        PLAYER_CHROME
    };
    let by_height = height - chrome - frame;
    let by_width = if narrow { width - NARROW_GUTTERS } else { ART_MAX };
    by_height.min(by_width - frame).clamp(ART_MIN, ART_MAX)
}

/// Sizes the artwork for the view's last known size.
fn fit_art(inner: &Inner) {
    let (width, height) = inner.view_size.get();
    if width <= 0 || height <= 0 {
        return;
    }
    let misty = try_app().is_some_and(|a| a.settings.borrow().style == BackdropStyle::Misty);
    let side = art_side(width, height, inner.narrow.get(), misty);
    inner.art.set_size(side);
    inner.art.set_size_request(side, side);
}

fn update_track(inner: &Inner) {
    let a = app();
    if let Some(view) = a.remote_view() {
        show_remote(inner, &view);
        return;
    }
    inner.device_line.set_visible(false);
    inner.error_line.set_visible(false);
    inner.lyrics.set_remote(None);
    inner.queue.set_remote(None);
    let now = a.now.borrow().clone();
    match &now {
        Some(np) => {
            inner.title.set_label(&np.title);
            let artist = np.artist.clone().unwrap_or_else(convert::unknown_artist);
            inner
                .subtitle
                .set_label(&format::dot_join([Some(artist.as_str()), np.album.as_deref()]));
            inner.subtitle.set_visible(true);
            let key = convert::art_of(np.artwork.as_ref());
            inner.art.set_art(key);
            inner.art.update_property(&[gtk::accessible::Property::Label(&trf(
                "Artwork of {title}",
                &[("title", &np.title)],
            ))]);
        }
        None => {
            inner.title.set_label(&tr("Nothing Playing"));
            inner
                .subtitle
                .set_label(&tr("Pick a song, an album or a playlist."));
            inner.subtitle.set_visible(true);
            inner.art.set_art(None);
        }
    }
    inner.info.update(now.as_ref(), a.lyrics.borrow().as_ref());
    update_favorite(inner);
    update_more(inner);
}

/// The remote session's track on the stage, with "Playing on {Device}"
/// and its error; the panels say that the service has the lyrics and the
/// queue.
fn show_remote(inner: &Inner, view: &RemoteView) {
    let title = view.title.clone().unwrap_or_else(|| view.service.clone());
    inner.title.set_label(&title);
    inner
        .subtitle
        .set_label(&format::dot_join([view.artist.as_deref(), view.album.as_deref()]));
    inner
        .subtitle
        .set_visible(view.artist.is_some() || view.album.is_some());
    // Artwork URLs are not fetched by the app (networking stays in the
    // core): the placeholder shows.
    inner.art.set_art(convert::art_of(view.artwork.as_ref()));
    inner.art.update_property(&[gtk::accessible::Property::Label(&trf(
        "Artwork of {title}",
        &[("title", &title)],
    ))]);
    let (_, playing_on) = convert::remote_lines(view);
    inner.device_label.set_label(&playing_on);
    inner
        .device_glyph
        .set_name_glyph(view.device_kind.map_or("devices", remote::device_glyph));
    inner.device_line.set_visible(true);
    inner
        .error_line
        .set_label(view.error.as_deref().unwrap_or_default());
    inner.error_line.set_visible(view.error.is_some());
    inner.heart.set_visible(false);
    inner.info.update_remote(view);
    inner.lyrics.set_remote(Some(&view.service));
    inner.queue.set_remote(Some(&view.service));
    let key = format!("remote {} {:?}", view.service, view.link);
    set_more(inner, key, || Some(actions::remote_menu(view)));
}

/// More for the engine's track: what the song menu offers for it, with
/// Go to Album once the core has said which album a library song is on.
fn update_more(inner: &Inner) {
    let a = app();
    // Desktop Lyrics' items follow the window: part of the key.
    let desktop = crate::ui::desktoplyrics::shown();
    if let Some(view) = a.remote_view() {
        let key = format!("remote {} {:?} {desktop}", view.service, view.link);
        set_more(inner, key, || Some(actions::remote_menu(&view)));
        return;
    }
    let now = a.now.borrow().clone();
    let Some(np) = now else {
        set_more(inner, "none".into(), || None);
        return;
    };
    let known = inner.album_of.get().filter(|(uid, _)| *uid == np.uid);
    let album = known.and_then(|(_, a)| a);
    // The sources' names can change (Change::Sources): part of the key.
    let service = match &np.track {
        jf_core::TrackRef::Source(s) => a.source_name(&s.plugin),
        _ => String::new(),
    };
    let key = format!("local {} {album:?} {service} {:?} {desktop}", np.uid, np.link);
    set_more(inner, key, || Some(actions::now_playing_menu(&np, album)));
    let jf_core::TrackRef::Library(id) = np.track else {
        return;
    };
    if known.is_some() {
        return;
    }
    let (album_of, uid) = (inner.album_of.clone(), np.uid);
    let weak = window::shell().map(|s| Rc::downgrade(&s.now_playing.0));
    spawn(async move {
        let album = app()
            .query(move |core| core.track(id).ok().flatten().and_then(|t| t.album_id))
            .await
            .flatten();
        album_of.set(Some((uid, album)));
        if let Some(inner) = weak.and_then(|w| w.upgrade())
            && app().now.borrow().as_ref().map(|n| n.uid) == Some(uid)
            && !app().is_remote()
        {
            update_more(&inner);
        }
    });
}

/// Asks the core whether the current track is a favourite.
fn update_favorite(inner: &Inner) {
    let a = app();
    if a.is_remote() {
        inner.heart.set_visible(false);
        return;
    }
    let track = a.now.borrow().as_ref().map(|n| n.track.clone());
    inner.heart.set_visible(track.is_some());
    let Some(track) = track else { return };
    let Some(shell_np) = window::shell().map(|s| s.now_playing.clone()) else {
        show_favorite(inner, false);
        return;
    };
    let weak = Rc::downgrade(&shell_np.0);
    glib::spawn_future_local(async move {
        let wanted = track.clone();
        let favorite = app()
            .query(move |core| core.is_favorite(&wanted).unwrap_or(false))
            .await
            .unwrap_or(false);
        if let Some(inner) = weak.upgrade()
            && app().now.borrow().as_ref().map(|n| &n.track) == Some(&track)
        {
            show_favorite(&inner, favorite);
        }
    });
}

fn show_favorite(inner: &Inner, favorite: bool) {
    inner.favorite.set(favorite);
    if let Some(g) = glyph::of(&inner.heart) {
        g.set_name_glyph(if favorite { "heart-filled" } else { "heart" });
    }
    let label = if favorite {
        tr("Remove from Favourites")
    } else {
        tr("Add to Favourites")
    };
    set_on(&inner.heart, favorite, &label);
}

fn update_transport(inner: &Inner) {
    transport::update_play(&inner.play, &inner.play_glyph);
    inner.volume.update();
    update_art_scale(inner);
}

/// The artwork rests at 0.88 while paused and springs back to full size
/// when playing (`dur-3` Spring); with reduced motion it stays at full size.
fn update_art_scale(inner: &Inner) {
    let allowed = style::allowed();
    let target = artwork_scale(app().is_playing(), allowed);
    let bin = &inner.art_bin;
    if (bin.scale() - target).abs() < 1e-6 {
        return;
    }
    let from = bin.scale();
    bin.animate(
        artwork_plan(allowed),
        move |b, e| b.set_transform(0.0, from + (target - from) * e),
        move |b| b.set_transform(0.0, target),
    );
}

/// The backdrop for the current style and theme.
fn update_backdrop(inner: &Inner) {
    let a = app();
    let style = a.settings.borrow().style;
    let theme = a.theme.borrow().clone();
    let dark = adw::StyleManager::default().is_dark();
    let path = match (style, theme.as_deref()) {
        (BackdropStyle::Artwork, Some(t)) => backdrop_path(t, dark),
        _ => None,
    };
    inner.backdrop.set_image(path);
    if style == BackdropStyle::Misty {
        inner.root.add_css_class("style-misty");
        inner.root.remove_css_class("style-artwork");
    } else {
        inner.root.add_css_class("style-artwork");
        inner.root.remove_css_class("style-misty");
    }
}

fn backdrop_path(theme: &ThemeSnapshot, dark: bool) -> Option<PathBuf> {
    if dark {
        theme.backdrop_dark.clone()
    } else {
        theme.backdrop_light.clone()
    }
}

/// The baked artwork backdrop, crossfaded over the Misty field.
struct Backdrop {
    root: gtk::Overlay,
    pictures: [gtk::Picture; 2],
    front: Cell<usize>,
    path: RefCell<Option<PathBuf>>,
    animation: RefCell<Option<adw::TimedAnimation>>,
}

impl Backdrop {
    fn new() -> Backdrop {
        let field = gtk::Box::new(gtk::Orientation::Vertical, 0);
        field.add_css_class("np-field");
        let root = gtk::Overlay::new();
        root.set_child(Some(&field));
        let pictures = [gtk::Picture::new(), gtk::Picture::new()];
        for p in &pictures {
            p.set_content_fit(gtk::ContentFit::Cover);
            p.set_can_shrink(true);
            p.set_opacity(0.0);
            p.set_can_target(false);
            p.add_css_class("np-backdrop");
            root.add_overlay(p);
        }
        root.set_can_target(false);
        Backdrop {
            root,
            pictures,
            front: Cell::new(0),
            path: RefCell::new(None),
            animation: RefCell::new(None),
        }
    }

    fn set_image(&self, path: Option<PathBuf>) {
        if *self.path.borrow() == path {
            return;
        }
        *self.path.borrow_mut() = path.clone();
        let Some(path) = path else {
            self.fade_to(None);
            return;
        };
        let pictures = self.pictures.clone();
        let wanted = path.clone();
        app().thumbs.load(ArtKey::File(path), 256, move |texture| {
            let Some(shell) = window::shell() else { return };
            let backdrop = &shell.now_playing.0.backdrop;
            if backdrop.path.borrow().as_ref() != Some(&wanted) {
                return;
            }
            let _ = &pictures;
            backdrop.fade_to(texture.cloned());
        });
    }

    /// Crossfades to `texture` (or to the field) on dur-4 with Fluid.
    fn fade_to(&self, texture: Option<gtk::gdk::Texture>) {
        if let Some(a) = self.animation.borrow_mut().take() {
            a.skip();
        }
        let front = self.front.get();
        let back = 1 - front;
        let (old, new) = (self.pictures[front].clone(), self.pictures[back].clone());
        let has_new = texture.is_some();
        new.set_paintable(texture.as_ref());
        let duration = motion::backdrop_crossfade_ms(style::allowed());
        let old_start = old.opacity();
        let target = adw::CallbackAnimationTarget::new(glib::clone!(
            #[weak]
            old,
            #[weak]
            new,
            move |t| {
                let e = FLUID.ease(t);
                old.set_opacity(old_start * (1.0 - e));
                if has_new {
                    new.set_opacity(e);
                }
            }
        ));
        let anim = adw::TimedAnimation::new(&self.root, 0.0, 1.0, duration.max(1), target);
        anim.set_easing(adw::Easing::Linear);
        anim.connect_done(glib::clone!(
            #[weak]
            old,
            move |_| {
                old.set_paintable(gtk::gdk::Paintable::NONE);
            }
        ));
        self.front.set(back);
        anim.play();
        *self.animation.borrow_mut() = Some(anim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_fills_wide_views_up_to_its_limit() {
        // Luminous: 380 px artwork on the stage.
        assert_eq!(ART_MAX, 380);
        assert_eq!(art_side(1280, 800, false, false), ART_MAX);
        assert_eq!(art_side(1400, 1000, false, false), ART_MAX);
        // The Misty frame stays within the same 400 px.
        assert_eq!(art_side(1400, 1000, false, true), ART_MAX - MISTY_FRAME);
    }

    #[test]
    fn artwork_leaves_room_for_the_player_in_short_views() {
        assert_eq!(art_side(1200, 780, false, false), 780 - PLAYER_CHROME);
        assert_eq!(art_side(1200, 500, false, false), ART_MIN);
    }

    #[test]
    fn artwork_keeps_narrow_gutters() {
        // A phone: 360 × 800, the dock a card under the titles.
        assert_eq!(art_side(360, 800, true, false), 360 - NARROW_GUTTERS);
        assert_eq!(art_side(360, 640, true, false), 640 - PLAYER_CHROME_NARROW);
        assert_eq!(art_side(360, 800, true, true) + MISTY_FRAME, 360 - NARROW_GUTTERS);
    }
}
