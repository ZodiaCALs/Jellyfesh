// The look on the page (ADR-0020): its layout rules (src/lib/look.ts) and the custom properties a
// resolved look sets (src/lib/state/look.svelte.ts), with jf-web's resolution.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import * as jf from '../../generated/wasm/jf_web.js';
import { arrangement, bottomPlayer, homeSections, placesInOrder } from '../../src/lib/look';
import { lookCss, type LookValue, scaledTokens } from '../../src/lib/state/look.svelte';

// The build reads the tokens with Vite's ?raw; the tests read the same file.
const TOKENS = scaledTokens(readFileSync(fileURLToPath(new URL('../../generated/tokens.css', import.meta.url)), 'utf8'));

beforeAll(() => {
  const wasm = fileURLToPath(new URL('../../generated/wasm/jf_web_bg.wasm', import.meta.url));
  jf.initSync({ module: readFileSync(wasm) });
});

describe('look layout', () => {
  it('picks the bottom player for the mini-player style', () => {
    expect(bottomPlayer('Bar', true)).toBe('bar');
    expect(bottomPlayer('Bar', false)).toBe('mini');
    expect(bottomPlayer('Compact', true)).toBe('mini');
    expect(bottomPlayer('Floating', true)).toBe('floating');
  });

  it('arranges Now Playing for the layout and the pane', () => {
    expect(arrangement('Stage', 'lyrics')).toBe('stage');
    expect(arrangement('Artwork', 'lyrics')).toBe('cover');
    expect(arrangement('Artwork', 'queue')).toBe('stage');
    expect(arrangement('Lyrics', 'info')).toBe('wide-pane');
  });

  it('orders the places the page has, once each', () => {
    const have = new Map<string, string>([
      ['Home', 'home'],
      ['Albums', 'library'],
      ['Songs', 'library'],
      ['Search', 'search'],
    ]);
    expect(placesInOrder(['Songs', 'Playlists', 'Home', 'Albums', 'Search'], have as never)).toEqual(['library', 'home', 'search']);
    expect(homeSections(['MostPlayed', 'RecentlyAdded', 'Summary'])).toEqual(['RecentlyAdded', 'Summary']);
  });
});

describe('look on the page', () => {
  const resolve = (look?: object): LookValue =>
    JSON.parse(jf.resolve_look(look ? JSON.stringify(look) : undefined, look ? 'org.jellyfesh.customize' : undefined));

  it('changes nothing the tokens set with the defaults', () => {
    const defaults = resolve();
    expect(defaults.customized).toBe(false);
    expect(defaults.density_scale).toBe(1);
    const css = lookCss(defaults, TOKENS);
    expect(css).not.toMatch(/--jf-radius-r-lg/);
    expect(css).not.toMatch(/--jf-space-s4/);
    expect(css).toContain('--jf-lyrics-glow:0;');
    expect(css).toContain('--jf-lyrics-highlight:var(--jf-ink-90);');
    expect(css).toContain('--jf-text-scale:1;');
  });

  it('scales radii, spacing, blur and type, and sets the face, weight and highlight', () => {
    const look = resolve({
      corner_scale: 2,
      glass: 0.5,
      text_scale: 1.2,
      density: 'Roomy',
      typeface: 'Serif',
      lyrics_weight: 'Heavy',
      lyrics_glow: 1,
      lyrics_highlight: { Custom: 0xff8800 },
    });
    expect(look.customized).toBe(true);
    expect(look.density_scale).toBeCloseTo(1.2, 5);
    expect(TOKENS.radii.length).toBeGreaterThan(5);
    const css = lookCss(look, TOKENS);
    expect(css).toContain('--jf-radius-r-lg:24.00px;');
    expect(css).toContain('--jf-space-s4:19.20px;');
    expect(css).toContain('--jf-glass-blur:13.00px;');
    expect(css).toContain('--jf-text-scale:1.2');
    expect(css).toMatch(/--jf-font-sans:ui-serif/);
    expect(css).toContain('--jf-lyrics-line-weight:800;');
    expect(css).toContain('--jf-lyrics-glow:1;');
    expect(css).toContain('--jf-lyrics-highlight:#ff8800;');
  });
});
