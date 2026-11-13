//! View logic that is not a widget: plain Rust, unit-tested on every
//! platform. The GTK code in `ui` calls into these modules and only turns
//! their results into widgets.
//!
//! Nothing here sorts, filters, matches or persists library data: that stays
//! in the core (ADR-0001). These modules decide how to *show* what the core
//! reports: time formats, slider and player geometry, motion plans, redraw
//! times, page windows and settings forms.

pub mod appearance;
pub mod easing;
pub mod eq;
pub mod format;
pub mod hue;
pub mod layout;
pub mod library;
pub mod lru;
pub mod lyrics;
pub mod motion;
pub mod paging;
pub mod queue;
pub mod schedule;
pub mod schema;
pub mod search;
pub mod seek;
pub mod sweep;
pub mod theme;
pub mod tokens_css;
