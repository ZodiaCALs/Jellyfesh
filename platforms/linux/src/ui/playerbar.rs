//! The player bar, for windows 900 px wide and wider (design-controls.md,
//! "Mini-player and the desktop player bar"), after Media Player and Apple
//! Music: a floating Veil bar 80 px high with `r-xl` corners. On the start
//! side the artwork and two lines of type open Now Playing; in the middle
//! the transport capsule (shuffle, previous, play, next, repeat; play is
//! the only accent fill) sits above the seek bar and its times; on the end
//! side the quality badge (ADR-0026 §3), Lyrics, Queue, the volume and More
//! (Now Playing's menu). With Show Lyrics Bar on, the line
//! being sung fills word by word under the title (ADR-0022 §6). This bends CMP-02 (one control) on
//! purpose, for desktop use; narrow windows keep the mini-player.
//!
//! While a remote session runs (ADR-0018) the bar shows its track with
//! "Playing on {Device}" under the title (its error in a quieter line), the
//! volume sets the device's volume, and the device button opens Devices.

use adw::prelude::*;

use crate::logic::seek::Times;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::lyricsbar::{LyricsBarView, Mode};
use crate::ui::outputs::DeviceButton;
use crate::ui::quality::QualityButton;
use crate::ui::state::{Change, app};
use crate::ui::transport::{self, Volume};
use crate::ui::widgets::art::Art;
use crate::ui::widgets::glyph::{self, Glyph};
use crate::ui::widgets::seekbar::SeekBar;
use crate::ui::window;

/// The middle's width: the transport capsule above the seek bar; narrow
/// bars give it less, down to the web's 232 px.
const MIDDLE_WIDTH: i32 = 360;
const MIDDLE_WIDTH_NARROW: i32 = 232;
/// The volume line's natural width in the bar.
const VOLUME_WIDTH: i32 = 96;

/// The bar, and what changes with the window's width.
pub struct PlayerBar {
    pub root: gtk::Widget,
}

impl std::fmt::Debug for PlayerBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerBar").finish_non_exhaustive()
    }
}

pub fn build() -> PlayerBar {
    // Start: artwork and titles, which open Now Playing.
    let art = Art::new(48, jf_tokens::artwork::RADIUS_LIST);
    art.set_size_request(48, 48);
    art.set_valign(gtk::Align::Center);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    title.add_css_class("bar-title");
    let artist = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    artist.add_css_class("bar-artist");
    let error = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .visible(false)
        .build();
    error.add_css_class("bar-error");
    // The lyrics bar: the line being sung under the title, instead of the
    // artist, while Show Lyrics Bar is on and the lyrics are timed.
    let lyrics_bar = LyricsBarView::new(Mode::Line);
    let shown_artist = artist.downgrade();
    lyrics_bar.connect_showing(move |showing| {
        if let Some(a) = shown_artist.upgrade() {
            a.set_visible(!showing);
        }
    });
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_valign(gtk::Align::Center);
    texts.append(&title);
    texts.append(&artist);
    texts.append(&lyrics_bar.root);
    texts.append(&error);
    let open_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    open_box.append(&art);
    open_box.append(&texts);
    let open = gtk::Button::builder().child(&open_box).build();
    open.add_css_class("bar-open");
    open.set_valign(gtk::Align::Center);
    open.set_tooltip_text(Some(&tr("Open Now Playing")));
    open.connect_clicked(|_| window::open_now_playing(None));

    // Middle: the transport capsule above the seek bar.
    let shuffle = transport::shuffle_button(18);
    shuffle.add_css_class("bar-toggle");
    let previous = transport::skip_button(false, 20);
    previous.add_css_class("bar-skip");
    let (play, glyph) = transport::play_button(22);
    play.add_css_class("bar-play");
    let next = transport::skip_button(true, 20);
    next.add_css_class("bar-skip");
    let repeat = transport::repeat_button(18);
    repeat.add_css_class("bar-toggle");
    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    capsule.add_css_class("bar-transport");
    capsule.set_halign(gtk::Align::Center);
    for b in [&shuffle, &previous, &play, &next, &repeat] {
        capsule.append(b);
    }
    let seek = SeekBar::new(Times::Beside);
    seek.add_css_class("bar-seek");
    seek.update_property(&[gtk::accessible::Property::Label(&tr("Position"))]);
    seek.connect_seek(|ms| {
        app().send(jf_core::Command::Seek { position_ms: ms });
    });
    let middle = gtk::Box::new(gtk::Orientation::Vertical, 0);
    middle.add_css_class("bar-middle");
    middle.set_valign(gtk::Align::Center);
    middle.set_size_request(MIDDLE_WIDTH, -1);
    middle.append(&capsule);
    middle.append(&seek);

    // End: the quality badge; Lyrics, Queue, the device button (while a
    // remote source plays) and the volume; More.
    let quality = QualityButton::new();
    quality.button.add_css_class("bar-badge");
    // The bar is at the bottom: its menus open upwards.
    quality.button.set_direction(gtk::ArrowType::Up);
    let lyrics = glyph::button("lyrics", 18, &tr("Lyrics"));
    lyrics.add_css_class("bar-icon");
    lyrics.set_valign(gtk::Align::Center);
    lyrics.set_action_name(Some("app.lyrics"));
    let queue = glyph::button("queue", 18, &tr("Queue"));
    queue.add_css_class("bar-icon");
    queue.set_valign(gtk::Align::Center);
    queue.set_action_name(Some("app.queue"));
    let volume = Volume::new(16, false);
    volume.row.set_valign(gtk::Align::Center);
    volume.bar.set_natural_width(VOLUME_WIDTH);
    volume.bar.set_hexpand(false);
    // Devices, while a remote session runs.
    let devices = DeviceButton::new(false);
    devices.button.add_css_class("bar-icon");
    devices.button.set_valign(gtk::Align::Center);
    devices.button.set_visible(false);
    let end = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    end.add_css_class("bar-end");
    end.set_halign(gtk::Align::End);
    // More: Now Playing's menu for what plays, as it stands when it opens.
    let more = gtk::MenuButton::builder()
        .child(&Glyph::new("more", 18))
        .tooltip_text(tr("More"))
        .valign(gtk::Align::Center)
        .build();
    more.add_css_class("bar-icon");
    more.set_direction(gtk::ArrowType::Up);
    more.update_property(&[gtk::accessible::Property::Label(&tr("More"))]);
    more.set_create_popup_func(|b| {
        let menu = window::shell().and_then(|s| s.now_playing.more_menu());
        b.set_menu_model(menu.as_ref());
        if let Some(p) = b.popover() {
            crate::ui::style::animate_popup(&p);
        }
    });
    // The volume row hides itself for a remote device without volume; the
    // holder hides it on narrow bars.
    let volume_holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    volume_holder.append(&volume.row);
    end.append(&quality.button);
    end.append(&lyrics);
    end.append(&queue);
    end.append(&devices.button);
    end.append(&volume_holder);
    end.append(&more);

    // The middle stays centred; the start and end share the rest.
    open.set_halign(gtk::Align::Start);
    let row = gtk::CenterBox::new();
    row.add_css_class("bar-row");
    row.set_start_widget(Some(&open));
    row.set_center_widget(Some(&middle));
    row.set_end_widget(Some(&end));
    let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bar.add_css_class("player-bar");
    bar.add_css_class("veil");
    bar.append(&row);
    bar.update_property(&[gtk::accessible::Property::Label(&tr("Player"))]);
    // The bar narrows as the web's does, by its own width: under 900 px it
    // drops the volume line (Ctrl+Up and Ctrl+Down still change the
    // volume), under 760 Lyrics and Queue (Now Playing has both, and Ctrl+L
    // and Ctrl+U), under 640 the mute button; the title keeps room to be
    // read.
    let fit = adw::BreakpointBin::builder()
        .child(&bar)
        .width_request(320)
        .height_request(jf_tokens::player::BAR_HEIGHT as i32)
        .build();
    let narrower: Vec<adw::Breakpoint> = [899.0, 759.0, 639.0]
        .into_iter()
        .map(|width| {
            let b = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
                adw::BreakpointConditionLengthType::MaxWidth,
                width,
                adw::LengthUnit::Px,
            ));
            fit.add_breakpoint(b.clone());
            b
        })
        .collect();
    let shown = (
        volume.bar.downgrade(),
        lyrics.downgrade(),
        queue.downgrade(),
        volume_holder.downgrade(),
        middle.downgrade(),
    );
    fit.connect_current_breakpoint_notify(move |fit| {
        let level = fit
            .current_breakpoint()
            .and_then(|current| narrower.iter().position(|b| *b == current))
            .map_or(0, |i| i + 1);
        if let Some(line) = shown.0.upgrade() {
            line.set_visible(level < 1);
        }
        for pane in [shown.1.upgrade(), shown.2.upgrade()].into_iter().flatten() {
            pane.set_visible(level < 2);
        }
        if let Some(volume) = shown.3.upgrade() {
            volume.set_visible(level < 3);
        }
        if let Some(middle) = shown.4.upgrade() {
            let width = if level < 2 {
                MIDDLE_WIDTH
            } else {
                MIDDLE_WIDTH_NARROW
            };
            middle.set_size_request(width, -1);
        }
    });
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.add_css_class("bar-holder");
    holder.set_valign(gtk::Align::End);
    holder.append(&fit);

    let weak = (
        title.downgrade(),
        artist.downgrade(),
        art.downgrade(),
        open.downgrade(),
        error.downgrade(),
        devices.button.downgrade(),
    );
    let update_track = move || {
        let (Some(title), Some(artist), Some(art), Some(open), Some(error), Some(devices)) = (
            weak.0.upgrade(),
            weak.1.upgrade(),
            weak.2.upgrade(),
            weak.3.upgrade(),
            weak.4.upgrade(),
            weak.5.upgrade(),
        ) else {
            return;
        };
        let remote = app().remote_view();
        devices.set_visible(remote.is_some());
        if let Some(view) = remote {
            let (name, playing_on) = convert::remote_lines(&view);
            title.set_label(&name);
            artist.set_label(&playing_on);
            artist.add_css_class("playing-on");
            error.set_label(view.error.as_deref().unwrap_or_default());
            error.set_visible(view.error.is_some());
            art.set_art(convert::art_of(view.artwork.as_ref()));
            open.update_property(&[gtk::accessible::Property::Label(&trf(
                "Open Now Playing: {title}",
                &[("title", &name)],
            ))]);
            return;
        }
        artist.remove_css_class("playing-on");
        error.set_visible(false);
        let now = app().now.borrow().clone();
        match &now {
            Some(np) => {
                title.set_label(&np.title);
                let who = np.artist.clone().unwrap_or_else(convert::unknown_artist);
                artist.set_label(&who);
                art.set_art(convert::art_of(np.artwork.as_ref()));
                open.update_property(&[gtk::accessible::Property::Label(&trf(
                    "Open Now Playing: {title}",
                    &[("title", &np.title)],
                ))]);
            }
            None => {
                title.set_label(&tr("Ready to play"));
                artist.set_label(&tr("Press play to resume"));
                art.set_art(None);
                open.update_property(&[gtk::accessible::Property::Label(&tr("Open Now Playing"))]);
            }
        }
    };
    update_track();
    transport::update_play(&play, &glyph);
    transport::update_toggles(&shuffle, &repeat);
    volume.update();
    let a = app();
    let (w_play, w_glyph, w_shuffle, w_repeat) = (
        play.downgrade(),
        glyph.downgrade(),
        shuffle.downgrade(),
        repeat.downgrade(),
    );
    let volume = std::rc::Rc::new(volume);
    let vol = volume.clone();
    let track = update_track.clone();
    let (r_play, r_glyph, r_shuffle, r_repeat) = (
        play.downgrade(),
        glyph.downgrade(),
        shuffle.downgrade(),
        repeat.downgrade(),
    );
    let vol_remote = volume.clone();
    a.listen_for(&holder, move |change| {
        if let Change::Remote = change {
            track();
            if let (Some(p), Some(g)) = (r_play.upgrade(), r_glyph.upgrade()) {
                transport::update_play(&p, &g);
            }
            if let (Some(s), Some(r)) = (r_shuffle.upgrade(), r_repeat.upgrade()) {
                transport::update_toggles(&s, &r);
            }
            vol_remote.update();
        }
    });
    a.listen_for(&holder, move |change| match change {
        Change::NowPlaying => update_track(),
        Change::Playback | Change::Muted => {
            if let (Some(p), Some(g)) = (w_play.upgrade(), w_glyph.upgrade()) {
                transport::update_play(&p, &g);
            }
            vol.update();
        }
        Change::Queue => {
            if let (Some(s), Some(r)) = (w_shuffle.upgrade(), w_repeat.upgrade()) {
                transport::update_toggles(&s, &r);
            }
        }
        _ => {}
    });
    crate::ui::pages::keep_alive(&holder, volume);
    crate::ui::pages::keep_alive(&holder, devices);
    crate::ui::pages::keep_alive(&holder, quality);
    crate::ui::pages::keep_alive(&holder, lyrics_bar);
    a.scheduler.add(
        &seek,
        |bar, pos, running| bar.set_position(pos, app().duration_us(), running),
        |bar, pos| bar.next_change(pos),
    );
    PlayerBar {
        root: holder.upcast(),
    }
}
