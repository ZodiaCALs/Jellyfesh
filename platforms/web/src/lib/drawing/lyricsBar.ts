// Port of jf_core::lyrics_bar (core/jf-core/src/timing.rs, ADR-0022 §6): what the one-line lyrics
// bar in the player bar and Desktop Lyrics show at a time. Tested against
// tests/contract/lyrics-bar.json, as the Swift, Kotlin and C# ports are; keep it in step with the
// Rust original (docs/ffi.md, "Drawing-time ports"). Times are milliseconds (u32).

import { cursor, type LyricTimeline, NO_LINE } from './lyricCursor';

export interface LyricsBar {
  /** The active line, or in a gap between lines the last one that started; null before the first. */
  line: number | null;
  /** The next line by start; before the first line, the first one. */
  next: number | null;
  /** Each word of `line`, from 0 (still to come) to 1 (sung). */
  words: number[];
  /** 0–1 through `line`, for lines without word timing. */
  lineProgress: number;
}

/** How many of the ascending elements of `list` are at most `t` (Rust's partition_point). */
function startedBy(list: number[], t: number): number {
  let low = 0;
  let high = list.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (list[mid] <= t) low = mid + 1;
    else high = mid;
  }
  return low;
}

/** The lyrics bar at `t` (ms): a binary search and one pass over a line's words. */
export function lyricsBar(tl: LyricTimeline, t: number): LyricsBar {
  const timed = (i: number) => i >= 0 && i < tl.line_starts.length && tl.line_starts[i] !== NO_LINE;
  const count = tl.line_starts.length;
  const active = cursor(tl, t).line;
  let line = active;
  if (line === null) {
    // The last line that started, if any: it stays through the gap.
    for (let i = startedBy(tl.line_starts, t) - 1; i >= 0; i--) {
      if (timed(i)) {
        line = i;
        break;
      }
    }
  }
  let next: number | null = null;
  for (let i = line === null ? 0 : line + 1; i < count; i++) {
    if (timed(i)) {
      next = i;
      break;
    }
  }
  if (line === null) return { line: null, next, words: [], lineProgress: 0 };
  const from = tl.word_offsets[line];
  const to = tl.word_offsets[line + 1];
  const words: number[] = [];
  if (from !== undefined && to !== undefined) {
    for (let w = from; w < to; w++) {
      const start = tl.word_starts[w];
      const end = Math.max(tl.word_ends[w], start);
      words.push(t < start ? 0 : t >= end ? 1 : Math.fround((t - start) / (end - start)));
    }
  }
  const start = tl.line_starts[line];
  const ownEnd = tl.line_ends[line];
  const end = ownEnd !== undefined && ownEnd !== NO_LINE ? ownEnd : next === null ? null : tl.line_starts[next];
  let lineProgress = 0;
  if (active === null && t >= start) {
    // In the gap after the line: it is over.
    lineProgress = words.length === 0 ? 0 : 1;
  } else if (end !== null && end > start) {
    lineProgress = Math.min(1, Math.max(0, Math.fround(Math.max(0, t - start) / (end - start))));
  }
  return { line, next, words, lineProgress };
}
