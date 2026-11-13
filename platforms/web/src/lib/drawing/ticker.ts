// Redraws only when the drawing changes: `draw` gets the position and answers when (in µs of track
// time) it next needs to run; the DisplayClock turns that into a timer. Nothing runs while paused,
// while the page is hidden or while the view says it is not shown (setActive), and nothing runs per
// frame (a sweeping lyric word is a CSS animation the browser runs).

import { DisplayClock, monotonicNowNs, type PlaybackAnchor } from './displayClock';

const live = new Set<Ticker>();
let listening = false;

function hidden(): boolean {
  return typeof document !== 'undefined' && document.visibilityState === 'hidden';
}

/** One listener for every ticker: they sleep while the page is hidden and catch up when it shows. */
function watchVisibility() {
  if (listening || typeof document === 'undefined') return;
  listening = true;
  document.addEventListener('visibilitychange', () => {
    for (const ticker of live) ticker.tick();
  });
}

export class Ticker {
  private clock = new DisplayClock();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private active = true;

  constructor(private readonly draw: (positionUs: bigint, running: boolean) => bigint | null) {
    live.add(this);
    watchVisibility();
  }

  /** A new anchor from the engine. */
  set(anchor: PlaybackAnchor) {
    this.clock.update(anchor, monotonicNowNs());
    this.tick();
  }

  /** Whether the view is shown; an inactive ticker draws once more when shown again. */
  setActive(active: boolean) {
    if (active === this.active) return;
    this.active = active;
    this.tick();
  }

  /** Draws again now (the drawing's inputs changed). */
  tick() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.stopped || !this.active || hidden()) return;
    const now = monotonicNowNs();
    const position = this.clock.positionUs(now);
    const target = this.draw(position, this.clock.isRunning);
    if (target === null) return;
    const wait = this.clock.waitUntil(target, now);
    if (wait === null) return;
    this.timer = setTimeout(() => this.tick(), Math.max(1, Math.ceil(Number(wait) / 1000)));
  }

  /** The position now, µs. */
  position(): bigint {
    return this.clock.positionUs(monotonicNowNs());
  }

  destroy() {
    this.stopped = true;
    live.delete(this);
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }
}
