// Motion: the lyrics cascade's order, and the curves and durations read from the tokens.

import { describe, expect, it } from 'vitest';
import { cascadeStep, MAX_CASCADE_STEPS } from '../../src/lib/drawing/cascade';
import { cubicBezier, durationOf, easingOf } from '../../src/lib/motion';

describe('the lyrics cascade', () => {
  it('starts at the line that was active and staggers each line below it', () => {
    // Line 5 became active: lines up to 4 start at once, then one stagger a line.
    expect([2, 3, 4, 5, 6, 7].map((line) => cascadeStep(line, 5))).toEqual([0, 0, 0, 1, 2, 3]);
  });

  it('caps the wait, so far lines start together', () => {
    expect(cascadeStep(40, 5)).toBe(MAX_CASCADE_STEPS);
    expect(cascadeStep(Number.NaN, 5)).toBe(0);
    expect(cascadeStep(3, null)).toBe(0);
  });
});

describe('motion from the tokens', () => {
  it('reads durations, also from the Now Playing motion tokens', () => {
    expect(durationOf('240ms', 0)).toBe(240);
    expect(durationOf('"Deeper 320 ms"', 0)).toBe(320);
    expect(durationOf('"Back 300 ms"', 0)).toBe(300);
    expect(durationOf('0.6s', 0)).toBe(600);
    // As the CSS minifier writes 160ms.
    expect(durationOf('.16s', 0)).toBe(160);
    expect(durationOf('', 160)).toBe(160);
  });

  it('follows cubic-bezier curves', () => {
    const linear = cubicBezier(0, 0, 1, 1);
    for (const t of [0, 0.25, 0.5, 0.75, 1]) expect(linear(t)).toBeCloseTo(t, 5);
    const fluid = easingOf('cubic-bezier(0.22, 1, 0.36, 1)', [0, 0, 1, 1]);
    // Fluid decelerates: well past half-way at the half-way time, monotonic, ends at 1.
    expect(fluid(0.5)).toBeGreaterThan(0.85);
    let last = 0;
    for (let i = 1; i <= 50; i++) {
      const value = fluid(i / 50);
      expect(value).toBeGreaterThanOrEqual(last - 1e-9);
      last = value;
    }
    expect(fluid(1)).toBe(1);
    // Exit accelerates.
    expect(easingOf('cubic-bezier(0.4, 0, 1, 1)', [0, 0, 1, 1])(0.5)).toBeLessThan(0.4);
    // A token that is not a curve falls back.
    expect(easingOf('ease', [0, 0, 1, 1])(0.3)).toBeCloseTo(0.3, 5);
  });
});
