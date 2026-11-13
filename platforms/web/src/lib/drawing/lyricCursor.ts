// Port of LyricTimeline::cursor and ::next_change (core/jf-lyrics/src/timeline.rs). Tested against
// tests/contract/lyrics-cursor.json; keep it in step with the Rust original (docs/ffi.md,
// "Drawing-time ports"). Times are milliseconds (u32).

/** The flat timeline arrays jf-lyrics builds (`LyricTimeline`, as serde writes it). */
export interface LyricTimeline {
  line_starts: number[];
  line_ends: number[];
  segment_starts: number[];
  segment_lines: number[];
  word_offsets: number[];
  word_starts: number[];
  word_ends: number[];
}

/** Marks "no active line" in segment_lines and "no end" in line_ends. */
export const NO_LINE = 0xffffffff;

/** Position of the playhead in the lyrics. */
export interface LyricCursor {
  /** Index of the active line in `LyricDoc.lines`, or null before, between and after lines. */
  line: number | null;
  /** Index of the current word in that line's `words`, or null before its first word. */
  word: number | null;
  /** How much of the current word has been sung, 0 to 1. */
  wordProgress: number;
}

const NONE: LyricCursor = { line: null, word: null, wordProgress: 0 };

/** How many of the ascending elements in [lo, hi) are at most t (Rust's partition_point). */
function countAtMost(list: number[], lo: number, hi: number, t: number): number {
  let low = lo;
  let high = hi;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (list[mid] <= t) low = mid + 1;
    else high = mid;
  }
  return low - lo;
}

function at(list: number[], index: number): number | null {
  return index >= 0 && index < list.length ? list[index] : null;
}

/** The bounds of from..to in the list, or an empty range when they do not fit. */
function slice(list: number[], from: number, to: number): [number, number] {
  return from <= to && to <= list.length ? [from, to] : [0, 0];
}

function activeLine(tl: LyricTimeline, t: number): number | null {
  const k = countAtMost(tl.segment_starts, 0, tl.segment_starts.length, t);
  if (k === 0) return null;
  const line = at(tl.segment_lines, k - 1);
  return line === null || line === NO_LINE ? null : line;
}

/**
 * The cursor at `t`: two binary searches, cheap enough for every frame. A line is active when
 * `start <= t < end`; the active line that started last wins. The word is the last word of that
 * line with `start <= t`; its progress is `(t - start) / (end - start)` clamped to 0..1 (1 for
 * zero-length words).
 */
export function cursor(tl: LyricTimeline, t: number): LyricCursor {
  const line = activeLine(tl, t);
  if (line === null) return NONE;
  const from = at(tl.word_offsets, line);
  const to = at(tl.word_offsets, line + 1);
  if (from === null || to === null) return { line, word: null, wordProgress: 0 };
  const [lo, hi] = slice(tl.word_starts, from, to);
  const count = countAtMost(tl.word_starts, lo, hi, t);
  if (count === 0) return { line, word: null, wordProgress: 0 };
  const word = count - 1;
  const start = tl.word_starts[lo + word];
  const end = at(tl.word_ends, from + word) ?? start;
  const progress = end <= start ? 1 : Math.min(1, Math.max(0, Math.fround((t - start) / (end - start))));
  return { line, word, wordProgress: progress };
}

/**
 * The earliest time after `t` at which the line or word changes, or the current word finishes;
 * null when nothing changes after `t`. A view that draws no word sweep sleeps until then.
 */
export function nextChange(tl: LyricTimeline, t: number): number | null {
  const k = countAtMost(tl.segment_starts, 0, tl.segment_starts.length, t);
  let next: number | null = at(tl.segment_starts, k);
  const line = activeLine(tl, t);
  if (line === null) return next;
  const from = at(tl.word_offsets, line);
  const to = at(tl.word_offsets, line + 1);
  if (from === null || to === null) return next;
  const [lo, hi] = slice(tl.word_starts, from, to);
  const count = countAtMost(tl.word_starts, lo, hi, t);
  if (lo + count < hi) {
    const start = tl.word_starts[lo + count];
    next = next === null ? start : Math.min(next, start);
  }
  if (count > 0) {
    const end = at(tl.word_ends, from + count - 1);
    if (end !== null && end > t && end !== NO_LINE) {
      next = next === null ? end : Math.min(next, end);
    }
  }
  return next;
}
