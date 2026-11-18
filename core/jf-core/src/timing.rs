//! Drawing time for UIs: a smoothed playback clock and redraw scheduling.
//!
//! Progress bars and lyrics are drawn from the [`PlaybackAnchor`] in the
//! playback slice, never by asking the core each frame. Two helpers make that
//! drawing smooth and cheap, and every shell uses them (or their port):
//!
//! * [`DisplayClock`] follows the anchors but never steps backwards while
//!   playing, and spreads the small corrections the engine makes when it
//!   re-anchors (drift over 15 ms) over a short slew instead of jumping. A
//!   progress bar or a word sweep therefore never jitters or un-sings a word.
//! * [`DisplayClock::wait_until`] turns "the next visible change is at
//!   position X" (the next second, the next step of a progress bar,
//!   [`jf_lyrics::LyricTimeline::next_change`]) into a wall-clock delay, so a
//!   view sleeps until something on screen actually changes instead of
//!   drawing every frame. That is what keeps Still mode cheap.
//! * [`lyrics_bar`] says what the one-line lyrics bar and Desktop Lyrics
//!   show at a time: the line, the next line and each word's progress
//!   (ADR-0022 §6). Shells in other languages port it and pass
//!   `tests/contract/lyrics-bar.json`.

use std::time::Duration;

use jf_engine::PlaybackAnchor;
use jf_lyrics::LyricTimeline;

/// Corrections up to this size are slewed; larger ones (a seek the engine did
/// not mark as one) jump.
pub const MAX_SLEW_US: i64 = 250_000;
/// Shortest slew. Longer corrections take four times their size, so the drawn
/// clock never runs more than 25 % fast or slow while it catches up.
pub const MIN_SLEW_US: i64 = 100_000;

/// A smoothed, monotonic playback position for drawing. See the
/// [module documentation](self).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DisplayClock {
    anchor: Option<PlaybackAnchor>,
    /// Correction added to the anchor's position, fading linearly to zero.
    offset_us: i64,
    offset_since_ns: u64,
    slew_us: i64,
    /// Last position handed out, so it never decreases while playing.
    last_us: Option<i64>,
}

impl DisplayClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds an anchor from the playback slice. A new track, a seek, a pause
    /// or a resume shows the new position at once; a small correction while
    /// playing is slewed.
    pub fn update(&mut self, anchor: PlaybackAnchor, now_ns: u64) {
        let continuous = self.anchor.is_some_and(|old| {
            old.track_generation == anchor.track_generation && old.rate > 0.0 && anchor.rate > 0.0
        });
        let shown = self.drawn(now_ns);
        self.anchor = Some(anchor);
        let error = shown - anchor.position_at(now_ns);
        if continuous && error != 0 && error.abs() <= MAX_SLEW_US {
            self.offset_us = error;
            self.offset_since_ns = now_ns;
            self.slew_us = (error.abs() * 4).max(MIN_SLEW_US);
        } else {
            // Show the new anchor exactly, backwards too.
            self.offset_us = 0;
            self.last_us = None;
        }
    }

    /// The position to draw at `now_ns`, in microseconds, within the track.
    pub fn position_us(&mut self, now_ns: u64) -> i64 {
        let Some(anchor) = self.anchor else {
            return 0;
        };
        let mut pos = self.drawn(now_ns);
        if anchor.rate > 0.0
            && let Some(last) = self.last_us
        {
            pos = pos.max(last);
        }
        if let Some(duration) = anchor.duration_us {
            pos = pos.min(duration.max(0));
        }
        pos = pos.max(0);
        self.last_us = Some(pos);
        pos
    }

    /// True while the position moves.
    pub fn is_running(&self) -> bool {
        self.anchor.is_some_and(|a| a.rate > 0.0)
    }

    /// The latest anchor.
    pub fn anchor(&self) -> Option<PlaybackAnchor> {
        self.anchor
    }

    /// How long until the drawn position reaches `target_us`: zero when it
    /// already has, `None` while stopped or paused. During a slew the result
    /// is off by up to a quarter, so a view re-asks when it wakes.
    pub fn wait_until(&self, target_us: i64, now_ns: u64) -> Option<Duration> {
        let anchor = self.anchor.filter(|a| a.rate > 0.0)?;
        let pos = self.drawn(now_ns).max(self.last_us.unwrap_or(i64::MIN));
        let ahead = target_us.saturating_sub(pos);
        if ahead <= 0 {
            return Some(Duration::ZERO);
        }
        Some(Duration::from_micros(
            (ahead as f64 / f64::from(anchor.rate)).ceil() as u64,
        ))
    }

    fn drawn(&self, now_ns: u64) -> i64 {
        let Some(anchor) = self.anchor else {
            return 0;
        };
        anchor
            .position_at(now_ns)
            .saturating_add(self.fading_offset(now_ns))
    }

    fn fading_offset(&self, now_ns: u64) -> i64 {
        if self.offset_us == 0 {
            return 0;
        }
        let elapsed_us =
            i64::try_from(now_ns.saturating_sub(self.offset_since_ns) / 1_000).unwrap_or(i64::MAX);
        if elapsed_us >= self.slew_us {
            0
        } else {
            self.offset_us * (self.slew_us - elapsed_us) / self.slew_us
        }
    }
}

/// The next whole second after `position_us`, when a `m:ss` display changes.
pub fn next_second_us(position_us: i64) -> i64 {
    (position_us.div_euclid(1_000_000) + 1) * 1_000_000
}

/// The next multiple of `step_us` after `position_us`, when a progress bar
/// drawn in steps of `step_us` changes. `None` for a step of zero.
pub fn next_step_us(position_us: i64, step_us: i64) -> Option<i64> {
    (step_us > 0).then(|| (position_us.div_euclid(step_us) + 1) * step_us)
}

/// What the lyrics bar shows at a time (see [`lyrics_bar`]).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LyricsBar {
    /// The line to show, as an index into `LyricDoc::lines`: the active line
    /// ([`LyricTimeline::cursor`]), or in a gap between lines the last one
    /// that started (it stays until the next one starts). `None` before the
    /// first line and for unsynced lyrics.
    pub line: Option<usize>,
    /// The line after it, shown dimmed or on a second row: the next line by
    /// start (before the first line, the first one).
    pub next: Option<usize>,
    /// How much of each word of `line` is sung, `0.0..=1.0`: 1 for words
    /// that ended, 0 for words still to come, the share of the word's time
    /// for the one being sung (1 once a zero-length word starts). Empty for
    /// lines without word timing.
    pub words: Vec<f32>,
    /// How much of `line` is over, `0.0..=1.0`, for lines without word
    /// timing: from its start to its end (or, without an end, the next
    /// line's start). 0 when neither is known, and in a gap after the line.
    pub line_progress: f32,
}

/// What the one-line lyrics bar (and Desktop Lyrics) shows at `t_ms`
/// (ADR-0022 §6): the line, the next line and each word's progress, from the
/// lyrics' [`LyricTimeline`]. Lines are in start order, as the timeline
/// keeps them. Costs a binary search and one pass over the line's words, so
/// it may run every frame while words are swept.
pub fn lyrics_bar(timeline: &LyricTimeline, t_ms: u32) -> LyricsBar {
    let timed = |i: usize| timeline.line_starts.get(i).is_some_and(|&s| s != u32::MAX);
    let count = timeline.line_starts.len();
    let cursor = timeline.cursor(t_ms);
    let active = cursor.line.is_some();
    let line = cursor.line.or_else(|| {
        // The last line that started, if any: it stays through the gap.
        let started = timeline.line_starts.partition_point(|&s| s <= t_ms);
        (0..started).rev().find(|&i| timed(i))
    });
    let next = match line {
        Some(l) => (l + 1..count).find(|&i| timed(i)),
        None => (0..count).find(|&i| timed(i)),
    };
    let Some(l) = line else {
        return LyricsBar {
            next,
            ..LyricsBar::default()
        };
    };
    let (from, to) = match (timeline.word_offsets.get(l), timeline.word_offsets.get(l + 1)) {
        (Some(&from), Some(&to)) => (from as usize, to as usize),
        _ => (0, 0),
    };
    let words: Vec<f32> = (from..to)
        .map(|w| {
            let start = timeline.word_starts[w];
            let end = timeline.word_ends[w].max(start);
            if t_ms < start {
                0.0
            } else if t_ms >= end {
                1.0
            } else {
                (t_ms - start) as f32 / (end - start) as f32
            }
        })
        .collect();
    let start = timeline.line_starts[l];
    let end = match timeline.line_ends.get(l) {
        Some(&end) if end != u32::MAX => Some(end),
        _ => next.map(|n| timeline.line_starts[n]),
    };
    let line_progress = match end {
        _ if !active && t_ms >= start => {
            // In the gap after the line: it is over.
            if words.is_empty() { 0.0 } else { 1.0 }
        }
        Some(end) if end > start => {
            ((t_ms.saturating_sub(start)) as f32 / (end - start) as f32).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    LyricsBar {
        line: Some(l),
        next,
        words,
        line_progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000; // nanoseconds

    fn anchor(generation: u64, position_ms: i64, at_ms: u64, rate: f32) -> PlaybackAnchor {
        PlaybackAnchor {
            track_generation: generation,
            position_us: position_ms * 1_000,
            anchor_mono_ns: at_ms * MS,
            rate,
            duration_us: Some(240_000_000),
        }
    }

    #[test]
    fn follows_the_anchor() {
        let mut c = DisplayClock::new();
        assert_eq!(c.position_us(0), 0);
        c.update(anchor(1, 10_000, 1_000, 1.0), 1_000 * MS);
        assert_eq!(c.position_us(1_500 * MS), 10_500_000);
        assert!(c.is_running());
    }

    #[test]
    fn small_corrections_are_slewed_not_jumped() {
        let mut c = DisplayClock::new();
        c.update(anchor(1, 0, 0, 1.0), 0);
        assert_eq!(c.position_us(1_000 * MS), 1_000_000);
        // The engine re-anchors 15 ms ahead of the extrapolation.
        c.update(anchor(1, 1_015, 1_000, 1.0), 1_000 * MS);
        assert_eq!(c.position_us(1_000 * MS), 1_000_000, "no jump at the correction");
        let mut last = 0;
        for t in (1_000..1_200).step_by(5) {
            let p = c.position_us(t * MS);
            assert!(p >= last);
            last = p;
        }
        // After the slew (100 ms for 15 ms) the drawn clock is the anchor's.
        assert_eq!(c.position_us(1_200 * MS), 1_215_000);
    }

    #[test]
    fn a_backward_correction_never_moves_backwards() {
        let mut c = DisplayClock::new();
        c.update(anchor(1, 0, 0, 1.0), 0);
        c.position_us(2_000 * MS);
        c.update(anchor(1, 1_980, 2_000, 1.0), 2_000 * MS); // 20 ms behind
        let mut last = c.position_us(2_000 * MS);
        for t in 2_000..2_200 {
            let p = c.position_us(t * MS);
            assert!(p >= last, "went back at {t} ms: {p} < {last}");
            last = p;
        }
        assert_eq!(c.position_us(2_200 * MS), 2_180_000);
    }

    #[test]
    fn seeks_pauses_and_new_tracks_show_at_once() {
        let mut c = DisplayClock::new();
        c.update(anchor(1, 0, 0, 1.0), 0);
        c.position_us(5_000 * MS);
        // Seek back: a new generation jumps immediately, backwards included.
        c.update(anchor(2, 1_000, 5_000, 1.0), 5_000 * MS);
        assert_eq!(c.position_us(5_000 * MS), 1_000_000);
        // Pause holds the position.
        c.update(anchor(2, 1_300, 5_300, 0.0), 5_300 * MS);
        assert_eq!(c.position_us(9_000 * MS), 1_300_000);
        assert!(!c.is_running());
        assert_eq!(c.wait_until(2_000_000, 9_000 * MS), None);
        // A big correction without a new generation still jumps.
        c.update(anchor(2, 1_300, 9_000, 1.0), 9_000 * MS);
        c.update(anchor(2, 3_000, 9_100, 1.0), 9_100 * MS);
        assert_eq!(c.position_us(9_100 * MS), 3_000_000);
    }

    #[test]
    fn stays_within_the_track() {
        let mut c = DisplayClock::new();
        c.update(anchor(1, 239_900, 0, 1.0), 0);
        assert_eq!(c.position_us(1_000 * MS), 240_000_000);
        c.update(anchor(2, -50, 0, 1.0), 0);
        assert_eq!(c.position_us(0), 0);
    }

    fn bar_timeline() -> LyricTimeline {
        // Invented lines for the test: word-timed, a gap, then a line
        // without words and without an end.
        let text = "[00:01.00]<00:01.00>one <00:01.50>two <00:02.00>\n\
                    [00:04.00]<00:04.00>three <00:04.40>four <00:05.00>\n\
                    [00:08.00]five six\n";
        jf_lyrics::parse(text, Some(jf_lyrics::Format::Lrc))
            .unwrap()
            .timeline()
    }

    #[test]
    fn the_lyrics_bar_shows_the_line_the_next_and_each_words_progress() {
        let tl = bar_timeline();
        let before = lyrics_bar(&tl, 500);
        assert_eq!((before.line, before.next), (None, Some(0)));
        assert!(before.words.is_empty());

        let first = lyrics_bar(&tl, 1_250);
        assert_eq!((first.line, first.next), (Some(0), Some(1)));
        assert_eq!(first.words, vec![0.5, 0.0]);
        let later = lyrics_bar(&tl, 1_750);
        assert_eq!(later.words, vec![1.0, 0.5]);

        // In the gap the last line stays, fully sung, and the next waits.
        let gap = lyrics_bar(&tl, 3_000);
        assert_eq!((gap.line, gap.next), (Some(0), Some(1)));
        assert_eq!(gap.words, vec![1.0, 1.0]);
        assert_eq!(gap.line_progress, 1.0);

        // A line without words: progress through the line to the next start.
        let plain = lyrics_bar(&tl, 9_000);
        assert_eq!((plain.line, plain.next), (Some(2), None));
        assert!(plain.words.is_empty());
        assert_eq!(plain.line_progress, 0.0, "no end and no next line");
        assert_eq!(lyrics_bar(&LyricTimeline::default(), 1_000), LyricsBar::default());
    }

    #[test]
    fn waits_until_a_target() {
        let mut c = DisplayClock::new();
        c.update(anchor(1, 1_250, 0, 1.0), 0);
        assert_eq!(c.wait_until(2_000_000, 0), Some(Duration::from_millis(750)));
        assert_eq!(c.wait_until(1_000_000, 0), Some(Duration::ZERO));
        c.update(anchor(2, 0, 0, 2.0), 0);
        assert_eq!(c.wait_until(1_000_000, 0), Some(Duration::from_millis(500)));
        assert_eq!(next_second_us(1_250_000), 2_000_000);
        assert_eq!(next_second_us(2_000_000), 3_000_000);
        assert_eq!(next_step_us(1_250_000, 400_000), Some(1_600_000));
        assert_eq!(next_step_us(1, 0), None);
    }
}
