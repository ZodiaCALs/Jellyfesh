// When a progress line next changes on screen (presentation math for the seek bar, the player
// bar and the mini-player's line). A line `widthPx` CSS pixels long shows a track of `durationUs`;
// its fill moves by one device pixel every `durationUs / (widthPx × devicePixelRatio)` µs, and the
// views redraw only then (or when a time label's second changes), never per frame.

import { nextSecond, nextStep } from './displayClock';

/** Track time per device pixel of a line, µs (at least 1); null without a length or a width. */
export function pixelStepUs(durationUs: bigint | null, widthPx: number, pixelRatio = 1): bigint | null {
  if (durationUs === null || durationUs <= 0n) return null;
  const pixels = Math.floor(widthPx * (pixelRatio > 0 ? pixelRatio : 1));
  if (!Number.isFinite(pixels) || pixels <= 0) return null;
  const step = durationUs / BigInt(pixels);
  return step > 0n ? step : 1n;
}

/**
 * The next position (µs) at which a line of `widthPx` and, with `seconds`, its m:ss labels change:
 * the earlier of the next device pixel and the next whole second.
 */
export function nextDrawUs(
  positionUs: bigint,
  durationUs: bigint | null,
  widthPx: number,
  pixelRatio = 1,
  seconds = true,
): bigint | null {
  const step = pixelStepUs(durationUs, widthPx, pixelRatio);
  const pixel = step === null ? null : nextStep(positionUs, step);
  const second = seconds ? nextSecond(positionUs) : null;
  if (pixel === null) return second;
  if (second === null) return pixel;
  return pixel < second ? pixel : second;
}
