// The lyrics cascade's order (presentation timing for LyricsPane): when the active line changes,
// the lines on screen move to their new places one lyrics.stagger (40 ms) after the line above.
// The lines above the line that was active start together, so the cascade begins where the eye is.

/** Lines further down than this many steps start together, so the cascade never drags. */
export const MAX_CASCADE_STEPS = 8;

/**
 * How many staggers line `index` waits before it moves, when `active` became the active line:
 * 0 down to the line that was active before it, then one more per line, up to MAX_CASCADE_STEPS.
 */
export function cascadeStep(index: number, active: number | null): number {
  if (!Number.isFinite(index) || active === null) return 0;
  return Math.min(MAX_CASCADE_STEPS, Math.max(0, index - active + 1));
}
