//! Keyboard (docs/ui.md, "Keyboard"): Space plays and pauses, Escape
//! closes Now Playing or goes back. The other shared keys are application
//! accelerators (`ui::mod`); Settings › About lists them all (`ui::about`).

use gtk::glib;
use gtk::prelude::*;
use jf_core::Command;

use crate::ui::state::app;
use crate::ui::window;

/// Space toggles playback unless the focused widget used it (a text field,
/// a button); Escape closes Now Playing, or goes back from a detail page
/// when the focus is outside the pages (which go back on Escape themselves).
pub fn attach(window: &impl IsA<gtk::Widget>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Bubble);
    keys.connect_key_pressed(|_, key, _, modifiers| {
        let plain = !modifiers.intersects(
            gtk::gdk::ModifierType::CONTROL_MASK
                | gtk::gdk::ModifierType::ALT_MASK
                | gtk::gdk::ModifierType::SUPER_MASK,
        );
        match key {
            gtk::gdk::Key::space if plain => {
                app().send(Command::TogglePlayPause);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Escape if window::now_playing_open() => {
                window::close_now_playing();
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Escape if plain && window::go_back() => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        }
    });
    window.as_ref().add_controller(keys);
}
