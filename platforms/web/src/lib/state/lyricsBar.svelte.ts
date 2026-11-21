// The lyrics bar's state at the playback position (ADR-0022 §6, docs/ui.md "Lyrics, word by
// word"): which line shows, the next one, and how far each word is sung, from the port of the
// core's `lyrics_bar` (src/lib/drawing/lyricsBar.ts). Redrawn only when a line or a word changes
// (the timeline's next_change); the word being sung sweeps as a CSS animation, so nothing runs per
// frame. The player bar and Desktop Lyrics each keep one.

import { nextChange, type LyricTimeline } from '../drawing/lyricCursor';
import type { PlaybackAnchor } from '../drawing/displayClock';
import { type LyricsBar, lyricsBar } from '../drawing/lyricsBar';
import { Ticker } from '../drawing/ticker';
import type { Lyrics } from '../core/types';

const EMPTY: LyricsBar = { line: null, next: null, words: [], lineProgress: 0 };

/** Whether the lyrics bar shows: the setting is on and the lyrics are timed (as the apps decide). */
export function showsLyricsBar(setting: boolean, lyrics: Lyrics | null): boolean {
  return setting && lyrics !== null && lyrics.synced && lyrics.doc.timing !== 'unsynced';
}

/**
 * The word being sung and how much of it (0–1), or the last word whole once the line is sung;
 * null without word timing. As the apps' `sweep`.
 */
export function sweepOf(words: number[]): { word: number; progress: number } | null {
  if (words.length === 0) return null;
  const word = words.findIndex((w) => w < 1);
  return word < 0 ? { word: words.length - 1, progress: 1 } : { word, progress: Math.min(1, Math.max(0, words[word])) };
}

export class LyricsBarClock {
  bar = $state.raw<LyricsBar>(EMPTY);
  /** Whether the position moves (playing, not buffering). */
  running = $state(false);
  /** How long the word being sung still sweeps, ms of wall time. */
  remainingMs = $state(0);
  /** Changes on every redraw, so a sweep restarts from where the word is. */
  parity = $state(0);
  private timeline: LyricTimeline | null = null;
  private rate = 1;
  private readonly ticker: Ticker;

  /** `hidden`: whether the window that shows the bar is hidden (the page's by default). */
  constructor(hidden?: () => boolean) {
    this.ticker = new Ticker((us, running) => this.draw(us, running), hidden);
  }

  /** New lyrics or a new anchor from the engine. */
  set(timeline: LyricTimeline | null, anchor: PlaybackAnchor) {
    this.timeline = timeline;
    this.rate = anchor.rate > 0 ? anchor.rate : 1;
    if (!timeline) {
      this.bar = EMPTY;
      this.remainingMs = 0;
    }
    this.ticker.set(anchor);
  }

  /** Whether the bar is on screen; it sleeps while not. */
  setActive(active: boolean) {
    this.ticker.setActive(active);
  }

  /** Draws again now (its window showed again). */
  wake() {
    this.ticker.tick();
  }

  destroy() {
    this.ticker.destroy();
  }

  private draw(us: bigint, running: boolean): bigint | null {
    const tl = this.timeline;
    if (!tl) return null;
    const ms = Number(us / 1000n);
    const bar = lyricsBar(tl, ms);
    this.bar = bar;
    this.running = running;
    const sweeping = bar.words.findIndex((w) => w > 0 && w < 1);
    if (bar.line !== null && sweeping >= 0) {
      const end = tl.word_ends[tl.word_offsets[bar.line] + sweeping] ?? ms;
      this.remainingMs = Math.max(0, end - ms) / this.rate;
    } else {
      this.remainingMs = 0;
    }
    this.parity += 1;
    const next = nextChange(tl, ms);
    return next === null ? null : BigInt(next) * 1000n;
  }
}
