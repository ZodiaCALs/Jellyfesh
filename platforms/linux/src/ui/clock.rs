//! Drawing time (architecture §5): one timer for every time-driven view.
//!
//! Views that show playback time (Now Playing's and the player bar's seek
//! bars and their times, the mini-player's progress line, the lyrics)
//! register here. On every anchor, and when a view appears or disappears,
//! the scheduler draws each visible view once and asks when it next changes;
//! it then sleeps until the earliest of those moments
//! (`DisplayClock::wait_until`). While paused, with no visible time-driven
//! view, or with the window hidden, no timer runs at all; of the capsule and
//! the bar only the one on screen is drawn. Views that animate continuously
//! (the word sweep, the playing row's level glyph) run a frame clock only
//! while they actually move.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use jf_core::monotonic_now_ns;

use crate::ui::state::try_app;

/// Never wake sooner than this, even when a change is due now.
const MIN_WAIT: Duration = Duration::from_millis(2);
/// Wake this much after a change is due, so the draw lands past it.
const WAKE_LATE: Duration = Duration::from_millis(1);

enum Op {
    Live,
    Draw(i64, bool),
    Next(i64),
}

enum Reply {
    Dead,
    Live(bool),
    Next(Option<i64>),
    Done,
}

type ViewFn = Box<dyn Fn(Op) -> Reply>;

#[derive(Default)]
pub struct Scheduler {
    views: RefCell<Vec<ViewFn>>,
    timer: RefCell<Option<glib::SourceId>>,
    /// False while the window is minimised or not visible.
    window_live: Cell<bool>,
    window_known: Cell<bool>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish_non_exhaustive()
    }
}

impl Scheduler {
    /// Registers a time-driven view. `draw` updates it for a position (and
    /// whether playback runs); `next` tells when it next changes. Both are
    /// only called while the widget is mapped and the window is shown.
    pub fn add<W: IsA<gtk::Widget>>(
        &self,
        widget: &W,
        draw: impl Fn(&W, i64, bool) + 'static,
        next: impl Fn(&W, i64) -> Option<i64> + 'static,
    ) {
        let weak = widget.downgrade();
        self.views.borrow_mut().push(Box::new(move |op| {
            let Some(w) = weak.upgrade() else {
                return Reply::Dead;
            };
            match op {
                Op::Live => Reply::Live(w.is_mapped()),
                Op::Draw(pos, running) => {
                    draw(&w, pos, running);
                    Reply::Done
                }
                Op::Next(pos) => Reply::Next(next(&w, pos)),
            }
        }));
        widget.connect_map(|_| refresh_soon());
        widget.connect_unmap(|_| refresh_soon());
    }

    /// Tells the scheduler whether the window can be seen.
    pub fn set_window_live(&self, live: bool) {
        let changed = !self.window_known.get() || self.window_live.get() != live;
        self.window_known.set(true);
        self.window_live.set(live);
        if changed {
            self.refresh();
        }
    }

    fn window_live(&self) -> bool {
        !self.window_known.get() || self.window_live.get()
    }

    /// Draws every visible view now and schedules the next wake-up.
    pub fn refresh(&self) {
        if let Some(id) = self.timer.borrow_mut().take() {
            id.remove();
        }
        let Some(app) = try_app() else { return };
        if !self.window_live() {
            return;
        }
        let now = monotonic_now_ns();
        // The remote session's clock while one runs (ADR-0018).
        let (pos, running) = {
            let mut clock = app.display_clock();
            (clock.position_us(now), clock.is_running())
        };
        let mut next: Option<i64> = None;
        let mut dead = false;
        {
            let views = self.views.borrow();
            for view in views.iter() {
                match view(Op::Live) {
                    Reply::Dead => {
                        dead = true;
                        continue;
                    }
                    Reply::Live(false) => continue,
                    _ => {}
                }
                view(Op::Draw(pos, running));
                if running && let Reply::Next(Some(t)) = view(Op::Next(pos)) {
                    next = Some(next.map_or(t, |n| n.min(t)));
                }
            }
        }
        if dead {
            self.views
                .borrow_mut()
                .retain(|v| !matches!(v(Op::Live), Reply::Dead));
        }
        let Some(target) = next else { return };
        let Some(wait) = app.display_clock().wait_until(target, now) else {
            return;
        };
        let id = glib::timeout_add_local_once(wait.max(MIN_WAIT) + WAKE_LATE, || {
            if let Some(app) = try_app() {
                app.scheduler.timer.borrow_mut().take();
                app.scheduler.refresh();
            }
        });
        *self.timer.borrow_mut() = Some(id);
    }
}

/// Refreshes on the next main-loop turn (map and unmap happen mid-layout).
pub fn refresh_soon() {
    glib::idle_add_local_once(|| {
        if let Some(app) = try_app() {
            app.scheduler.refresh();
        }
    });
}

/// Converts a frame clock time (µs, `g_get_monotonic_time`) to the core's
/// clock (`monotonic_now_ns`). Both read CLOCK_MONOTONIC; the offset is
/// measured once.
pub fn frame_time_to_core_ns(frame_time_us: i64) -> u64 {
    thread_local! {
        static OFFSET: i128 = i128::from(glib::monotonic_time()) * 1_000 - i128::from(monotonic_now_ns());
    }
    let offset = OFFSET.with(|o| *o);
    u64::try_from((i128::from(frame_time_us) * 1_000 - offset).max(0)).unwrap_or(0)
}
