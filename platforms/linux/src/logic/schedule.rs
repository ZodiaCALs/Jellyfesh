//! When time-driven views next change (architecture §5, "Drawing time
//! without waste").
//!
//! Every view that shows playback time reports the next position at which
//! what it draws changes: the next second of a `m:ss` label, the next pixel of
//! a progress bar, the next lyric line or word. The app sleeps until the
//! earliest of them (`DisplayClock::wait_until`), and runs a frame clock only
//! while something actually moves (a word sweep, a transition). Paused, or
//! with no visible time-driven view, nothing is scheduled at all.

use jf_core::LyricTimeline;
use jf_core::timing::{next_second_us, next_step_us};

/// Progress indicators never redraw more often than this, even when one
/// pixel stands for less time (short tracks on wide bars).
pub const MIN_BAR_STEP_US: i64 = 100_000;

/// The earliest of the given times.
pub fn earliest(times: impl IntoIterator<Item = Option<i64>>) -> Option<i64> {
    times.into_iter().flatten().min()
}

/// The time one pixel of a `length_px` long progress indicator stands for,
/// at least [`MIN_BAR_STEP_US`]. `None` without a duration or a length.
pub fn bar_step_us(duration_us: Option<i64>, length_px: f64) -> Option<i64> {
    let duration = duration_us.filter(|&d| d > 0)?;
    if length_px.is_nan() || length_px < 1.0 {
        return None;
    }
    let step = (duration as f64 / length_px).floor() as i64;
    Some(step.max(MIN_BAR_STEP_US))
}

/// [`bar_step_us`] for a bar `length_px` logical pixels long on a screen
/// with `scale` device pixels to the logical one: the bar redraws when its
/// fill moves by a device pixel (design-controls.md, "No loop animation").
pub fn bar_step_device_us(duration_us: Option<i64>, length_px: f64, scale: f64) -> Option<i64> {
    let scale = if scale.is_finite() { scale.max(1.0) } else { 1.0 };
    bar_step_us(duration_us, length_px * scale)
}

/// The step index a bar shows at `position_us`; a bar redraws only when it
/// changes.
pub fn bar_step_index(position_us: i64, step_us: i64) -> i64 {
    if step_us <= 0 {
        return 0;
    }
    position_us.max(0).div_euclid(step_us)
}

/// The next change of a bar drawn at `step_us` resolution, and of the
/// `m:ss` label beside it when `with_clock`.
pub fn next_bar_change(position_us: i64, step_us: Option<i64>, with_clock: bool) -> Option<i64> {
    earliest([
        step_us.and_then(|s| next_step_us(position_us, s)),
        with_clock.then(|| next_second_us(position_us)),
    ])
}

/// What the lyrics view needs next at `t_ms`: a frame for every vsync while
/// a word sweeps, otherwise a wake-up at the timeline's next change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Nothing changes until the next event.
    Idle,
    /// Sleep until this position (µs).
    At(i64),
    /// Draw every frame (a word sweep is running).
    Frames,
}

/// The lyrics plan at `t_ms`: frames while `sweeping`, else the next change.
pub fn lyrics_plan(timeline: &LyricTimeline, t_ms: u32, sweeping: bool) -> Plan {
    if sweeping {
        return Plan::Frames;
    }
    match timeline.next_change(t_ms) {
        Some(next) => Plan::At(i64::from(next) * 1_000),
        None => Plan::Idle,
    }
}

/// Milliseconds for the lyrics cursor from a drawn position in µs.
pub fn cursor_ms(position_us: i64) -> u32 {
    u32::try_from(position_us.max(0) / 1_000).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earliest_ignores_missing_times() {
        assert_eq!(earliest([None, Some(5), Some(3)]), Some(3));
        assert_eq!(earliest([None, None]), None);
    }

    #[test]
    fn bar_steps() {
        // A 4 min track on a 400 px bar moves one pixel every 0.6 s.
        assert_eq!(bar_step_us(Some(240_000_000), 400.0), Some(600_000));
        // A 10 s track on 600 px would step every 16 ms: held to 100 ms.
        assert_eq!(bar_step_us(Some(10_000_000), 600.0), Some(MIN_BAR_STEP_US));
        assert_eq!(bar_step_us(None, 400.0), None);
        assert_eq!(bar_step_us(Some(1), 0.0), None);
        assert_eq!(bar_step_us(Some(1_000_000), f64::NAN), None);
        // On a 2× screen the same bar moves a device pixel every 0.3 s.
        assert_eq!(bar_step_device_us(Some(240_000_000), 400.0, 2.0), Some(300_000));
        assert_eq!(bar_step_device_us(Some(240_000_000), 400.0, 0.5), Some(600_000));
        assert_eq!(
            bar_step_device_us(Some(240_000_000), 400.0, f64::NAN),
            Some(600_000)
        );
        assert_eq!(bar_step_index(1_250_000, 600_000), 2);
        assert_eq!(bar_step_index(-5, 600_000), 0);
        assert_eq!(bar_step_index(5, 0), 0);
    }

    #[test]
    fn next_bar_change_takes_the_label_into_account() {
        assert_eq!(next_bar_change(1_250_000, Some(600_000), true), Some(1_800_000));
        assert_eq!(next_bar_change(1_850_000, Some(600_000), true), Some(2_000_000));
        assert_eq!(next_bar_change(1_850_000, Some(600_000), false), Some(2_400_000));
        assert_eq!(next_bar_change(1_850_000, None, false), None);
    }

    #[test]
    fn lyrics_plans() {
        let tl = LyricTimeline {
            line_starts: vec![1_000],
            line_ends: vec![3_000],
            segment_starts: vec![1_000, 3_000],
            segment_lines: vec![0, u32::MAX], // u32::MAX: no line (NO_LINE)
            word_offsets: vec![0, 0],
            word_starts: vec![],
            word_ends: vec![],
        };
        assert_eq!(lyrics_plan(&tl, 0, false), Plan::At(1_000_000));
        assert_eq!(lyrics_plan(&tl, 1_500, false), Plan::At(3_000_000));
        assert_eq!(lyrics_plan(&tl, 3_500, false), Plan::Idle);
        assert_eq!(lyrics_plan(&tl, 1_500, true), Plan::Frames);
        assert_eq!(cursor_ms(-3), 0);
        assert_eq!(cursor_ms(1_999), 1);
    }
}
