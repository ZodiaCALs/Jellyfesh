//! The mini player (ADR-0026 §4; docs/ui.md, "The player bar and the mini
//! player"): a small undecorated window with the cover, the title and the
//! artist, previous, play and next, a progress line, and the lyrics bar's
//! line while Show Lyrics Line is on. It opens with the player bar's mini
//! player button or Open Mini Player (Ctrl+M) and closes from its own
//! button; the cover and the titles bring back the main window, on Now
//! Playing. It is dragged anywhere by its body.
//!
//! With Keep on Top it stays above other windows on X11
//! (`_NET_WM_STATE_ABOVE`, as Desktop Lyrics does). Wayland lets no app
//! keep its window above the others, so there it is a normal window, and
//! Keep on Top… opens the desktop's window menu, where Always on Top may be
//! offered (docs/ui.md, "Allowed differences").
//!
//! With Open When the Window Is Closed, closing the main window while
//! music plays hides it and leaves the mini player (`window.rs`); closing
//! the mini player then quits.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};
use jf_core::Command;

use crate::ui::convert;
use crate::ui::desktoplyrics::set_keep_above;
use crate::ui::i18n::tr;
use crate::ui::lyricsbar::{LyricsBarView, Mode};
use crate::ui::state::{Change, app, try_app};
use crate::ui::widgets::art::Art;
use crate::ui::widgets::glyph;
use crate::ui::widgets::playpause::PlayPause;
use crate::ui::widgets::progressline::ProgressLine;
use crate::ui::window;

/// The window's corners: square, so it reads the same with and without a
/// compositor; the progress line runs its whole width.
const RADIUS: f64 = 0.0;

struct Mini {
    window: gtk::Window,
    /// Keep on Top…: the desktop's window menu, where the window cannot
    /// keep itself above the others (Wayland).
    on_top: gtk::Button,
    /// Kept for as long as the window.
    _bar: Rc<LyricsBarView>,
}

thread_local! {
    static MINI: RefCell<Option<Rc<Mini>>> = const { RefCell::new(None) };
}

fn mini() -> Option<Rc<Mini>> {
    MINI.with(|m| m.borrow().clone())
}

/// Whether the mini player is open.
pub fn shown() -> bool {
    mini().is_some()
}

/// Open Mini Player, or closes it when it is open.
pub fn toggle() {
    if shown() { close() } else { open() }
}

fn label(class: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    l.add_css_class(class);
    l
}

pub fn open() {
    if let Some(m) = mini() {
        m.window.present();
        return;
    }
    let window = gtk::Window::builder()
        .title(tr("Mini Player"))
        .decorated(false)
        .resizable(false)
        .default_width(360)
        .build();
    window.add_css_class("mini-window");

    // The cover, the title and the artist, and the lyrics line.
    let art = Art::new(56, 12.0);
    art.set_size_request(56, 56);
    art.set_valign(gtk::Align::Center);
    let title = label("mini-window-title");
    let artist = label("mini-window-artist");
    let bar = LyricsBarView::new(Mode::Mini);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 1);
    texts.set_valign(gtk::Align::Center);
    texts.set_hexpand(true);
    texts.append(&title);
    texts.append(&artist);
    texts.append(&bar.root);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    top.add_css_class("mini-window-top");
    top.append(&art);
    top.append(&texts);
    top.set_tooltip_text(Some(&tr("Open Now Playing")));
    let back = gtk::GestureClick::new();
    back.connect_released(|_, _, _, _| window::present_now_playing());
    top.add_controller(back);

    // Previous, play and next, between Keep on Top… and Close.
    let previous = glyph::button("previous", 20, &tr("Previous"));
    previous.set_action_name(Some("app.previous"));
    previous.add_css_class("mini-window-button");
    let play_glyph = PlayPause::new(24);
    let play = gtk::Button::builder().child(&play_glyph).build();
    play.add_css_class("mini-window-play");
    play.connect_clicked(|_| {
        app().send(Command::TogglePlayPause);
    });
    let next = glyph::button("next", 20, &tr("Next"));
    next.set_action_name(Some("app.next"));
    next.add_css_class("mini-window-button");
    let transport = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    transport.set_halign(gtk::Align::Center);
    transport.set_hexpand(true);
    transport.append(&previous);
    transport.append(&play);
    transport.append(&next);
    let on_top = gtk::Button::from_icon_name("view-pin-symbolic");
    on_top.add_css_class("mini-window-button");
    on_top.set_tooltip_text(Some(&tr("Keep on Top…")));
    on_top.update_property(&[gtk::accessible::Property::Label(&tr("Keep on Top…"))]);
    on_top.set_visible(false);
    // The desktop's window menu needs the press that asks for it.
    let menu = gtk::GestureClick::new();
    menu.connect_pressed(|gesture, _, _, _| {
        let surface = gesture
            .widget()
            .and_then(|w| w.native())
            .and_then(|n| n.surface())
            .and_downcast::<gdk::Toplevel>();
        if let (Some(toplevel), Some(event)) = (surface, gesture.current_event()) {
            toplevel.show_window_menu(event);
        }
    });
    on_top.add_controller(menu);
    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.add_css_class("mini-window-button");
    close_button.set_tooltip_text(Some(&tr("Close Mini Player")));
    close_button.update_property(&[gtk::accessible::Property::Label(&tr("Close Mini Player"))]);
    close_button.connect_clicked(|_| close());
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    controls.append(&on_top);
    controls.append(&transport);
    controls.append(&close_button);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.add_css_class("mini-window-body");
    body.append(&top);
    body.append(&controls);
    // The body moves the window, as a title bar would.
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&body));
    let progress = ProgressLine::new(RADIUS);
    progress.set_can_target(false);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&handle));
    overlay.add_overlay(&progress);
    window.set_child(Some(&overlay));

    // Closed by the window manager: the same as Close.
    window.connect_close_request(|_| {
        close();
        glib::Propagation::Stop
    });
    // While it shows, the clock keeps time for it even with the main
    // window hidden or minimised.
    window.connect_map(|w| {
        if let Some(a) = try_app() {
            a.scheduler.set_extra_window(true);
        }
        let w = w.clone();
        glib::idle_add_local_once(move || apply_keep_on_top(&w));
    });
    window.connect_unmap(|_| {
        if let Some(a) = try_app() {
            a.scheduler.set_extra_window(false);
        }
    });

    let update = {
        let (title, artist, art, play, play_glyph) = (
            title.downgrade(),
            artist.downgrade(),
            art.downgrade(),
            play.downgrade(),
            play_glyph.downgrade(),
        );
        move || {
            let (Some(title), Some(artist), Some(art), Some(play), Some(play_glyph)) = (
                title.upgrade(),
                artist.upgrade(),
                art.upgrade(),
                play.upgrade(),
                play_glyph.upgrade(),
            ) else {
                return;
            };
            let a = app();
            let now = a.now.borrow().clone();
            match (a.remote_view(), now) {
                (Some(view), _) => {
                    let (name, playing_on) = convert::remote_lines(&view);
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
                    title.set_label(&tr("Nothing Playing"));
                    artist.set_label("");
                    art.set_art(None);
                }
            }
            let playing = a.is_playing();
            play_glyph.set_playing(playing);
            let name = if playing { tr("Pause") } else { tr("Play") };
            play.set_tooltip_text(Some(&name));
            play.update_property(&[gtk::accessible::Property::Label(&name)]);
        }
    };
    update();
    let a = app();
    let w = window.downgrade();
    a.listen_for(&window, move |change| match change {
        Change::NowPlaying | Change::Playback | Change::Queue | Change::Remote => update(),
        Change::Settings => {
            if let Some(w) = w.upgrade() {
                apply_keep_on_top(&w);
            }
        }
        _ => {}
    });
    a.scheduler.add(
        &progress,
        |line, pos, _running| line.set_position(pos, app().duration_us()),
        |line, pos| line.next_change(pos),
    );
    MINI.with(|m| {
        *m.borrow_mut() = Some(Rc::new(Mini {
            window: window.clone(),
            on_top,
            _bar: bar,
        }))
    });
    set_action_state(true);
    window.present();
}

/// Keep on Top: above other windows on X11; on Wayland Keep on Top… shows
/// while the setting is on.
fn apply_keep_on_top(window: &gtk::Window) {
    let on = try_app().is_some_and(|a| a.settings.borrow().mini_player.keep_on_top);
    let placed = set_keep_above(window, on);
    if let Some(m) = mini() {
        m.on_top.set_visible(on && !placed);
    }
}

pub fn close() {
    if let Some(m) = MINI.with(|m| m.borrow_mut().take()) {
        m.window.destroy();
    }
    set_action_state(false);
    // The main window hidden by Open When the Window Is Closed: nothing is
    // left on screen, so Jellyfesh quits.
    if window::shell().is_some_and(|s| !s.window.is_visible())
        && let Some(application) = gtk::gio::Application::default()
    {
        application.quit();
    }
}

/// The app's `mini-player` action carries whether it is open (the player
/// bar's button shows it pressed).
fn set_action_state(open: bool) {
    if let Some(application) = gtk::gio::Application::default()
        && let Some(action) = application
            .lookup_action("mini-player")
            .and_downcast::<gtk::gio::SimpleAction>()
    {
        action.set_state(&open.to_variant());
    }
}
