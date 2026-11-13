// The transition scheduler with a fake clock: when the next track starts for a gapless join or a
// crossfade, how often it wakes, and how it learns the start latency.

import { describe, expect, it } from 'vitest';
import { DEFAULT_LEAD_MS, type DeckState, MAX_WAIT_MS, type Plan, TransitionScheduler } from '../../src/lib/audio/scheduler';

/** A playing element on a fake clock: plays from `at` seconds for `duration` seconds. */
class FakeDeck {
  now = 0;
  constructor(
    readonly duration: number,
    private readonly at = 0,
    public rate = 1,
    public paused = false,
  ) {}
  state(): DeckState {
    const t = Math.min(this.duration, this.at + (this.now / 1000) * this.rate);
    return { currentTime: t, duration: this.duration, paused: this.paused, ended: t >= this.duration, rate: this.rate };
  }
}

/** Follows the scheduler's waits until it starts the next track; returns when, and how often it woke. */
function run(scheduler: TransitionScheduler, deck: FakeDeck, plan: Plan, nextReady = true) {
  let wakeups = 0;
  for (let i = 0; i < 10_000; i++) {
    const decision = scheduler.decide(deck.state(), plan, nextReady);
    if (decision.kind === 'start') return { at: deck.now, fadeMs: decision.fadeMs, wakeups };
    if (decision.kind === 'idle') return { at: null, fadeMs: null, wakeups };
    expect(decision.ms).toBeGreaterThan(0);
    expect(decision.ms).toBeLessThanOrEqual(MAX_WAIT_MS);
    deck.now += decision.ms;
    wakeups++;
  }
  throw new Error('the scheduler never decided');
}

describe('TransitionScheduler', () => {
  it('starts a gapless join by the assumed latency before the end until one is measured', () => {
    const result = run(new TransitionScheduler(), new FakeDeck(30), { kind: 'gapless' });
    expect(result.at).toBe(30_000 - DEFAULT_LEAD_MS);
    expect(result.fadeMs).toBe(0);
    // About one wake-up a second, then a precise one.
    expect(result.wakeups).toBeLessThanOrEqual(33);
  });

  it('starts a gapless join early by the learned start latency', () => {
    const scheduler = new TransitionScheduler();
    scheduler.calibrate(40);
    const result = run(scheduler, new FakeDeck(30), { kind: 'gapless' });
    expect(result.at).toBe(30_000 - 40);
  });

  it('learns the latency as a moving average, within bounds', () => {
    const scheduler = new TransitionScheduler();
    expect(scheduler.leadMs).toBe(DEFAULT_LEAD_MS);
    scheduler.calibrate(50);
    expect(scheduler.leadMs).toBe(50);
    scheduler.calibrate(100);
    expect(scheduler.leadMs).toBeCloseTo(65, 6);
    scheduler.calibrate(5000);
    expect(scheduler.leadMs).toBeLessThanOrEqual(120);
    const before = scheduler.leadMs;
    scheduler.calibrate(Number.NaN);
    scheduler.calibrate(-3);
    expect(scheduler.leadMs).toBe(before);
  });

  it('starts a crossfade when the remaining time reaches the overlap, fading over what is left', () => {
    const result = run(new TransitionScheduler(), new FakeDeck(30), { kind: 'crossfade', overlapMs: 6000 });
    expect(result.at).toBe(24_000);
    expect(result.fadeMs).toBe(6000);
  });

  it('never overlaps more than half of a short track', () => {
    const result = run(new TransitionScheduler(), new FakeDeck(8), { kind: 'crossfade', overlapMs: 6000 });
    expect(result.at).toBe(4000);
    expect(result.fadeMs).toBe(4000);
  });

  it('counts in wall-clock time at other playback rates', () => {
    // At double speed the 30 s track lasts 15 s; the overlap is 2 s of real time.
    const result = run(new TransitionScheduler(), new FakeDeck(30, 0, 2), { kind: 'crossfade', overlapMs: 2000 });
    expect(result.at).toBe(13_000);
    expect(result.fadeMs).toBe(2000);
  });

  it('starts a track resumed past its crossfade point at once, with what is left', () => {
    const result = run(new TransitionScheduler(), new FakeDeck(30, 27), { kind: 'crossfade', overlapMs: 6000 });
    expect(result.at).toBe(0);
    expect(result.fadeMs).toBe(3000);
  });

  it('does nothing while paused, without a plan or a known length', () => {
    const scheduler = new TransitionScheduler();
    expect(scheduler.decide(new FakeDeck(30, 0, 1, true).state(), { kind: 'gapless' }, true)).toEqual({ kind: 'idle' });
    expect(scheduler.decide(new FakeDeck(30).state(), null, true)).toEqual({ kind: 'idle' });
    const unknown = { ...new FakeDeck(30).state(), duration: Number.POSITIVE_INFINITY };
    expect(scheduler.decide(unknown, { kind: 'gapless' }, true)).toEqual({ kind: 'idle' });
  });

  it('waits for the next track to be ready at the start point', () => {
    const deck = new FakeDeck(10, 10);
    expect(new TransitionScheduler().decide({ ...deck.state(), ended: false }, { kind: 'gapless' }, false)).toEqual({
      kind: 'idle',
    });
  });
});
