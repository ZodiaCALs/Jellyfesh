//! The line sliders (seek bar and volume): geometry, thickness by state,
//! hit-testing and keyboard steps (design-controls.md, "v0.0.4: the controls
//! as built"). The widgets in `ui::widgets::seekbar` only draw what these
//! functions decide.

use jf_tokens::player;

/// Keyboard and media-key seek step (Left and Right).
pub const SEEK_STEP_MS: u64 = 5_000;
/// Larger seek step (Page Up and Page Down).
pub const SEEK_PAGE_MS: u64 = 30_000;
/// Volume step for the arrow keys, and the larger one for Page Up and Down.
pub const VOLUME_STEP: f64 = 0.05;
pub const VOLUME_PAGE: f64 = 0.10;

/// Room for the time labels under the line: the line's band, the gap and
/// the labels' line (12 px figures at 1.45).
pub const LINE_BAND: f64 = 20.0;
pub const TIMES_GAP: f64 = 2.0;
/// Space between a time label and the line when the times sit beside it.
pub const BESIDE_GAP: f64 = 10.0;

/// The fraction of the track played, 0 to 1.
pub fn fraction(position_us: i64, duration_us: Option<i64>) -> f64 {
    match duration_us {
        Some(d) if d > 0 => (position_us as f64 / d as f64).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

/// A keyboard seek from `position_ms` by `delta_ms`, kept in the track.
pub fn step(position_ms: u64, delta_ms: i64, duration_ms: u64) -> u64 {
    let target = i128::from(position_ms) + i128::from(delta_ms);
    target.clamp(0, i128::from(duration_ms)) as u64
}

/// How the person is using a line slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// Neither hovered, focused nor dragged.
    Rest,
    /// Hovered or focused.
    Hover,
    /// Being dragged.
    Drag,
}

/// The kind of pointer that last used the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Input {
    #[default]
    Pointer,
    Touch,
}

/// The line's thickness in px (player tokens): 4 at rest, 6 hovered or
/// focused, 8 dragged; on touch screens 6, and 12 while dragged.
pub fn thickness(state: Interaction, input: Input) -> f64 {
    let px = match (input, state) {
        (Input::Pointer, Interaction::Rest) => player::SCRUBBER_TRACK,
        (Input::Pointer, Interaction::Hover) => player::SCRUBBER_TRACK_HOVER,
        (Input::Pointer, Interaction::Drag) => player::SCRUBBER_TRACK_DRAG,
        (Input::Touch, Interaction::Drag) => player::SCRUBBER_TRACK_TOUCH_DRAG,
        (Input::Touch, _) => player::SCRUBBER_TRACK_TOUCH,
    };
    f64::from(px)
}

/// Whether the thumb shows: hovered, focused or dragged with a pointer
/// (after Media Player); never at rest, never on touch (after Apple Music).
pub fn thumb_shown(state: Interaction, input: Input) -> bool {
    input == Input::Pointer && state != Interaction::Rest
}

/// The thumb's diameter in px.
pub fn thumb_size() -> f64 {
    f64::from(player::SCRUBBER_THUMB)
}

/// Half the widest thing drawn at an end of the line (the thumb, or the
/// thickest round cap): the line keeps this far from the widget's sides.
pub fn end_inset() -> f64 {
    let thickest = thickness(Interaction::Drag, Input::Touch);
    (thumb_size().max(thickest) / 2.0).ceil() + 1.0
}

/// A straight line: from `x0` to `x1` (the centres of its round caps) at
/// height `y`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
}

impl Line {
    pub fn length(&self) -> f64 {
        (self.x1 - self.x0).max(0.0)
    }

    /// The x of the played part's end for `fraction`.
    pub fn x_at(&self, fraction: f64) -> f64 {
        self.x0 + self.length() * fraction.clamp(0.0, 1.0)
    }

    /// The fraction under the pointer at `x`.
    pub fn fraction_at(&self, x: f64) -> f64 {
        let len = self.length();
        if len <= 0.0 {
            return 0.0;
        }
        ((x - self.x0) / len).clamp(0.0, 1.0)
    }
}

/// Where the times go: under the line's two ends (Now Playing), or beside
/// them on the same row (the player bar, after Media Player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Times {
    None,
    Below,
    Beside,
}

/// The line and the time labels' boxes in a widget `width` × `height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeekLayout {
    pub line: Line,
    /// Left edge and top of the elapsed time.
    pub elapsed: (f64, f64),
    /// Right edge and top of the remaining time.
    pub remaining: (f64, f64),
}

/// Lays out a seek bar: `time_width` is the widest time label for the
/// track's length, `text_height` the labels' line height.
pub fn seek_layout(width: f64, height: f64, times: Times, time_width: f64, text_height: f64) -> SeekLayout {
    let inset = end_inset();
    match times {
        Times::None => {
            let line = Line {
                x0: inset,
                x1: (width - inset).max(inset),
                y: height / 2.0,
            };
            SeekLayout {
                line,
                elapsed: (0.0, 0.0),
                remaining: (width, 0.0),
            }
        }
        Times::Below => {
            let y = LINE_BAND / 2.0;
            let line = Line {
                x0: inset,
                x1: (width - inset).max(inset),
                y,
            };
            // The times line up with the ends of the line's round caps.
            let cap = thickness(Interaction::Rest, Input::Pointer) / 2.0;
            let top = LINE_BAND + TIMES_GAP;
            SeekLayout {
                line,
                elapsed: (line.x0 - cap, top),
                remaining: (line.x1 + cap, top),
            }
        }
        Times::Beside => {
            let x0 = time_width + BESIDE_GAP + inset;
            let x1 = (width - time_width - BESIDE_GAP - inset).max(x0);
            let top = ((height - text_height) / 2.0).max(0.0);
            SeekLayout {
                line: Line {
                    x0,
                    x1,
                    y: height / 2.0,
                },
                elapsed: (0.0, top),
                remaining: (width, top),
            }
        }
    }
}

/// The height a seek bar asks for.
pub fn seek_height(times: Times, text_height: f64) -> f64 {
    let band = thumb_size().max(thickness(Interaction::Drag, Input::Touch)) + 8.0;
    match times {
        Times::None | Times::Beside => band.max(text_height),
        Times::Below => LINE_BAND + TIMES_GAP + text_height,
    }
}

/// A key on a focused slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderKey {
    Back,
    Forward,
    PageBack,
    PageForward,
    Home,
    End,
}

/// The seek target for a key: ±5 s, ±30 s, the start, or a second before
/// the end (so End does not skip to the next song).
pub fn seek_key(key: SliderKey, position_ms: u64, duration_ms: u64) -> u64 {
    let (step_ms, page_ms) = (SEEK_STEP_MS as i64, SEEK_PAGE_MS as i64);
    match key {
        SliderKey::Back => step(position_ms, -step_ms, duration_ms),
        SliderKey::Forward => step(position_ms, step_ms, duration_ms),
        SliderKey::PageBack => step(position_ms, -page_ms, duration_ms),
        SliderKey::PageForward => step(position_ms, page_ms, duration_ms),
        SliderKey::Home => 0,
        SliderKey::End => duration_ms.saturating_sub(1_000),
    }
}

/// The volume (0 to 1) for a key: ±5 %, ±10 %, silent or full.
pub fn volume_key(key: SliderKey, volume: f64) -> f64 {
    let target = match key {
        SliderKey::Back => volume - VOLUME_STEP,
        SliderKey::Forward => volume + VOLUME_STEP,
        SliderKey::PageBack => volume - VOLUME_PAGE,
        SliderKey::PageForward => volume + VOLUME_PAGE,
        SliderKey::Home => 0.0,
        SliderKey::End => 1.0,
    };
    // Whole percents, so repeated steps land on 0 and 1 exactly.
    (target.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

/// A volume as a whole percentage, for screen readers.
pub fn percent(volume: f64) -> u32 {
    (volume.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// `a` towards `b` by `t` (0 to 1): thickness and thumb size between states.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_and_positions() {
        assert_eq!(fraction(84_000_000, Some(232_000_000)), 84.0 / 232.0);
        assert_eq!(fraction(1, None), 0.0);
        assert_eq!(fraction(-5, Some(10)), 0.0);
        assert_eq!(fraction(50, Some(10)), 1.0);
    }

    #[test]
    fn keyboard_steps_stay_in_the_track() {
        assert_eq!(step(10_000, 5_000, 60_000), 15_000);
        assert_eq!(step(2_000, -5_000, 60_000), 0);
        assert_eq!(step(58_000, 5_000, 60_000), 60_000);
        assert_eq!(seek_key(SliderKey::Forward, 10_000, 60_000), 15_000);
        assert_eq!(seek_key(SliderKey::Back, 10_000, 60_000), 5_000);
        assert_eq!(seek_key(SliderKey::PageForward, 10_000, 60_000), 40_000);
        assert_eq!(seek_key(SliderKey::PageBack, 10_000, 60_000), 0);
        assert_eq!(seek_key(SliderKey::Home, 10_000, 60_000), 0);
        assert_eq!(seek_key(SliderKey::End, 10_000, 60_000), 59_000);
        assert_eq!(seek_key(SliderKey::End, 0, 500), 0);
    }

    #[test]
    fn volume_keys_step_in_whole_percents() {
        assert_eq!(volume_key(SliderKey::Forward, 0.5), 0.55);
        assert_eq!(volume_key(SliderKey::Back, 0.03), 0.0);
        assert_eq!(volume_key(SliderKey::PageForward, 0.95), 1.0);
        assert_eq!(volume_key(SliderKey::PageBack, 0.5), 0.4);
        assert_eq!(volume_key(SliderKey::Home, 0.5), 0.0);
        assert_eq!(volume_key(SliderKey::End, 0.5), 1.0);
        let mut v = 0.0;
        for _ in 0..20 {
            v = volume_key(SliderKey::Forward, v);
        }
        assert_eq!(v, 1.0);
        assert_eq!(percent(0.554), 55);
        assert_eq!(percent(2.0), 100);
    }

    #[test]
    fn thickness_follows_the_tokens() {
        assert_eq!(thickness(Interaction::Rest, Input::Pointer), 4.0);
        assert_eq!(thickness(Interaction::Hover, Input::Pointer), 6.0);
        assert_eq!(thickness(Interaction::Drag, Input::Pointer), 8.0);
        assert_eq!(thickness(Interaction::Rest, Input::Touch), 6.0);
        assert_eq!(thickness(Interaction::Hover, Input::Touch), 6.0);
        assert_eq!(thickness(Interaction::Drag, Input::Touch), 12.0);
        assert_eq!(thumb_size(), 14.0);
    }

    #[test]
    fn the_thumb_shows_only_for_pointers_in_use() {
        assert!(!thumb_shown(Interaction::Rest, Input::Pointer));
        assert!(thumb_shown(Interaction::Hover, Input::Pointer));
        assert!(thumb_shown(Interaction::Drag, Input::Pointer));
        assert!(!thumb_shown(Interaction::Drag, Input::Touch));
        assert!(!thumb_shown(Interaction::Hover, Input::Touch));
    }

    #[test]
    fn the_line_keeps_its_ends_inside_the_widget() {
        // Half the widest end (the 14 px thumb), plus a pixel.
        assert_eq!(end_inset(), 8.0);
        let layout = seek_layout(300.0, 40.0, Times::None, 0.0, 16.0);
        assert_eq!(layout.line.x0, 8.0);
        assert_eq!(layout.line.x1, 292.0);
        assert_eq!(layout.line.y, 20.0);
        // Too narrow: the line has no length, never a negative one.
        let tiny = seek_layout(10.0, 40.0, Times::None, 0.0, 16.0);
        assert_eq!(tiny.line.length(), 0.0);
        assert_eq!(tiny.line.fraction_at(5.0), 0.0);
    }

    #[test]
    fn hit_testing_matches_drawing() {
        let line = Line {
            x0: 10.0,
            x1: 110.0,
            y: 5.0,
        };
        assert_eq!(line.x_at(0.25), 35.0);
        assert_eq!(line.fraction_at(35.0), 0.25);
        assert_eq!(line.fraction_at(-50.0), 0.0);
        assert_eq!(line.fraction_at(500.0), 1.0);
        assert_eq!(line.x_at(3.0), 110.0);
    }

    #[test]
    fn times_sit_under_the_ends_or_beside_them() {
        let below = seek_layout(400.0, 44.0, Times::Below, 30.0, 17.0);
        assert_eq!(below.line.y, LINE_BAND / 2.0);
        assert_eq!(below.elapsed, (below.line.x0 - 2.0, LINE_BAND + TIMES_GAP));
        assert_eq!(below.remaining, (below.line.x1 + 2.0, LINE_BAND + TIMES_GAP));
        let beside = seek_layout(400.0, 24.0, Times::Beside, 30.0, 16.0);
        assert_eq!(beside.line.x0, 30.0 + BESIDE_GAP + end_inset());
        assert_eq!(beside.line.x1, 400.0 - 30.0 - BESIDE_GAP - end_inset());
        assert_eq!(beside.line.y, 12.0);
        assert_eq!(beside.elapsed, (0.0, 4.0));
        assert_eq!(beside.remaining, (400.0, 4.0));
        assert_eq!(seek_height(Times::Below, 17.0), LINE_BAND + TIMES_GAP + 17.0);
        assert_eq!(seek_height(Times::Beside, 16.0), 22.0);
    }

    #[test]
    fn lerps_between_states() {
        assert_eq!(lerp(4.0, 6.0, 0.5), 5.0);
        assert_eq!(lerp(4.0, 6.0, 2.0), 6.0);
        assert_eq!(lerp(4.0, 6.0, -1.0), 4.0);
    }
}
