//! The mini-player, for windows under 900 px: a floating glass capsule along
//! the bottom of the library with artwork, two lines of type, play and next
//! (docs/ui.md, "The player bar and the mini player"), and a straight 2 px progress line
//! along its bottom edge (design-controls.md, "Mini-player and the desktop
//! player bar"). Clicking the capsule opens Now Playing. The play button is
//! never accent-filled, so a library page keeps its own primary action
//! (COL-04). Wider windows show the player bar instead (`playerbar.rs`);
//! only the one on screen is drawn and scheduled. While a remote session
//! runs (ADR-0018) it shows the session's track with "Playing on {Device}"
//! under the title and its error in a quieter line.

use adw::prelude::*;
use jf_core::Command;

use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{Change, app};
use crate::ui::widgets::art::Art;
use crate::ui::widgets::glyph::Glyph;
use crate::ui::widgets::playpause::PlayPause;
use crate::ui::widgets::progressline::ProgressLine;
use crate::ui::window;

/// The capsule's corners (`radius.r-full`, as in the stylesheet): a pill.
const CAPSULE_RADIUS: f64 = 999.0;

pub fn build() -> gtk::Widget {
    let art = Art::new(44, 22.0);
    art.set_round(true);
    art.set_size_request(44, 44);
    art.set_valign(gtk::Align::Center);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    title.add_css_class("mini-title");
    let artist = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    artist.add_css_class("mini-artist");
    let error = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .visible(false)
        .build();
    error.add_css_class("bar-error");
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 0);
    texts.set_valign(gtk::Align::Center);
    texts.set_hexpand(true);
    texts.append(&title);
    texts.append(&artist);
    texts.append(&error);
    let open_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    open_box.append(&art);
    open_box.append(&texts);
    let open = gtk::Button::builder().child(&open_box).hexpand(true).build();
    open.add_css_class("mini-open");
    open.set_tooltip_text(Some(&tr("Open Now Playing")));
    open.connect_clicked(|_| window::open_now_playing(None));

    let glyph = PlayPause::new(24);
    let play = gtk::Button::builder()
        .child(&glyph)
        .valign(gtk::Align::Center)
        .build();
    play.add_css_class("mini-play");
    play.connect_clicked(|_| {
        app().send(Command::TogglePlayPause);
    });

    let next = gtk::Button::builder()
        .child(&Glyph::new("next", 20))
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Next"))
        .action_name("app.next")
        .build();
    next.add_css_class("mini-play");
    next.add_css_class("mini-next");
    next.update_property(&[gtk::accessible::Property::Label(&tr("Next"))]);

    let capsule = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    capsule.add_css_class("mini-player");
    capsule.add_css_class("veil");
    capsule.append(&open);
    capsule.append(&play);
    capsule.append(&next);
    let progress = ProgressLine::new(CAPSULE_RADIUS);
    let framed = gtk::Overlay::new();
    framed.set_child(Some(&capsule));
    framed.add_overlay(&progress);
    framed.add_css_class("mini-frame");
    let clamp = adw::Clamp::builder()
        .child(&framed)
        .maximum_size(560)
        .valign(gtk::Align::End)
        .build();
    clamp.add_css_class("mini-clamp");

    let update = {
        let (title, artist, art, play, glyph, progress, error) = (
            title.downgrade(),
            artist.downgrade(),
            art.downgrade(),
            play.downgrade(),
            glyph.downgrade(),
            progress.downgrade(),
            error.downgrade(),
        );
        move || {
            let (Some(title), Some(artist), Some(art), Some(play), Some(glyph), Some(progress), Some(error)) = (
                title.upgrade(),
                artist.upgrade(),
                art.upgrade(),
                play.upgrade(),
                glyph.upgrade(),
                progress.upgrade(),
                error.upgrade(),
            ) else {
                return;
            };
            let a = app();
            let now = a.now.borrow().clone();
            let remote = a.remote_view();
            error.set_label(
                remote
                    .as_ref()
                    .and_then(|v| v.error.as_deref())
                    .unwrap_or_default(),
            );
            error.set_visible(remote.as_ref().is_some_and(|v| v.error.is_some()));
            if remote.is_some() {
                artist.add_css_class("playing-on");
            } else {
                artist.remove_css_class("playing-on");
            }
            match (&remote, &now) {
                (Some(view), _) => {
                    let (name, playing_on) = convert::remote_lines(view);
                    title.set_label(&name);
                    artist.set_label(&playing_on);
                    art.set_art(convert::art_of(view.artwork.as_ref()));
                }
                (None, Some(np)) => {
                    title.set_label(&np.title);
                    artist.set_label(&np.artist.clone().unwrap_or_else(convert::unknown_artist));
                    art.set_art(convert::art_of(np.artwork.as_ref()));
                }
                (None, None) => {
                    title.set_label(&tr("Ready to play"));
                    artist.set_label(&tr("Press play to resume"));
                    art.set_art(None);
                }
            }
            let playing = a.is_playing();
            glyph.set_playing(playing);
            let percent = (progress.fraction() * 100.0).round() as u32;
            let label = if playing {
                trf("Pause, {percent} % played", &[("percent", &percent.to_string())])
            } else {
                tr("Play")
            };
            play.set_tooltip_text(Some(&if playing { tr("Pause") } else { tr("Play") }));
            play.update_property(&[gtk::accessible::Property::Label(&label)]);
        }
    };
    update();
    let a = app();
    let u = update.clone();
    a.listen_for(&clamp, move |change| {
        if matches!(
            change,
            Change::NowPlaying | Change::Playback | Change::Queue | Change::Remote
        ) {
            u();
        }
    });
    a.scheduler.add(
        &progress,
        |line, pos, _running| line.set_position(pos, app().duration_us()),
        |line, pos| line.next_change(pos),
    );
    clamp.upcast()
}
