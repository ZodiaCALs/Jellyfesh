// Presentation math for the theme the page applies: the transparency setting on Misty's glass
// grades, and the lyric inks as opacities. Run when the setting or the theme changes, never per
// frame. No DOM here, so the unit tests can check it with the core's real glass_alpha.

export type Rgba = [number, number, number, number];

/** A theme colour as CSS. */
export function cssColour([r, g, b, a]: Rgba): string {
  return a === 255 ? `rgb(${r} ${g} ${b})` : `rgb(${r} ${g} ${b} / ${(a / 255).toFixed(3)})`;
}

/** The theme colours that are glass grades (Clear, Mist, Veil), as jf_palette::GLASS_TINTS. */
export const GLASS_TINTS = ['glass.tint-clear', 'glass.tint-mist', 'glass.tint-veil'] as const;

/**
 * `colors` with the alpha of the glass tints set for `transparency` (0 solid … 1 clearest), as
 * jf_palette::Theme::with_transparency does: `glassAlpha` is the core's `glass_alpha` (the wasm
 * export), and the result is quantised to a byte like the core's. Other colours are unchanged.
 */
export function withTransparency(
  colors: Record<string, Rgba>,
  transparency: number,
  glassAlpha: (alpha: number, transparency: number) => number,
): Record<string, Rgba> {
  const out = { ...colors };
  for (const name of GLASS_TINTS) {
    const colour = colors[name];
    if (!colour) continue;
    const alpha = glassAlpha(colour[3] / 255, transparency);
    out[name] = [colour[0], colour[1], colour[2], Math.round(Math.min(1, Math.max(0, alpha)) * 255)];
  }
  return out;
}

/**
 * How opaque text in `ink` looks next to text in `full`, as an opacity: lyric lines are drawn in
 * ink-90 and faded to ink-50 or ink-30 with `opacity`, which the compositor animates (a colour
 * would be repainted every frame). ink-90 at this opacity is exactly `ink`.
 */
export function inkOpacity(colors: Record<string, Rgba>, ink: string, full = 'ink.ink-90'): number {
  const a = colors[ink]?.[3];
  const b = colors[full]?.[3];
  if (a === undefined || !b) return 1;
  return Math.min(1, a / b);
}
