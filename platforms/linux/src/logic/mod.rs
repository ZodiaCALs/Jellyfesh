//! View logic that is not a widget: plain Rust, unit-tested on every
//! platform. The GTK code in `ui` calls into these modules and only turns
//! their results into widgets.
//!
//! Nothing here sorts, filters, matches or persists library data: that stays
//! in the core (ADR-0001). These modules decide how to *show* what the core
//! reports: time formats, slider and player geometry, motion plans, redraw
//! times, page windows, settings forms, and what the remote session's
//! slice and the items' links offer.

pub mod about;
pub mod ailyrics;
pub mod appearance;
pub mod browse;
pub mod build;
// WCAG checks of the Luminous field and glass, run by the tests.
pub mod comments;
#[cfg(test)]
pub mod contrast;
pub mod details;
pub mod discover;
pub mod downloads;
pub mod easing;
pub mod eq;
pub mod format;
pub mod hue;
pub mod layout;
pub mod library;
pub mod links;
pub mod look;
pub mod lru;
pub mod lyrics;
pub mod lyricsbar;
pub mod motion;
pub mod nav;
pub mod paging;
pub mod playback;
pub mod pro;
pub mod quality;
pub mod queue;
pub mod remote;
pub mod schedule;
pub mod schema;
pub mod search;
pub mod seek;
pub mod signin;
pub mod sourcelib;
pub mod sweep;
pub mod theme;
pub mod tokens_css;
