//! Keyboard (docs/ui.md, "Keyboard"): Space plays and pauses, Escape
//! closes Now Playing or goes back, and the shortcuts window lists every
//! shortcut. The other shared keys are application accelerators (`ui::mod`).

use gtk::glib;
use gtk::prelude::*;
use jf_core::Command;

use crate::ui::i18n::tr;
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

fn escape(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

/// The shortcuts window.
pub fn show_shortcuts() {
    let groups: [(String, Vec<(&str, String)>); 3] = [
        (
            tr("Playback"),
            vec![
                ("space", tr("Play or Pause")),
                ("<Primary>Right", tr("Next Song")),
                ("<Primary>Left", tr("Previous Song")),
                ("<Shift>Right", tr("Skip Forward 5 Seconds")),
                ("<Shift>Left", tr("Skip Back 5 Seconds")),
                ("<Primary>Up", tr("Volume Up")),
                ("<Primary>Down", tr("Volume Down")),
                ("<Primary>m", tr("Mute")),
                ("<Primary>s", tr("Shuffle")),
                ("<Primary>r", tr("Repeat")),
            ],
        ),
        (
            tr("Views"),
            vec![
                ("<Primary>p", tr("Now Playing")),
                ("<Primary>l", tr("Lyrics")),
                ("<Primary>u", tr("Queue")),
                ("<Primary>f", tr("Search")),
                ("Escape", tr("Back")),
            ],
        ),
        (
            tr("General"),
            vec![
                ("<Primary>comma", tr("Settings")),
                ("<Primary>question", tr("Keyboard Shortcuts")),
                ("<Primary>w", tr("Close Window")),
                ("<Primary>q", tr("Quit")),
            ],
        ),
    ];
    let mut xml = format!(
        "<interface><object class=\"GtkShortcutsWindow\" id=\"w\"><property name=\"modal\">1</property>\
         <property name=\"title\">{}</property>\
         <child><object class=\"GtkShortcutsSection\"><property name=\"section-name\">main</property>",
        escape(&tr("Keyboard Shortcuts"))
    );
    for (title, items) in &groups {
        xml.push_str(&format!(
            "<child><object class=\"GtkShortcutsGroup\"><property name=\"title\">{}</property>",
            escape(title)
        ));
        for (accel, label) in items {
            xml.push_str(&format!(
                "<child><object class=\"GtkShortcutsShortcut\"><property name=\"accelerator\">{}</property>\
                 <property name=\"title\">{}</property></object></child>",
                escape(accel),
                escape(label)
            ));
        }
        xml.push_str("</object></child>");
    }
    xml.push_str("</object></child></object></interface>");
    let builder = gtk::Builder::from_string(&xml);
    if let Some(w) = builder.object::<gtk::ShortcutsWindow>("w") {
        if let Some(s) = window::shell() {
            w.set_transient_for(Some(&s.window));
        }
        w.present();
    }
}
