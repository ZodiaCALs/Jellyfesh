//! Desktop Lyrics (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): the
//! line being sung, filling word by word in the highlight colour, and the
//! next line under it, in a small undecorated window of their own, opened
//! and closed by the Desktop Lyrics setting. Before the first line and
//! without timed lyrics it shows the song's title and artist, with nothing
//! playing "Nothing Playing". As on macOS the words are light ink with a
//! soft shadow, so they read over any desktop. Pointing at it shows a dark
//! backdrop with Lock and Close; locked, clicks pass through it (unlock
//! from Now Playing's More); it is dragged anywhere by its body.
//!
//! GTK 4 has no "keep above" for windows, so on X11 the window asks the
//! window manager for `_NET_WM_STATE_ABOVE` itself (EWMH; its XID through
//! gdk4-x11, the message through x11rb). Wayland lets no client place
//! itself above others: there it is a normal window (docs/ui.md, "Allowed
//! differences").

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::lyricsbar::{LyricsBarView, Mode};
use crate::ui::state::{Change, app, try_app};

struct Desktop {
    window: gtk::Window,
    /// Clicks pass through while locked.
    locked: Cell<bool>,
    controls: gtk::Box,
    lock: gtk::Button,
    /// The title (or "Nothing Playing") and the artist, without a line.
    fallback_title: gtk::Label,
    fallback_artist: gtk::Label,
    /// The dark appearance's ink for the words.
    ink: gtk::CssProvider,
    /// Kept for as long as the window.
    _bar: Rc<LyricsBarView>,
}

thread_local! {
    static DESKTOP: RefCell<Option<Rc<Desktop>>> = const { RefCell::new(None) };
}

fn desktop() -> Option<Rc<Desktop>> {
    DESKTOP.with(|d| d.borrow().clone())
}

/// Opens or closes the window as the Desktop Lyrics setting says, now and
/// whenever it changes.
pub fn follow() {
    let apply = || {
        let wanted = try_app().is_some_and(|a| a.settings.borrow().desktop_lyrics);
        match (wanted, desktop().is_some()) {
            (true, false) => open(),
            (false, true) => close(),
            _ => {}
        }
    };
    apply();
    app().listen(move |change| {
        if matches!(change, Change::Settings) {
            apply();
        }
    });
}

/// Whether the window shows.
pub fn shown() -> bool {
    desktop().is_some()
}

/// Whether clicks pass through the window.
pub fn locked() -> bool {
    desktop().is_some_and(|d| d.locked.get())
}

/// Locks the window (clicks pass through it) or unlocks it.
pub fn set_locked(locked: bool) {
    let Some(d) = desktop() else { return };
    d.locked.set(locked);
    apply_lock(&d);
}

fn apply_lock(d: &Desktop) {
    let locked = d.locked.get();
    if let Some(surface) = d.window.surface() {
        if locked {
            surface.set_input_region(Some(&gtk::cairo::Region::create()));
        } else {
            surface.set_input_region(None);
        }
    }
    d.controls.set_visible(false);
    d.window.remove_css_class("hover");
    let label = if locked {
        tr("Unlock Desktop Lyrics")
    } else {
        tr("Lock Desktop Lyrics")
    };
    d.lock.set_tooltip_text(Some(&label));
    d.lock
        .update_property(&[gtk::accessible::Property::Label(&label)]);
    if locked {
        d.window.add_css_class("locked");
    } else {
        d.window.remove_css_class("locked");
    }
}

fn control(icon: &str, label: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.add_css_class("circular");
    b.add_css_class("desktop-lyrics-control");
    b.set_tooltip_text(Some(label));
    b.update_property(&[gtk::accessible::Property::Label(label)]);
    b
}

fn open() {
    let window = gtk::Window::builder()
        .title(tr("Desktop Lyrics"))
        .decorated(false)
        .default_width(760)
        .default_height(128)
        .build();
    window.add_css_class("desktop-lyrics");
    let bar = LyricsBarView::new(Mode::WithNext);
    let line = |class: &str| {
        let l = gtk::Label::builder()
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .build();
        l.add_css_class(class);
        l
    };
    let fallback_title = line("desktop-lyrics-title");
    let fallback_artist = line("desktop-lyrics-artist");
    let fallback = gtk::Box::new(gtk::Orientation::Vertical, 1);
    fallback.append(&fallback_title);
    fallback.append(&fallback_artist);
    let shown_fallback = fallback.downgrade();
    bar.connect_showing(move |showing| {
        if let Some(f) = shown_fallback.upgrade() {
            f.set_visible(!showing);
        }
    });
    let lines = gtk::Box::new(gtk::Orientation::Vertical, 0);
    lines.set_valign(gtk::Align::Center);
    lines.set_vexpand(true);
    lines.append(&bar.root);
    lines.append(&fallback);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("desktop-lyrics-body");
    body.append(&lines);
    // The body moves the window, as a title bar would.
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&body));
    let lock = control("changes-prevent-symbolic", &tr("Lock Desktop Lyrics"));
    let close_button = control("window-close-symbolic", &tr("Close Desktop Lyrics"));
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    controls.set_halign(gtk::Align::End);
    controls.set_valign(gtk::Align::Start);
    controls.set_margin_top(8);
    controls.set_margin_end(8);
    controls.append(&lock);
    controls.append(&close_button);
    controls.set_visible(false);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&handle));
    overlay.add_overlay(&controls);
    window.set_child(Some(&overlay));
    // The backdrop, Lock and Close show while the pointer is over the
    // window.
    let hover = gtk::EventControllerMotion::new();
    let (c, w) = (controls.downgrade(), window.downgrade());
    hover.connect_enter(move |_, _, _| {
        if let (Some(c), Some(w)) = (c.upgrade(), w.upgrade())
            && !locked()
        {
            c.set_visible(true);
            w.add_css_class("hover");
        }
    });
    let (c, w) = (controls.downgrade(), window.downgrade());
    hover.connect_leave(move |_| {
        if let (Some(c), Some(w)) = (c.upgrade(), w.upgrade()) {
            c.set_visible(false);
            w.remove_css_class("hover");
        }
    });
    window.add_controller(hover);
    lock.connect_clicked(|_| {
        // Through the app's action, so More's check item follows.
        if let Some(app) = gtk::gio::Application::default() {
            app.change_action_state("lock-desktop-lyrics", &(!locked()).to_variant());
        }
    });
    close_button.connect_clicked(|_| {
        app().update_settings(|s| s.desktop_lyrics = false);
    });
    // Closed by the window manager: the setting follows.
    window.connect_close_request(|_| {
        if let Some(a) = try_app() {
            a.update_settings(|s| s.desktop_lyrics = false);
        }
        glib::Propagation::Stop
    });
    window.connect_map(|w| {
        let w = w.clone();
        glib::idle_add_local_once(move || {
            keep_above(&w);
            if let Some(d) = desktop() {
                apply_lock(&d);
            }
        });
    });
    let ink = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&window),
        &ink,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    let d = Rc::new(Desktop {
        window: window.clone(),
        locked: Cell::new(false),
        controls,
        lock,
        fallback_title,
        fallback_artist,
        ink,
        _bar: bar,
    });
    DESKTOP.with(|slot| *slot.borrow_mut() = Some(d.clone()));
    let weak = Rc::downgrade(&d);
    app().listen_for(&window, move |change| {
        let Some(d) = weak.upgrade() else { return };
        match change {
            Change::NowPlaying | Change::Remote => update_fallback(&d),
            Change::Theme | Change::Settings | Change::Look => update_ink(&d),
            _ => {}
        }
    });
    update_fallback(&d);
    update_ink(&d);
    window.present();
}

/// The song's title and artist, or "Nothing Playing".
fn update_fallback(d: &Desktop) {
    let a = app();
    let (title, artist) = match (a.remote_view(), a.now.borrow().as_ref()) {
        (Some(view), _) => (convert::remote_lines(&view).0, view.artist.clone()),
        (None, Some(np)) => (np.title.clone(), np.artist.clone()),
        (None, None) => (tr("Nothing Playing"), None),
    };
    d.fallback_title.set_label(&title);
    d.fallback_artist.set_label(artist.as_deref().unwrap_or_default());
    d.fallback_artist
        .set_visible(artist.is_some_and(|a| !a.trim().is_empty()));
}

/// The words in the dark appearance's ink, whatever the shown one.
fn update_ink(d: &Desktop) {
    let colors = crate::ui::style::colors_for(&app(), true);
    let ink = |name: &str| {
        colors
            .get(name)
            .copied()
            .map_or_else(|| "white".to_owned(), crate::logic::theme::css_color)
    };
    let (sung, unsung) = (ink("ink.ink-90"), ink("ink.ink-70"));
    d.ink.load_from_string(&format!(
        "window.desktop-lyrics .lyrics-bar-text, window.desktop-lyrics .lyrics-bar-next, \
         window.desktop-lyrics .desktop-lyrics-artist {{ color: {unsung}; }}\n\
         window.desktop-lyrics .lyrics-bar-text.sung, window.desktop-lyrics .desktop-lyrics-title {{ color: {sung}; }}\n"
    ));
}

fn close() {
    if let Some(d) = DESKTOP.with(|slot| slot.borrow_mut().take()) {
        gtk::style_context_remove_provider_for_display(&WidgetExt::display(&d.window), &d.ink);
        d.window.destroy();
    }
    // A new window starts unlocked.
    if let Some(app) = gtk::gio::Application::default() {
        app.change_action_state("lock-desktop-lyrics", &false.to_variant());
    }
}

/// Asks an X11 window manager to keep `window` above other windows
/// (`_NET_WM_STATE_ABOVE`, EWMH); does nothing elsewhere (Wayland).
fn keep_above(window: &gtk::Window) {
    use gdk4_x11::X11Surface;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask};

    let Some(surface) = window.surface().and_downcast::<X11Surface>() else {
        return;
    };
    let xid = surface.xid();
    let display = WidgetExt::display(window).name();
    let ask = || -> Result<(), Box<dyn std::error::Error>> {
        let (conn, screen) = x11rb::connect(Some(display.as_str()))?;
        let root = conn.setup().roots[screen].root;
        let state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        // _NET_WM_STATE_ADD, the property, none, a normal application.
        let message = ClientMessageEvent::new(32, xid as u32, state, [1, above, 0, 1, 0]);
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        )?;
        conn.flush()?;
        Ok(())
    };
    if let Err(e) = ask() {
        tracing::warn!("Desktop Lyrics cannot stay above other windows: {e}");
    }
}
