//! Lyrics view decisions: which extra lines to show, where to scroll, how
//! each line is coloured, sized and scaled, and how the lines cascade when
//! the active one changes (design-controls.md, "Now Playing").

use jf_core::{AuxKind, Line, LyricDoc, LyricsAlign, LyricsSize};
use jf_tokens::lyrics as tokens;

use crate::logic::easing::CubicBezier;

/// Translation and romanization text to show under `line`, following the
/// Lyrics settings: romanization first (it reads the original aloud), then
/// the translation.
pub fn aux_lines(
    doc: &LyricDoc,
    line: &Line,
    translation: bool,
    romanization: bool,
) -> Vec<(AuxKind, String)> {
    let mut out = Vec::new();
    for (kind, wanted) in [
        (AuxKind::Romanization, romanization),
        (AuxKind::Translation, translation),
    ] {
        if !wanted {
            continue;
        }
        if let Some(text) = doc
            .track(kind)
            .and_then(|t| t.lines.get(&line.id))
            .map(|l| l.text.trim())
            .filter(|t| !t.is_empty())
        {
            out.push((kind, text.to_owned()));
        }
    }
    out
}

/// How a line is drawn relative to the active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineState {
    /// Sung already.
    Past,
    /// Being sung.
    Active,
    /// Not reached yet.
    Future,
}

/// The state of line `index` when `active` is the cursor's line. In a gap
/// between lines (no active line) every line is drawn inactive; lines before
/// `last_started` count as past.
pub fn line_state(index: usize, active: Option<usize>, last_started: Option<usize>) -> LineState {
    match active {
        Some(a) if a == index => LineState::Active,
        _ => match last_started {
            Some(s) if index <= s => LineState::Past,
            _ => LineState::Future,
        },
    }
}

/// The index of the last line that started at or before `t_ms`, which a
/// gap between lines keeps in place (the view stays where it was).
pub fn last_started(line_starts: &[u32], t_ms: u32) -> Option<usize> {
    line_starts.partition_point(|&s| s <= t_ms).checked_sub(1)
}

/// The scroll offset that puts the middle of a line at `anchor` (a fraction
/// of the viewport height, 0.38 in the tokens), within `0..=max_offset`.
pub fn scroll_target(line_top: f64, line_height: f64, viewport: f64, anchor: f64, max_offset: f64) -> f64 {
    let target = line_top + line_height / 2.0 - viewport * anchor;
    target.clamp(0.0, max_offset.max(0.0))
}

/// The factor the Lyrics size setting applies to every lyrics size
/// (`lyrics.scale-small`, 1, `lyrics.scale-large`).
pub fn size_scale(size: LyricsSize) -> f64 {
    match size {
        LyricsSize::Small => f64::from(tokens::SCALE_SMALL),
        LyricsSize::Medium => 1.0,
        LyricsSize::Large => f64::from(tokens::SCALE_LARGE),
    }
}

/// The lyrics column width (px) from which lines take the full
/// `lyrics.line` size: Now Playing's lyrics panel at its widest.
pub const FULL_SIZE_WIDTH: f64 = 640.0;

/// The lyric line size for a lyrics column `width` px wide: `lyrics.line`
/// (40 px in Luminous) from [`FULL_SIZE_WIDTH`], smaller in narrower
/// columns within `lyrics.line` fluid range (28 to 40 px), times the Lyrics
/// size, in whole pixels so text does not reflow on every resize step.
pub fn line_size(width: f64, size: LyricsSize) -> f64 {
    let fluid = tokens::LINE_FLUID;
    let full = f64::from(tokens::LINE.size);
    let base = (width * full / FULL_SIZE_WIDTH).clamp(f64::from(fluid.min), f64::from(fluid.max));
    (base * size_scale(size)).round()
}

/// The translation and romanization sizes for the Lyrics size, in whole
/// pixels (`lyrics.translation`, `lyrics.romanization`).
pub fn aux_sizes(size: LyricsSize) -> (f64, f64) {
    let s = size_scale(size);
    (
        (f64::from(tokens::TRANSLATION.size) * s).round(),
        (f64::from(tokens::ROMANIZATION.size) * s).round(),
    )
}

/// The scale a line is drawn at: the active line at full size, the others
/// at `lyrics.scale-inactive`.
pub fn line_scale(state: LineState) -> f64 {
    match state {
        LineState::Active => 1.0,
        LineState::Past | LineState::Future => f64::from(tokens::SCALE_INACTIVE),
    }
}

/// The x a line scales from: its start edge, or its centre when centred.
pub fn scale_origin(align: LyricsAlign, rtl: bool, width: f64) -> f64 {
    match (align, rtl) {
        (LyricsAlign::Center, _) => width / 2.0,
        (LyricsAlign::Start, false) => 0.0,
        (LyricsAlign::Start, true) => width,
    }
}

/// The cascade when the active line changes: the lines on screen move by
/// `distance` px (where the view scrolled), line `k` (counted from the top
/// of the screen) starting `k × stagger_ms` after the first, each over
/// `duration_ms` on `curve`. The distance line `k` still has to go
/// `elapsed_ms` after the change.
pub fn cascade_offset(
    k: usize,
    elapsed_ms: f64,
    duration_ms: f64,
    stagger_ms: f64,
    distance: f64,
    curve: &CubicBezier,
) -> f64 {
    let t = crate::logic::motion::delayed_progress(elapsed_ms, k as f64 * stagger_ms, duration_ms);
    distance * (1.0 - curve.ease(t))
}

/// How long a cascade over `lines` lines lasts.
pub fn cascade_ms(lines: usize, duration_ms: f64, stagger_ms: f64) -> f64 {
    duration_ms + lines.saturating_sub(1) as f64 * stagger_ms
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> LyricDoc {
        serde_json::from_value(json!({
            "language": "en", "timing": "line", "offset_ms": 0, "agents": [],
            "lines": [
                { "id": 0, "start_ms": 0, "end_ms": 1000, "agent": null, "text": "one",
                  "words": [], "background": [], "direction": "ltr" },
                { "id": 1, "start_ms": 1000, "end_ms": 2000, "agent": null, "text": "two",
                  "words": [], "background": [], "direction": "ltr" }
            ],
            "tracks": [
                { "kind": "translation", "language": "zh-Hans",
                  "lines": { "0": { "text": "一", "words": [] }, "1": { "text": " ", "words": [] } } },
                { "kind": "romanization", "language": "en-Latn",
                  "lines": { "0": { "text": "wan", "words": [] } } }
            ],
            "source": { "format": "lrc", "origin": null },
            "metadata": {}
        }))
        .unwrap()
    }

    #[test]
    fn shows_aux_lines_per_settings() {
        let d = doc();
        let first = &d.lines[0];
        assert_eq!(
            aux_lines(&d, first, true, true),
            vec![
                (AuxKind::Romanization, "wan".into()),
                (AuxKind::Translation, "一".into())
            ]
        );
        assert_eq!(
            aux_lines(&d, first, true, false),
            vec![(AuxKind::Translation, "一".into())]
        );
        assert!(aux_lines(&d, first, false, false).is_empty());
        // Blank translations are not shown.
        assert!(aux_lines(&d, &d.lines[1], true, true).is_empty());
    }

    #[test]
    fn line_states() {
        assert_eq!(line_state(2, Some(2), Some(2)), LineState::Active);
        assert_eq!(line_state(1, Some(2), Some(2)), LineState::Past);
        assert_eq!(line_state(3, Some(2), Some(2)), LineState::Future);
        // In a gap after line 2, line 2 is past and nothing is active.
        assert_eq!(line_state(2, None, Some(2)), LineState::Past);
        assert_eq!(line_state(0, None, None), LineState::Future);
        assert_eq!(last_started(&[1000, 2000, 3000], 999), None);
        assert_eq!(last_started(&[1000, 2000, 3000], 2000), Some(1));
        assert_eq!(last_started(&[1000, 2000, 3000], 9000), Some(2));
    }

    #[test]
    fn scrolls_the_line_to_the_anchor() {
        // A 40 px line at 500 px in a 600 px viewport: its middle (520) at 38 %.
        assert_eq!(scroll_target(500.0, 40.0, 600.0, 0.38, 10_000.0), 520.0 - 228.0);
        assert_eq!(scroll_target(10.0, 40.0, 600.0, 0.38, 10_000.0), 0.0);
        assert_eq!(scroll_target(5_000.0, 40.0, 600.0, 0.38, 1_000.0), 1_000.0);
        assert_eq!(scroll_target(5_000.0, 40.0, 600.0, 0.38, -5.0), 0.0);
    }

    #[test]
    fn line_size_is_fluid_within_the_tokens() {
        let (min, max) = (
            f64::from(tokens::LINE_FLUID.min),
            f64::from(tokens::LINE_FLUID.max),
        );
        // Luminous: 40 px lines from the panel's full width, never under
        // the fluid minimum (28 px) on phones.
        assert_eq!(
            line_size(FULL_SIZE_WIDTH, LyricsSize::Medium),
            f64::from(tokens::LINE.size)
        );
        assert_eq!(line_size(FULL_SIZE_WIDTH, LyricsSize::Medium), 40.0);
        assert_eq!(line_size(2_000.0, LyricsSize::Medium), max);
        assert_eq!(line_size(300.0, LyricsSize::Medium), min);
        assert_eq!(min, 28.0);
        assert_eq!(line_size(560.0, LyricsSize::Medium), 35.0);
    }

    #[test]
    fn lyrics_size_scales_every_size() {
        assert_eq!(size_scale(LyricsSize::Small), f64::from(0.8f32));
        assert_eq!(size_scale(LyricsSize::Large), f64::from(1.2f32));
        assert_eq!(line_size(FULL_SIZE_WIDTH, LyricsSize::Small), 32.0);
        assert_eq!(line_size(560.0, LyricsSize::Large), 42.0);
        assert_eq!(line_size(2_000.0, LyricsSize::Large), 48.0);
        assert_eq!(line_size(300.0, LyricsSize::Small), 22.0);
        assert_eq!(aux_sizes(LyricsSize::Medium), (18.0, 15.0));
        assert_eq!(aux_sizes(LyricsSize::Small), (14.0, 12.0));
        assert_eq!(aux_sizes(LyricsSize::Large), (22.0, 18.0));
    }

    #[test]
    fn inactive_lines_are_smaller_and_scale_from_their_start() {
        // Luminous: inactive lines at 92 % (`lyrics.scale-inactive`).
        let inactive = f64::from(tokens::SCALE_INACTIVE);
        assert_eq!(inactive, f64::from(0.92f32));
        assert_eq!(line_scale(LineState::Active), 1.0);
        assert_eq!(line_scale(LineState::Past), inactive);
        assert_eq!(line_scale(LineState::Future), inactive);
        assert_eq!(scale_origin(LyricsAlign::Start, false, 500.0), 0.0);
        assert_eq!(scale_origin(LyricsAlign::Start, true, 500.0), 500.0);
        assert_eq!(scale_origin(LyricsAlign::Center, false, 500.0), 250.0);
    }

    #[test]
    fn lines_settle_in_a_cascade() {
        let fluid = crate::logic::motion::FLUID;
        // The first line moves at once, the third waits 60 ms.
        assert_eq!(cascade_offset(0, 0.0, 380.0, 30.0, 90.0, &fluid), 90.0);
        assert!(cascade_offset(0, 30.0, 380.0, 30.0, 90.0, &fluid) < 90.0);
        assert_eq!(cascade_offset(2, 59.0, 380.0, 30.0, 90.0, &fluid), 90.0);
        assert!(cascade_offset(2, 100.0, 380.0, 30.0, 90.0, &fluid) < 90.0);
        // Every line arrives, and lower lines are never ahead of upper ones.
        let total = cascade_ms(8, 380.0, 30.0);
        assert_eq!(total, 380.0 + 7.0 * 30.0);
        for k in 0..8 {
            assert_eq!(cascade_offset(k, total, 380.0, 30.0, 90.0, &fluid), 0.0);
        }
        for ms in (0..700).step_by(10) {
            let t = f64::from(ms);
            for k in 1..8 {
                let above = cascade_offset(k - 1, t, 380.0, 30.0, 90.0, &fluid);
                let below = cascade_offset(k, t, 380.0, 30.0, 90.0, &fluid);
                assert!(below >= above - 1e-9, "line {k} at {ms} ms");
            }
        }
        assert_eq!(cascade_ms(0, 380.0, 30.0), 380.0);
    }
}
