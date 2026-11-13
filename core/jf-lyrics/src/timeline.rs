//! Flat timeline arrays and the cursor lookup that shells replicate.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::LyricDoc;

/// Marks "no active line" in [`LyricTimeline::segment_lines`].
pub const NO_LINE: u32 = u32::MAX;

/// Sorted lookup arrays for a document; unsynced documents give empty arrays.
///
/// This is what crosses the FFI: plain `u32` arrays, so each frame a shell
/// does two binary searches and nothing else (ADR-0001).
///
/// # Cursor rules
///
/// For a time `t`:
///
/// * A line is *active* when `start ≤ t < end`; a line without an end stays
///   active once started. Zero-length lines are never active.
/// * [`Cursor::line`] is the active line that started last (the later line wins
///   ties). Overlapping lines, as in duets, therefore hand back to the earlier
///   line when the later one ends. It is `None` before the first line, in gaps
///   between lines, and after the last line ends.
/// * [`Cursor::word`] is the last word of that line with `start ≤ t`, as an
///   index into [`Line::words`](crate::Line::words); `None` before the line's first word.
/// * [`Cursor::word_progress`] is `(t − start) / (end − start)` clamped to
///   `0..=1` (1 for zero-length words, 0 without a word).
///
/// The active-line rule is precomputed as a step function: from
/// `segment_starts[k]` until the next breakpoint the active line is
/// `segment_lines[k]` ([`NO_LINE`] for none). A shell finds `k` with one binary
/// search (the last breakpoint `≤ t`), then the word with a second one in the
/// line's slice of `word_starts`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricTimeline {
    /// Start of each line (`lines[i]` ↔ index `i`), ascending.
    pub line_starts: Vec<u32>,
    /// End of each line; `u32::MAX` when the line has no end (it never ends).
    pub line_ends: Vec<u32>,
    /// Breakpoints of the active-line step function, strictly ascending.
    pub segment_starts: Vec<u32>,
    /// Active line from the matching breakpoint until the next one, or [`NO_LINE`].
    pub segment_lines: Vec<u32>,
    /// Words of line `i` are at `word_offsets[i]..word_offsets[i + 1]` in the word arrays.
    pub word_offsets: Vec<u32>,
    /// Word starts, ascending within each line.
    pub word_starts: Vec<u32>,
    /// Word ends, never before the word's start.
    pub word_ends: Vec<u32>,
}

/// Position of the playhead in the lyrics; see [`LyricTimeline`] for the rules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    /// Index of the active line in [`LyricDoc::lines`].
    pub line: Option<usize>,
    /// Index of the current word in that line's words.
    pub word: Option<usize>,
    /// How much of the current word has been sung, `0.0..=1.0`.
    pub word_progress: f32,
}

impl LyricDoc {
    /// Builds the flat lookup arrays for this document.
    ///
    /// Lines without a start (unsynced documents) are never active. Word starts
    /// are made non-decreasing per line, as the parsers already guarantee.
    pub fn timeline(&self) -> LyricTimeline {
        let mut tl = LyricTimeline::default();
        if !self.is_synced() {
            return tl;
        }
        tl.word_offsets.push(0);
        for line in &self.lines {
            let start = line.start_ms.unwrap_or(u32::MAX);
            tl.line_starts.push(start);
            tl.line_ends.push(line.end_ms.unwrap_or(u32::MAX));
            let mut floor = 0;
            for word in &line.words {
                floor = floor.max(word.start_ms);
                tl.word_starts.push(floor);
                tl.word_ends.push(word.end_ms.max(floor));
            }
            tl.word_offsets.push(tl.word_starts.len() as u32);
        }
        (tl.segment_starts, tl.segment_lines) = segments(&tl.line_starts, &tl.line_ends);
        tl
    }
}

/// Sweeps line starts and ends to precompute the active line between breakpoints.
fn segments(starts: &[u32], ends: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let live: Vec<usize> = (0..starts.len()).filter(|&i| starts[i] < ends[i]).collect();
    let mut by_start = live.clone();
    by_start.sort_by_key(|&i| starts[i]);
    let mut by_end = live;
    by_end.sort_by_key(|&i| ends[i]);
    let mut points: Vec<u32> = by_start.iter().map(|&i| starts[i]).collect();
    points.extend(by_end.iter().map(|&i| ends[i]).filter(|&e| e != u32::MAX));
    points.sort_unstable();
    points.dedup();

    let (mut seg_starts, mut seg_lines) = (Vec::new(), Vec::new());
    let mut active: BTreeSet<(u32, usize)> = BTreeSet::new();
    let (mut next_start, mut next_end) = (0, 0);
    for &point in &points {
        while next_end < by_end.len() && ends[by_end[next_end]] <= point {
            let i = by_end[next_end];
            active.remove(&(starts[i], i));
            next_end += 1;
        }
        while next_start < by_start.len() && starts[by_start[next_start]] <= point {
            let i = by_start[next_start];
            if ends[i] > point {
                active.insert((starts[i], i));
            }
            next_start += 1;
        }
        let line = active.last().map_or(NO_LINE, |&(_, i)| i as u32);
        if seg_lines.last() != Some(&line) {
            seg_starts.push(point);
            seg_lines.push(line);
        }
    }
    (seg_starts, seg_lines)
}

impl LyricTimeline {
    /// True when the timeline has no timed lines.
    pub fn is_empty(&self) -> bool {
        self.segment_starts.is_empty()
    }

    /// The earliest time after `t_ms` at which the cursor's line or word
    /// changes, or the current word finishes (its progress reaches 1).
    ///
    /// Until then only [`Cursor::word_progress`] moves, so a view that draws no
    /// word sweep can sleep until this time instead of drawing every frame, and
    /// one that does sweep knows when the sweep ends. `None` when nothing
    /// changes after `t_ms`. Costs two binary searches, like [`Self::cursor`].
    pub fn next_change(&self, t_ms: u32) -> Option<u32> {
        let k = self.segment_starts.partition_point(|&s| s <= t_ms);
        let mut next = self.segment_starts.get(k).copied();
        let line = match k.checked_sub(1).and_then(|k| self.segment_lines.get(k)) {
            Some(&line) if line != NO_LINE => line as usize,
            _ => return next,
        };
        let (Some(&from), Some(&to)) = (self.word_offsets.get(line), self.word_offsets.get(line + 1)) else {
            return next;
        };
        let (from, to) = (from as usize, to as usize);
        let starts = self.word_starts.get(from..to).unwrap_or_default();
        let w = starts.partition_point(|&s| s <= t_ms);
        if let Some(&start) = starts.get(w) {
            next = Some(next.map_or(start, |n| n.min(start)));
        }
        if let Some(current) = w.checked_sub(1)
            && let Some(&end) = self.word_ends.get(from + current)
            && end > t_ms
            && end != u32::MAX
        {
            next = Some(next.map_or(end, |n| n.min(end)));
        }
        next
    }

    /// The cursor at `t_ms`, following the rules on [`LyricTimeline`].
    pub fn cursor(&self, t_ms: u32) -> Cursor {
        let k = self.segment_starts.partition_point(|&s| s <= t_ms);
        let line = match k.checked_sub(1).and_then(|k| self.segment_lines.get(k)) {
            Some(&line) if line != NO_LINE => line as usize,
            _ => return Cursor::default(),
        };
        let (Some(&from), Some(&to)) = (self.word_offsets.get(line), self.word_offsets.get(line + 1)) else {
            return Cursor {
                line: Some(line),
                ..Cursor::default()
            };
        };
        let (from, to) = (from as usize, to as usize);
        let starts = self.word_starts.get(from..to).unwrap_or_default();
        let Some(word) = starts.partition_point(|&s| s <= t_ms).checked_sub(1) else {
            return Cursor {
                line: Some(line),
                ..Cursor::default()
            };
        };
        let start = starts[word];
        let end = self.word_ends.get(from + word).copied().unwrap_or(start);
        let word_progress = if end <= start {
            1.0
        } else {
            ((t_ms - start) as f32 / (end - start) as f32).clamp(0.0, 1.0)
        };
        Cursor {
            line: Some(line),
            word: Some(word),
            word_progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Direction, Format, Line, Timing, Word};

    fn word(start: u32, end: u32) -> Word {
        Word {
            start_ms: start,
            end_ms: end,
            text: "w".into(),
            joins_next: false,
        }
    }

    fn doc(lines: &[(u32, Option<u32>, Vec<Word>)]) -> LyricDoc {
        let mut doc = LyricDoc::empty(Format::Lrc);
        doc.timing = Timing::Word;
        doc.lines = lines
            .iter()
            .enumerate()
            .map(|(i, (start, end, words))| Line {
                id: i as u32,
                start_ms: Some(*start),
                end_ms: *end,
                agent: None,
                text: String::new(),
                words: words.clone(),
                background: Vec::new(),
                direction: Direction::Auto,
            })
            .collect();
        doc
    }

    fn at(tl: &LyricTimeline, t: u32) -> (Option<usize>, Option<usize>) {
        let c = tl.cursor(t);
        (c.line, c.word)
    }

    #[test]
    fn edges() {
        let d = doc(&[
            (1_000, Some(2_000), vec![word(1_000, 1_400), word(1_500, 2_000)]),
            (3_000, Some(4_000), vec![]),
            (4_000, Some(5_000), vec![word(4_200, 4_200)]),
        ]);
        let tl = d.timeline();
        assert_eq!(at(&tl, 0), (None, None), "before the first line");
        assert_eq!(at(&tl, 999), (None, None));
        assert_eq!(at(&tl, 1_000), (Some(0), Some(0)), "exactly at the start");
        assert_eq!(
            at(&tl, 1_450),
            (Some(0), Some(0)),
            "between words: the last started word"
        );
        assert_eq!(tl.cursor(1_450).word_progress, 1.0);
        assert_eq!(at(&tl, 2_000), (None, None), "a gap between lines");
        assert_eq!(at(&tl, 3_500), (Some(1), None), "a line without words");
        assert_eq!(
            at(&tl, 4_000),
            (Some(2), None),
            "contiguous lines hand over at the boundary"
        );
        assert_eq!(tl.cursor(4_200).word_progress, 1.0, "zero-length word");
        assert_eq!(at(&tl, 5_000), (None, None), "after the last line ends");
        assert_eq!(at(&tl, u32::MAX), (None, None));
    }

    #[test]
    fn progress_inside_a_word() {
        let tl = doc(&[(0, None, vec![word(1_000, 2_000)])]).timeline();
        assert_eq!(tl.cursor(1_000).word_progress, 0.0);
        assert_eq!(tl.cursor(1_250).word_progress, 0.25);
        assert_eq!(tl.cursor(1_999).word_progress, 0.999);
        assert_eq!(tl.cursor(9_000).word_progress, 1.0);
        assert_eq!(
            tl.cursor(500),
            Cursor {
                line: Some(0),
                word: None,
                word_progress: 0.0
            }
        );
        assert_eq!(
            at(&tl, u32::MAX - 1),
            (Some(0), Some(0)),
            "an open last line stays active"
        );
    }

    #[test]
    fn overlapping_lines_fall_back_to_the_earlier_one() {
        let tl = doc(&[
            (0, Some(10_000), vec![]),
            (2_000, Some(4_000), vec![]),
            (2_000, Some(3_000), vec![]),
        ])
        .timeline();
        assert_eq!(at(&tl, 1_000).0, Some(0));
        assert_eq!(at(&tl, 2_000).0, Some(2), "equal starts: the later line");
        assert_eq!(at(&tl, 3_000).0, Some(1));
        assert_eq!(at(&tl, 4_000).0, Some(0));
        assert_eq!(at(&tl, 10_000).0, None);
        assert_eq!(tl.segment_starts, [0, 2_000, 3_000, 4_000, 10_000]);
        assert_eq!(tl.segment_lines, [0, 2, 1, 0, NO_LINE]);
    }

    #[test]
    fn unsynced_documents_have_no_cursor() {
        let mut d = doc(&[(0, None, vec![])]);
        d.timing = Timing::Unsynced;
        let tl = d.timeline();
        assert!(tl.is_empty());
        assert_eq!(tl.cursor(5), Cursor::default());
    }

    #[test]
    fn inconsistent_arrays_do_not_panic() {
        let tl = LyricTimeline {
            segment_starts: vec![0],
            segment_lines: vec![3],
            ..LyricTimeline::default()
        };
        assert_eq!(tl.cursor(10).line, Some(3));
    }
}
