//! Jellyfesh for Linux: a GTK 4 and libadwaita app over the `jf-core` facade.
//!
//! `logic` holds the view logic that is not a widget (plain Rust, tested on
//! every platform); `ui` holds the GTK app and exists only on Linux.

// Elsewhere only the tests use the view logic.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod logic;
#[cfg(target_os = "linux")]
mod ui;

#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    ui::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Jellyfesh's GTK app runs on Linux; use the jf command-line player here.");
}
