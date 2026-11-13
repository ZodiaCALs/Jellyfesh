// The transparency setting on the glass tints, with the core's real glass_alpha (jf-web, built by
// `npm run wasm`), and the lyric inks as opacities.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import * as jf from '../../generated/wasm/jf_web.js';
import type { Themes } from '../../src/lib/core/types';
import { cssColour, GLASS_TINTS, inkOpacity, type Rgba, withTransparency } from '../../src/lib/state/glass';

beforeAll(() => {
  const wasm = fileURLToPath(new URL('../../generated/wasm/jf_web_bg.wasm', import.meta.url));
  jf.initSync({ module: readFileSync(wasm) });
});

describe('transparency on the glass tints', () => {
  const themes = (): Themes => JSON.parse(jf.theme_for_hue(250));
  const glassAlpha = (alpha: number, t: number) => jf.glass_alpha(alpha, t);

  it('keeps the tokens’ alphas at 50 %', () => {
    for (const appearance of ['light', 'dark'] as const) {
      const colors = themes()[appearance].colors as Record<string, Rgba>;
      expect(withTransparency(colors, 0.5, glassAlpha)).toEqual(colors);
    }
  });

  it('makes glass opaque at 0 % and halves its alpha at 100 %', () => {
    const colors = themes().light.colors as Record<string, Rgba>;
    const solid = withTransparency(colors, 0, glassAlpha);
    const clear = withTransparency(colors, 1, glassAlpha);
    for (const name of GLASS_TINTS) {
      expect(solid[name].slice(0, 3)).toEqual(colors[name].slice(0, 3));
      expect(solid[name][3]).toBe(255);
      expect(Math.abs(clear[name][3] - colors[name][3] / 2)).toBeLessThanOrEqual(1);
    }
    // Only the glass tints change.
    expect(solid['accent.accent']).toEqual(colors['accent.accent']);
    expect(clear['ink.ink-90']).toEqual(colors['ink.ink-90']);
  });

  it('never makes glass more opaque as transparency rises', () => {
    const colors = themes().dark.colors as Record<string, Rgba>;
    let last = 256;
    for (let step = 0; step <= 20; step++) {
      const alpha = withTransparency(colors, step / 20, glassAlpha)['glass.tint-veil'][3];
      expect(alpha).toBeLessThanOrEqual(last);
      last = alpha;
    }
  });

  it('writes colours as CSS', () => {
    expect(cssColour([16, 24, 32, 255])).toBe('rgb(16 24 32)');
    expect(cssColour([251, 254, 255, 158])).toBe('rgb(251 254 255 / 0.620)');
  });
});

describe('lyric inks as opacities', () => {
  it('fades ink-90 to exactly ink-50 and ink-30', () => {
    for (const appearance of ['light', 'dark'] as const) {
      const colors = (JSON.parse(jf.theme_for_hue(250)) as Themes)[appearance].colors as Record<string, Rgba>;
      const full = colors['ink.ink-90'][3];
      expect(full * inkOpacity(colors, 'ink.ink-50')).toBeCloseTo(colors['ink.ink-50'][3], 6);
      expect(full * inkOpacity(colors, 'ink.ink-30')).toBeCloseTo(colors['ink.ink-30'][3], 6);
    }
  });

  it('is opaque when a colour is missing', () => {
    expect(inkOpacity({}, 'ink.ink-50')).toBe(1);
  });
});
