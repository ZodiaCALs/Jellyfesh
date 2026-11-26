//! The network the system reports, sent to the core (ADR-0025 §3, ADR-0026
//! §1) at start and on every change, so Auto quality and downloads follow
//! it. GIO's network monitor says whether a network is available and
//! whether it is metered (NetworkManager's metered flag; in the Flatpak,
//! the network portal's).

use std::cell::Cell;

use gtk::gio;
use gtk::gio::prelude::*;
use jf_core::Command;

use crate::ui::state::try_app;

thread_local! {
    /// The state last sent (online, metered).
    static SENT: Cell<Option<(bool, bool)>> = const { Cell::new(None) };
}

/// Sends the network state now and whenever it changes.
pub fn follow() {
    let monitor = gio::NetworkMonitor::default();
    send(&monitor);
    monitor.connect_network_changed(|m, _| send(m));
    monitor.connect_network_metered_notify(send);
}

fn send(monitor: &gio::NetworkMonitor) {
    let state = (monitor.is_network_available(), monitor.is_network_metered());
    if SENT.with(|s| s.replace(Some(state))) == Some(state) {
        return;
    }
    if let Some(app) = try_app() {
        let (online, metered) = state;
        app.send_quiet(Command::SetNetworkState { online, metered });
    }
}
