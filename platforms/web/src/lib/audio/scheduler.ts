// When the next track starts: the timing half of the engine's transitions, without the browser
// (tests/unit/scheduler.test.ts drives it with a fake clock).
//
// The core decides *what* the transition is (jf-web: gapless, or an equal-power crossfade of a
// given length, from the same rules as jf-engine). A browser cannot start a media element at an
// exact audio-clock time, so this decides *when* to call play() on the prepared element:
//
// * crossfade: when the remaining time reaches the overlap; the fade then runs for what is left;
// * gapless: when the remaining time reaches the measured start latency of an element (how long
//   play() takes until its clock moves), so the next track's first sample lands as close as
//   possible to the current one's last. The latency is learned from every start; until the first
//   one, a typical value is assumed (a few milliseconds of overlap are inaudible, a gap is not).
//
// Times are wall-clock milliseconds: a crossfade's overlap is how long both tracks sound.
//
// Until then it asks to be woken: coarsely while the end is far, then once, precisely.

/** What the scheduler needs from the playing element. */
export interface DeckState {
  /** Media time in seconds. */
  currentTime: number;
  /** Media length in seconds; NaN or Infinity while unknown. */
  duration: number;
  paused: boolean;
  ended: boolean;
  /** Playback rate (1 normally). */
  rate: number;
}

export type Plan = { kind: 'gapless' } | { kind: 'crossfade'; overlapMs: number };

export type Decision =
  /** Nothing to plan: paused, no next track, or an unknown length (the end event decides). */
  | { kind: 'idle' }
  /** Ask again in `ms`. */
  | { kind: 'wait'; ms: number }
  /** Start the next track now, fading over `fadeMs` (0: a gapless join). */
  | { kind: 'start'; fadeMs: number };

/** The longest sleep between checks while the end is far away. */
export const MAX_WAIT_MS = 1000;
/** From this far before the start point, wake once more, precisely. */
const PRECISE_MS = 300;
/** Bounds for the learned start latency. */
const MAX_LEAD_MS = 120;
/** The start latency assumed before the first measurement. */
export const DEFAULT_LEAD_MS = 30;

export class TransitionScheduler {
  /** How early a gapless next element is started: its learned start latency, ms. */
  leadMs: number;

  private calibrated = false;

  constructor(leadMs = DEFAULT_LEAD_MS) {
    this.leadMs = leadMs;
  }

  /** Learns from a start that took `latencyMs` from play() to a moving clock. */
  calibrate(latencyMs: number) {
    if (!Number.isFinite(latencyMs) || latencyMs < 0) return;
    const measured = Math.min(MAX_LEAD_MS, latencyMs);
    this.leadMs = this.calibrated ? this.leadMs * 0.7 + measured * 0.3 : measured;
    this.calibrated = true;
  }

  /** What to do now for the playing element, the plan, and whether the next element is ready. */
  decide(current: DeckState, plan: Plan | null, nextReady: boolean): Decision {
    if (!plan || current.paused || current.ended || !(current.rate > 0)) return { kind: 'idle' };
    if (!Number.isFinite(current.duration) || current.duration <= 0) return { kind: 'idle' };
    const remaining = ((current.duration - current.currentTime) * 1000) / current.rate;
    const startAt = plan.kind === 'crossfade' ? Math.min(plan.overlapMs, (current.duration * 1000) / 2) : this.leadMs;
    const until = remaining - startAt;
    // Within half a millisecond counts as there (timers have millisecond steps).
    if (until <= 0.5) {
      if (!nextReady) return { kind: 'idle' };
      return { kind: 'start', fadeMs: plan.kind === 'crossfade' ? Math.max(0, Math.round(remaining)) : 0 };
    }
    if (until > PRECISE_MS) return { kind: 'wait', ms: Math.min(MAX_WAIT_MS, until - PRECISE_MS / 2) };
    return { kind: 'wait', ms: Math.max(1, Math.round(until)) };
  }
}
