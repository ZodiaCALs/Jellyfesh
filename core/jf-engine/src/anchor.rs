//! The playback anchor (architecture §5) and the clock it is stamped with.

use std::sync::OnceLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// A point on the playback timeline that UIs extrapolate from, so they can draw progress and
/// lyrics every frame without asking the engine.
///
/// `position(now) = position_us + (now_mono_ns - anchor_mono_ns) * rate / 1000`
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaybackAnchor {
    /// Changes on every track change and seek; extrapolations across generations are invalid.
    pub track_generation: u64,
    /// Audible position in the current track at `anchor_mono_ns`, in microseconds.
    pub position_us: i64,
    /// When the position was measured, on [`monotonic_now_ns`]'s clock.
    pub anchor_mono_ns: u64,
    /// 1.0 while playing, 0.0 while paused, stopped or ended.
    pub rate: f32,
    /// Duration of the current track, when known.
    pub duration_us: Option<i64>,
}

impl PlaybackAnchor {
    /// The extrapolated audible position, in microseconds, at `now_mono_ns`.
    ///
    /// Times before the anchor extrapolate backwards; a paused anchor (rate 0) always returns
    /// `position_us`.
    pub fn position_at(&self, now_mono_ns: u64) -> i64 {
        if self.rate == 0.0 {
            return self.position_us;
        }
        let elapsed_ns = i128::from(now_mono_ns) - i128::from(self.anchor_mono_ns);
        let advance_us = (elapsed_ns as f64 * f64::from(self.rate) / 1000.0).round();
        // The cast saturates for out-of-range floats.
        self.position_us.saturating_add(advance_us as i64)
    }
}

/// Nanoseconds on the monotonic clock the engine stamps anchors with.
///
/// This is `std::time::Instant` (CLOCK_MONOTONIC on Linux and Android, CLOCK_UPTIME_RAW on
/// macOS, QueryPerformanceCounter on Windows: the clocks architecture §5 names), counted from
/// the first call in this process. In-process callers (the core, the CLI) read the same clock
/// through this function. Stamping with the raw platform counter, which out-of-process UIs need,
/// requires a small platform shim and comes with the FFI layer.
pub fn monotonic_now_ns() -> u64 {
    static BASE: OnceLock<Instant> = OnceLock::new();
    let base = *BASE.get_or_init(Instant::now);
    u64::try_from(base.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(position_us: i64, anchor_mono_ns: u64, rate: f32) -> PlaybackAnchor {
        PlaybackAnchor {
            track_generation: 1,
            position_us,
            anchor_mono_ns,
            rate,
            duration_us: None,
        }
    }

    #[test]
    fn playing_anchor_extrapolates_forward() {
        let a = anchor(1_000_000, 5_000_000_000, 1.0);
        assert_eq!(a.position_at(5_000_000_000), 1_000_000);
        assert_eq!(a.position_at(5_250_000_000), 1_250_000);
        assert_eq!(a.position_at(7_000_000_000), 3_000_000);
        // Sub-microsecond steps round to the nearest microsecond.
        assert_eq!(a.position_at(5_000_000_499), 1_000_000);
        assert_eq!(a.position_at(5_000_000_500), 1_000_001);
    }

    #[test]
    fn paused_anchor_holds_still() {
        let a = anchor(42_000, 1_000, 0.0);
        assert_eq!(a.position_at(1_000), 42_000);
        assert_eq!(a.position_at(u64::MAX), 42_000);
    }

    #[test]
    fn earlier_times_extrapolate_backwards() {
        let a = anchor(2_000_000, 3_000_000_000, 1.0);
        assert_eq!(a.position_at(2_000_000_000), 1_000_000);
        assert_eq!(a.position_at(0), -1_000_000);
    }

    #[test]
    fn other_rates_scale_elapsed_time() {
        let a = anchor(0, 0, 2.0);
        assert_eq!(a.position_at(1_000_000_000), 2_000_000);
        let half = anchor(0, 0, 0.5);
        assert_eq!(half.position_at(1_000_000_000), 500_000);
    }

    #[test]
    fn extreme_values_saturate() {
        let a = anchor(i64::MAX - 10, 0, 1.0);
        assert_eq!(a.position_at(u64::MAX), i64::MAX);
    }

    #[test]
    fn monotonic_clock_never_goes_backwards() {
        let a = monotonic_now_ns();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = monotonic_now_ns();
        assert!(b >= a + 1_000_000, "{a} -> {b}");
    }
}
