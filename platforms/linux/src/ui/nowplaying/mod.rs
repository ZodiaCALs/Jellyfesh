//! Now Playing (after Apple Music): the artwork, title, seek bar, transport
//! and volume on the left; lyrics, the queue or track info on the right (one
//! column when the window is narrow). The backdrop is the core's baked
//! artwork backdrop, crossfaded on track change (style Artwork), or the
//! Misty field at the artwork hue (style Misty). The artwork scales to 0.88
//! while paused and springs back when playing; play and pause morph into
//! each other (design-controls.md, "Now Playing").

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
use crate::logic::motion::{self, artwork_plan, artwork_scale};
use crate::logic::seek::Times;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{Change, app, try_app};
use crate::ui::style::{self, FLUID};
use crate::ui::thumbs::ArtKey;
use crate::ui::transport::{self, Volume, set_on};
use crate::ui::widgets::art::Art;
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

struct Inner {
    root: adw::BreakpointBin,
    backdrop: Backdrop,
    art: Art,
    /// The artwork and its frame, scaled while paused.
    art_bin: MotionBin,
    title: gtk::Label,
    subtitle: gtk::Label,
    heart: gtk::Button,
    favorite: Cell<bool>,
    seek: SeekBar,
    play: gtk::Button,
    play_glyph: PlayPause,
    shuffle: gtk::Button,
    repeat: gtk::Button,
    volume: Volume,
    tabs: Vec<(Tab, gtk::ToggleButton)>,
    tab_guard: Cell<bool>,
    panel: gtk::Stack,
    player: gtk::Box,
    narrow: Cell<bool>,
    /// The view's size, for fitting the artwork.
    view_size: Cell<(i32, i32)>,
    tab: Cell<Tab>,
    show_panel_narrow: Cell<bool>,
    chip: gtk::MenuButton,
    chip_label: gtk::Label,
    lyrics: Rc<lyrics::Lyrics>,
    _queue: Rc<queue::Queue>,
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

        // Top bar: back, the panel switcher, the output device chip.
        let back = gtk::Button::builder().build();
        let back_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        back_box.append(&Glyph::new("back", 20));
        let back_label = gtk::Label::new(Some(&tr("Library")));
        back_box.append(&back_label);
        back.set_child(Some(&back_box));
        back.add_css_class("np-back");
        back.set_tooltip_text(Some(&tr("Back to Library")));
        back.update_property(&[gtk::accessible::Property::Label(&tr("Back to Library"))]);
        back.connect_clicked(|_| window::close_now_playing());
        let segmented = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        segmented.add_css_class("segmented");
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
        let chip_label = gtk::Label::new(Some(&tr("Speakers")));
        chip_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        chip_label.set_max_width_chars(18);
        let chip_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chip_box.append(&Glyph::new("speaker", 18));
        chip_box.append(&chip_label);
        let chip = gtk::MenuButton::builder()
            .child(&chip_box)
            .tooltip_text(tr("Output Device"))
            .build();
        chip.add_css_class("device-chip");
        chip.update_property(&[gtk::accessible::Property::Label(&tr("Output Device"))]);
        let topbar = gtk::CenterBox::new();
        topbar.add_css_class("np-topbar");
        topbar.add_css_class("veil");
        topbar.set_start_widget(Some(&back));
        topbar.set_center_widget(Some(&segmented));
        topbar.set_end_widget(Some(&chip));

        // The player column.
        let art = Art::new(400, jf_tokens::artwork::RADIUS_HERO);
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
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        titles.set_hexpand(true);
        titles.append(&title);
        titles.append(&subtitle);
        let heart = glyph::button("heart", 22, &tr("Add to Favourites"));
        heart.add_css_class("np-toggle");
        heart.set_valign(gtk::Align::Center);
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        title_row.append(&titles);
        title_row.append(&heart);

        // The straight seek bar, its times under its ends.
        let seek = SeekBar::new(Times::Below);
        seek.add_css_class("np-seek");
        seek.update_property(&[gtk::accessible::Property::Label(&tr("Position"))]);
        seek.connect_seek(|ms| {
            app().send(Command::Seek { position_ms: ms });
        });

        let shuffle = transport::shuffle_button(20);
        shuffle.add_css_class("np-toggle");
        let previous = transport::skip_button(false, 24);
        previous.add_css_class("np-skip");
        let (play, play_glyph) = transport::play_button(26);
        play.add_css_class("np-play");
        let next = transport::skip_button(true, 24);
        next.add_css_class("np-skip");
        let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        capsule.add_css_class("transport");
        capsule.add_css_class("veil");
        capsule.append(&previous);
        capsule.append(&play);
        capsule.append(&next);
        let repeat = transport::repeat_button(20);
        repeat.add_css_class("np-toggle");
        let transport_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        transport_row.set_halign(gtk::Align::Center);
        transport_row.add_css_class("np-transport");
        transport_row.append(&shuffle);
        transport_row.append(&capsule);
        transport_row.append(&repeat);

        // The volume: the same line between two speaker glyphs.
        let volume = Volume::new(18, true);
        volume.row.add_css_class("np-volume");

        let player = gtk::Box::new(gtk::Orientation::Vertical, 20);
        player.add_css_class("np-player");
        player.set_valign(gtk::Align::Center);
        player.append(&art_bin);
        player.append(&title_row);
        player.append(&seek);
        player.append(&transport_row);
        player.append(&volume.row);
        let player_clamp = adw::Clamp::builder().child(&player).maximum_size(400).build();
        player_clamp.set_hexpand(false);

        // The panels.
        let lyrics = lyrics::Lyrics::new();
        let queue_panel = queue::Queue::new();
        let info = info::Info::new();
        let panel = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(style::duration(jf_tokens::motion::DUR_2_MS))
            .vexpand(true)
            .build();
        panel.add_named(&lyrics.root, Some(Tab::Lyrics.name()));
        panel.add_named(&queue_panel.root, Some(Tab::Queue.name()));
        panel.add_named(&info.root, Some(Tab::Info.name()));
        panel.add_css_class("np-panel");
        let panel_clamp = adw::Clamp::builder().child(&panel).maximum_size(640).build();
        panel_clamp.set_hexpand(true);

        let main = gtk::Box::new(gtk::Orientation::Horizontal, 56);
        main.add_css_class("np-main");
        main.set_vexpand(true);
        main.append(&player_clamp);
        main.append(&panel_clamp);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&topbar);
        column.append(&main);
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
        // Phone widths (also narrow): the top bar keeps its glyphs and drops
        // the words, which the tooltips and accessible names still say.
        let compact =
            adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 540sp").expect("condition"));
        compact.add_setters(&[(&back_label, "visible", false), (&chip_label, "visible", false)]);
        root.add_breakpoint(compact);

        let inner = Rc::new(Inner {
            root,
            backdrop,
            art,
            art_bin,
            title,
            subtitle,
            heart,
            favorite: Cell::new(false),
            seek,
            play,
            play_glyph,
            shuffle,
            repeat,
            volume,
            tabs,
            tab_guard: Cell::new(false),
            panel,
            player: player.clone(),
            narrow: Cell::new(false),
            view_size: Cell::new((0, 0)),
            tab: Cell::new(Tab::Lyrics),
            show_panel_narrow: Cell::new(false),
            chip,
            chip_label,
            lyrics,
            _queue: queue_panel,
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
        // Narrow windows show the player or one panel.
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
        // The device chip lists outputs when opened (the core never polls).
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");
        style::animate_popup(&popover);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("device-list");
        popover.set_child(Some(&list));
        inner.chip.set_popover(Some(&popover));
        popover.connect_show(|_| {
            app().send(Command::RefreshDevices);
        });
        let weak_list = list.downgrade();
        let weak_pop = popover.downgrade();
        let fill = move || {
            let (Some(list), Some(pop)) = (weak_list.upgrade(), weak_pop.upgrade()) else {
                return;
            };
            fill_devices(&list, &pop);
        };
        fill();

        // Time-driven parts: the seek bar and its times, and the lyrics.
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
                if let Some(l) = lyrics.upgrade() {
                    l.update(pos, running);
                }
            },
            move |_, pos| lyrics_next.upgrade().and_then(|l| l.next_change(pos)),
        );
        // Follow the core.
        let weak = Rc::downgrade(&self.0);
        a.listen_for(&inner.root, move |change| {
            let Some(inner) = weak.upgrade() else { return };
            match change {
                Change::NowPlaying => update_track(&inner),
                Change::Library => update_favorite(&inner),
                Change::Playback | Change::Muted => update_transport(&inner),
                Change::Queue => transport::update_toggles(&inner.shuffle, &inner.repeat),
                Change::Lyrics => inner.lyrics.set_lyrics(app().lyrics.borrow().clone()),
                Change::Theme | Change::Settings => {
                    update_backdrop(&inner);
                    if matches!(change, Change::Settings) {
                        // The Misty frame takes room from the artwork.
                        fit_art(&inner);
                        inner.lyrics.apply_settings();
                        update_art_scale(&inner);
                    }
                }
                Change::Devices => fill(),
                _ => {}
            }
        });
        update_track(&self.0);
        update_transport(&self.0);
        transport::update_toggles(&inner.shuffle, &inner.repeat);
        update_backdrop(&self.0);
        inner.lyrics.set_lyrics(a.lyrics.borrow().clone());
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
            inner.panel.parent().and_downcast::<adw::Clamp>(),
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

/// What the player column needs besides the artwork: top bar, padding,
/// titles, seek bar, transport, volume and gaps.
const PLAYER_CHROME: i32 = 430;
/// The player's side gutters in narrow windows (`space.s4` each, in CSS).
const NARROW_GUTTERS: i32 = 2 * 16;
/// The Misty glass frame around the artwork (12 px each side, in CSS).
const MISTY_FRAME: i32 = 2 * 12;
const ART_MIN: i32 = 160;
const ART_MAX: i32 = 400;

/// The artwork's side in a view of `width` × `height`: as large as the
/// rest of the player leaves room for, between 160 and 400 px with its
/// frame, and never wider than a narrow window's gutters allow.
fn art_side(width: i32, height: i32, narrow: bool, misty: bool) -> i32 {
    let frame = if misty { MISTY_FRAME } else { 0 };
    let by_height = height - PLAYER_CHROME - frame;
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
    let now = a.now.borrow().clone();
    match &now {
        Some(np) => {
            inner.title.set_label(&np.title);
            let artist = np.artist.clone().unwrap_or_else(convert::unknown_artist);
            inner
                .subtitle
                .set_label(&format::dot_join([Some(artist.as_str()), np.album.as_deref()]));
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
            inner.art.set_art(None);
        }
    }
    inner.info.update(now.as_deref(), a.lyrics.borrow().as_deref());
    update_favorite(inner);
}

/// Asks the core whether the current track is a favourite.
fn update_favorite(inner: &Inner) {
    let a = app();
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

fn fill_devices(list: &gtk::ListBox, popover: &gtk::Popover) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    let a = app();
    let devices = a.devices.borrow().clone();
    let selected = devices.selected.clone();
    let mut entries: Vec<(Option<String>, String)> = vec![(None, tr("System Default"))];
    entries.extend(
        devices
            .devices
            .iter()
            .map(|d| (Some(d.name.clone()), d.name.clone())),
    );
    for (name, label) in entries {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let check = Glyph::new("check", 16);
        check.set_opacity(if name == selected { 1.0 } else { 0.0 });
        row.append(&check);
        let l = gtk::Label::new(Some(&label));
        l.set_xalign(0.0);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        l.set_max_width_chars(32);
        row.append(&l);
        let button = gtk::Button::builder().child(&row).build();
        button.add_css_class("flat");
        button.update_property(&[gtk::accessible::Property::Label(&label)]);
        let pop = popover.downgrade();
        button.connect_clicked(move |_| {
            app().send(Command::SetOutputDevice { name: name.clone() });
            if let Some(p) = pop.upgrade() {
                p.popdown();
            }
        });
        list.append(&button);
    }
    if let Some(shell) = window::shell() {
        let label = selected.unwrap_or_else(|| tr("Speakers"));
        shell.now_playing.0.chip_label.set_label(&label);
    }
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
        // A phone: 360 × 800.
        assert_eq!(art_side(360, 800, true, false), 360 - NARROW_GUTTERS);
        assert_eq!(art_side(360, 800, true, true) + MISTY_FRAME, 360 - NARROW_GUTTERS);
    }
}
