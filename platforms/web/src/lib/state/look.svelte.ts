// The look (ADR-0020; docs/ui.md "The Look slice"): the first enabled appearance plugin's look
// while Pro is active, asked for again when it, its version or its settings change; the
// Luminous defaults otherwise. jf-web resolves it (core/jf-web/src/look.rs, after the core's);
// this applies it live: corner radii, glass, the aurora, typeface, text size, density, the lyrics'
// weight, glow and highlight as custom properties, and the places, Home, Now Playing and the
// mini-player through `value`.

import tokens from '../../../generated/tokens.css?raw';
import type { Jf } from '../core/wasm';
import type { HomeSection, MiniPlayerStyle, NowPlayingLayout, Place } from '../look';
import type { Plugin, PluginHost } from '../plugins/host';
import type { Pro } from './pro.svelte';

export interface LookValue {
  corner_scale: number;
  glass: number;
  aurora: number;
  typeface: 'System' | 'Rounded' | 'Serif' | 'Mono';
  text_scale: number;
  density: 'Compact' | 'Standard' | 'Roomy';
  density_scale: number;
  lyrics_weight: 'Regular' | 'Medium' | 'Semibold' | 'Bold' | 'Heavy';
  lyrics_glow: number;
  lyrics_highlight: 'Artwork' | 'Accent' | { Custom: number };
  sidebar: Place[];
  home: HomeSection[];
  now_playing: NowPlayingLayout;
  mini_player: MiniPlayerStyle;
  customized: boolean;
  plugin: string | null;
}

/** The tokens a look scales, with their base values (px), from the generated tokens' text. */
export function scaledTokens(text: string): { radii: [string, number][]; spaces: [string, number][]; blurs: [string, number][] } {
  const px = (pattern: RegExp) => [...text.matchAll(pattern)].map((m) => [m[1], Number(m[2])] as [string, number]);
  return {
    radii: px(/(--jf-(?:radius-r-[a-z0-9]+|artwork-radius-[a-z]+|player-mini-radius)):\s*([0-9.]+)px/g),
    spaces: px(/(--jf-space-s\d+):\s*([0-9.]+)px/g),
    blurs: px(/(--jf-glass-(?:[a-z]+-)?blur(?:-[a-z]+)?):\s*([0-9.]+)px/g),
  };
}
const TOKENS = scaledTokens(tokens);

const FACES: Record<LookValue['typeface'], string | null> = {
  System: null,
  Rounded: 'ui-rounded, "SF Pro Rounded", "Hiragino Maru Gothic ProN", "Arial Rounded MT Bold", var(--jf-font-sans)',
  Serif: 'ui-serif, "New York", Georgia, "Noto Serif", "Songti SC", "Hiragino Mincho ProN", serif',
  Mono: 'var(--jf-font-mono)',
};
const WEIGHTS: Record<LookValue['lyrics_weight'], number> = { Regular: 400, Medium: 500, Semibold: 600, Bold: 700, Heavy: 800 };

/** The custom properties for a look (the defaults change nothing the tokens set). */
export function lookCss(look: LookValue, scaled = TOKENS): string {
  let css = ':root{';
  if (look.corner_scale !== 1) for (const [name, px] of scaled.radii) css += `${name}:${(px * look.corner_scale).toFixed(2)}px;`;
  if (look.density_scale !== 1) for (const [name, px] of scaled.spaces) css += `${name}:${(px * look.density_scale).toFixed(2)}px;`;
  if (look.glass !== 1) for (const [name, px] of scaled.blurs) css += `${name}:${(px * look.glass).toFixed(2)}px;`;
  css += `--jf-look-glass:${look.glass};`;
  css += `--jf-field-aurora-strength:${look.aurora};`;
  css += `--jf-text-scale:${look.text_scale};`;
  const face = FACES[look.typeface];
  if (face) css += `--jf-font-sans:${face};`;
  css += `--jf-lyrics-line-weight:${WEIGHTS[look.lyrics_weight]};`;
  // v0.0.6 had no glow: only a customized look draws one.
  css += `--jf-lyrics-glow:${look.customized ? look.lyrics_glow : 0};`;
  const highlight = look.lyrics_highlight;
  css +=
    highlight === 'Artwork'
      ? '--jf-lyrics-highlight:var(--jf-ink-90);'
      : highlight === 'Accent'
        ? '--jf-lyrics-highlight:var(--jf-accent-text);'
        : `--jf-lyrics-highlight:#${highlight.Custom.toString(16).padStart(6, '0')};`;
  return `${css}}`;
}

export class Look {
  value = $state.raw<LookValue>() as LookValue;
  private readonly defaults: LookValue;
  private key = '';
  private readonly style: HTMLStyleElement | null;

  constructor(
    private readonly jf: Jf,
    private readonly host: PluginHost,
    private readonly pro: Pro,
  ) {
    this.defaults = JSON.parse(jf.resolve_look(undefined, undefined));
    this.value = this.defaults;
    this.style = typeof document === 'undefined' ? null : document.head.appendChild(document.createElement('style'));
    if (this.style) this.style.id = 'jf-look';
    this.apply();
    host.onChange(() => void this.refresh());
    pro.onChange(() => void this.refresh());
  }

  /** The appearance plugin in charge: the first enabled one that runs, while Pro is active. */
  private active(): Plugin | undefined {
    if (!this.pro.active) return undefined;
    return this.host.plugins.find((p) => p.state.enabled && !p.blocked && p.declares('Appearance'));
  }

  /** Asks the plugin again when it, its version or its settings changed. */
  async refresh() {
    const plugin = this.active();
    const key = plugin ? `${plugin.id}\u0000${plugin.info?.version ?? plugin.entry.version}\u0000${JSON.stringify(plugin.state.settings ?? null)}` : '';
    if (key === this.key) return;
    this.key = key;
    if (!plugin) {
      this.set(this.defaults);
      return;
    }
    try {
      const look = await plugin.look();
      if (this.key !== key) return;
      this.set(JSON.parse(this.jf.resolve_look(JSON.stringify(look), plugin.id)));
    } catch {
      if (this.key === key) this.set(this.defaults);
    }
  }

  private listeners = new Set<(look: LookValue) => void>();

  onChange(listener: (look: LookValue) => void): () => void {
    this.listeners.add(listener);
    listener(this.value);
    return () => this.listeners.delete(listener);
  }

  private set(value: LookValue) {
    this.value = value;
    this.apply();
    for (const listener of this.listeners) listener(value);
  }

  private apply() {
    if (this.style) this.style.textContent = lookCss(this.value);
  }
}
