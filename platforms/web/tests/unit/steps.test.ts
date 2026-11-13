// When progress lines redraw (the seek bar, the player bar, the mini-player's line): a device
// pixel at a time, or when a time label's second changes, never per frame.

import { describe, expect, it } from 'vitest';
import { nextSecond } from '../../src/lib/drawing/displayClock';
import { nextDrawUs, pixelStepUs } from '../../src/lib/drawing/steps';

describe('progress lines redraw a device pixel at a time', () => {
  it('steps by the track time of one device pixel', () => {
    // 4 minutes over 400 CSS px: 600 ms a pixel at 1x, 300 ms at 2x.
    expect(pixelStepUs(240_000_000n, 400, 1)).toBe(600_000n);
    expect(pixelStepUs(240_000_000n, 400, 2)).toBe(300_000n);
    // Partial pixels do not count; a very short track still steps by at least 1 µs.
    expect(pixelStepUs(240_000_000n, 400.7, 1)).toBe(600_000n);
    expect(pixelStepUs(100n, 1000, 1)).toBe(1n);
  });

  it('has no step without a length or a width', () => {
    expect(pixelStepUs(null, 400)).toBeNull();
    expect(pixelStepUs(0n, 400)).toBeNull();
    expect(pixelStepUs(240_000_000n, 0)).toBeNull();
    expect(pixelStepUs(240_000_000n, Number.NaN)).toBeNull();
  });

  it('wakes at the next pixel or the next second, whichever comes first', () => {
    // 600 ms pixels: at 0.5 s the next pixel (0.6 s) comes before the next second.
    expect(nextDrawUs(500_000n, 240_000_000n, 400, 1)).toBe(600_000n);
    // At 0.9 s the next second (1 s) comes before the next pixel (1.2 s).
    expect(nextDrawUs(900_000n, 240_000_000n, 400, 1)).toBe(1_000_000n);
    // Without times (the mini-player's line), only pixels count.
    expect(nextDrawUs(900_000n, 240_000_000n, 400, 1, false)).toBe(1_200_000n);
    // Without a width (hidden), only the time labels.
    expect(nextDrawUs(900_000n, 240_000_000n, 0, 1)).toBe(nextSecond(900_000n));
    expect(nextDrawUs(900_000n, null, 0, 1, false)).toBeNull();
  });
});
