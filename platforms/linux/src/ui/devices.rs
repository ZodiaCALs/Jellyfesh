//! Sound-card hotplug. The core lists output devices only when asked, so
//! the app asks again when the kernel's playback devices come or go (a USB
//! DAC plugged in or out); that also lets playback follow a new default.
//! Changes inside the sound server (a Bluetooth headset, a new default
//! sink) do not show here: opening the device chip refreshes the list too.
//! The watch is an inotify watch, so it costs nothing while nothing changes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use jf_core::Command;

use crate::ui::state::try_app;

/// Where the kernel puts sound device nodes.
const SOUND_DEVICES: &str = "/dev/snd";

/// Hotplug comes as a burst of node events: one refresh once it settles.
const SETTLE: Duration = Duration::from_millis(500);

thread_local! {
    static MONITOR: RefCell<Option<gio::FileMonitor>> = const { RefCell::new(None) };
}

/// Whether a node in `/dev/snd` is a playback device (`pcmC1D0p`).
fn is_playback_node(name: &str) -> bool {
    name.starts_with("pcmC") && name.ends_with('p')
}

/// Starts watching; without the folder (no sound cards, a sandbox) it
/// waits for it to appear.
pub fn watch() {
    let dir = gio::File::for_path(SOUND_DEVICES);
    let monitor = match dir.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("not watching sound devices: {e}");
            return;
        }
    };
    let pending = Rc::new(Cell::new(false));
    monitor.connect_changed(move |_, file, _, event| {
        if !matches!(
            event,
            gio::FileMonitorEvent::Created | gio::FileMonitorEvent::Deleted
        ) {
            return;
        }
        let playback = file
            .basename()
            .and_then(|n| n.to_str().map(is_playback_node))
            .unwrap_or(false);
        if !playback || pending.replace(true) {
            return;
        }
        let pending = pending.clone();
        glib::timeout_add_local_once(SETTLE, move || {
            pending.set(false);
            if let Some(a) = try_app() {
                a.send(Command::RefreshDevices);
            }
        });
    });
    MONITOR.with(|m| *m.borrow_mut() = Some(monitor));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_playback_nodes_count() {
        assert!(is_playback_node("pcmC1D0p"));
        assert!(is_playback_node("pcmC0D3p"));
        assert!(!is_playback_node("pcmC1D0c"));
        assert!(!is_playback_node("controlC1"));
        assert!(!is_playback_node("seq"));
    }
}
