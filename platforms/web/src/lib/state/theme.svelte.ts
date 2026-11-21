// Misty in the page: every colour comes from the core's theme for one hue (the artwork's, or the
// one the person fixed), set as --jf-<name> custom properties next to the generated static tokens;
// the Artwork backdrop is the core's baked image (jf-palette: downsampled, blurred, tone-mapped and
// dithered once per track, never blurred per frame).
//
// The glass tints follow the Transparency setting (the core's glass_alpha). New colours crossfade
// over dur-4 (registered colour properties the browser interpolates), except while a slider is
// dragged, when they follow it at once. Nothing here runs per frame: only when the settings, the
// artwork or the system's appearance or motion preference change.

import type { Jf } from '../core/wasm';
import type { ImageRef, Theme as CoreTheme, Themes, WebSettings } from '../core/types';
import { analyseArtwork } from '../media/client';
import type { RelayConfig } from '../plugins/relay';
import { fetchDirect, fetchRelayed } from './artwork';
import { cssColour as rgba, inkOpacity, withTransparency } from './glass';

export type Appearance = 'light' | 'dark';

/** Changes closer together than this are a slider being dragged: no crossfade. */
const BURST_MS = 150;

/** Artwork to show and take colours from. */
export type ArtworkSource =
  /** A picture of a source; `relay` when the source goes through one (its service refuses browsers). */
  | { kind: 'remote'; key: string; image: ImageRef; relay?: RelayConfig | null }
  | { kind: 'bytes'; key: string; bytes: Uint8Array; mime: string | null };

interface Analysed {
  /** What an <img> shows: an object URL of the bytes, or the remote URL when they cannot be read. */
  display: string | null;
  themes: Themes | null;
  backdrops: Map<string, string>;
  owned: string[];
}

const CACHE = 24;

/** `field.sky` → `--jf-sky`, `elevation.shadow-1.0` → `--jf-shadow-1-0` (as xtask's CSS target). */
export function cssName(token: string): string {
  const dot = token.indexOf('.');
  return `--jf-${token.slice(dot + 1).replaceAll('.', '-')}`;
}

export class Theme {
  /** The appearance in use. */
  appearance = $state<Appearance>('light');
  /** The colours in use (both appearances). */
  themes = $state.raw<Themes>() as Themes;
  /** The current artwork, ready for an <img>. */
  artwork = $state<string | null>(null);
  /** The baked backdrop of the current artwork for the appearance in use. */
  backdrop = $state<string | null>(null);

  /** Motion is reduced (the system's preference, or Reduce motion and transparency). */
  reducedMotion = $state(false);

  private settings: WebSettings;
  private system: MediaQueryList | null;
  private motion: MediaQueryList | null;
  private style: HTMLStyleElement;
  private cache = new Map<string, Analysed>();
  private currentKey: string | null = null;
  private loading: string | null = null;
  private lastApply = -Infinity;

  constructor(
    private readonly jf: Jf,
    settings: WebSettings,
  ) {
    this.settings = settings;
    this.system = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;
    this.system?.addEventListener('change', () => this.apply());
    this.motion = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
    this.motion?.addEventListener('change', () => this.apply());
    this.style = document.createElement('style');
    this.style.id = 'jf-theme';
    document.head.append(this.style);
    this.themes = this.fixedThemes();
    this.registerColours(Object.keys(this.themes.light.colors));
    this.apply(false);
  }

  /**
   * Registers the theme colours as typed properties, so the browser can interpolate them when a
   * theme changes (the crossfade), and lists them in the transition :root uses while fading.
   * Browsers without CSS.registerProperty switch colours at once.
   */
  private registerColours(tokens: string[]) {
    const names = tokens.map(cssName);
    if (typeof CSS === 'undefined' || typeof CSS.registerProperty !== 'function') return;
    for (const name of names) {
      try {
        CSS.registerProperty({ name, syntax: '<color>', inherits: true, initialValue: 'transparent' });
      } catch {
        // Already registered (a second Theme in the same page).
      }
    }
    const fade = document.createElement('style');
    fade.id = 'jf-theme-fade';
    fade.textContent =
      `:root[data-theme-fade]{transition-property:${names.join(',')};` +
      'transition-duration:var(--jf-motion-dur-4);transition-timing-function:var(--jf-motion-ease-fluid)}' +
      ":root[data-theme-fade][data-motion='reduced']{transition-duration:var(--jf-motion-dur-1)}";
    document.head.append(fade);
  }

  /** The default or fixed hue's themes. */
  private fixedThemes(): Themes {
    const hue = this.settings.hue_source.kind === 'fixed' ? this.settings.hue_source.hue : 250;
    return JSON.parse(this.jf.theme_for_hue(hue));
  }

  /** The look's glass (alpha × this, 0–1; ADR-0020). */
  private glassScale = 1;

  setGlass(scale: number) {
    if (scale === this.glassScale) return;
    this.glassScale = scale;
    this.apply(false);
  }

  setSettings(settings: WebSettings) {
    this.settings = settings;
    this.refresh();
  }

  /** The artwork of what plays now (`null`: none). */
  async setArtwork(source: ArtworkSource | null) {
    const key = source?.key ?? null;
    if (key === this.currentKey) return;
    this.currentKey = key;
    if (!source) {
      this.refresh();
      return;
    }
    const cached = this.cache.get(source.key);
    if (cached) {
      // Most recently used last.
      this.cache.delete(source.key);
      this.cache.set(source.key, cached);
      this.refresh();
      return;
    }
    // Until the new artwork is read, keep the old colours (no flash of the default hue).
    this.artwork =
      source.kind === 'remote' && source.image.headers.length === 0 && !source.relay ? source.image.url : this.artwork;
    if (this.loading === source.key) return;
    this.loading = source.key;
    const analysed = await this.analyse(source);
    if (this.loading === source.key) this.loading = null;
    this.remember(source.key, analysed);
    if (this.currentKey === source.key) this.refresh();
  }

  private async analyse(source: ArtworkSource): Promise<Analysed> {
    const result: Analysed = { display: null, themes: null, backdrops: new Map(), owned: [] };
    let blob: Blob | null = null;
    if (source.kind === 'bytes') {
      blob = new Blob([source.bytes as BlobPart], source.mime ? { type: source.mime } : {});
    } else {
      const headers = source.image.headers.map((h) => [h.name, h.value] as [string, string]);
      // The colours need the pixels: through the relay, or with CORS from the image's server.
      blob = source.relay ? await fetchRelayed(source.relay, source.image.url, headers) : await fetchDirect(source.image.url, headers);
      // No CORS for images on this server: show it, without its colours.
      if (!blob && source.image.headers.length === 0 && !source.relay) result.display = source.image.url;
    }
    if (!blob) return result;
    const display = URL.createObjectURL(blob);
    result.display = display;
    result.owned.push(display);
    try {
      const analysed = await analyseArtwork(blob, ['light', 'dark']);
      result.themes = analysed.themes;
      for (const [appearance, png] of analysed.backdrops) {
        const url = URL.createObjectURL(png);
        result.owned.push(url);
        result.backdrops.set(appearance, url);
      }
    } catch {
      // Not an image the browser can decode: no colours from it.
    }
    return result;
  }

  private remember(key: string, analysed: Analysed) {
    this.cache.set(key, analysed);
    while (this.cache.size > CACHE) {
      const [oldest, entry] = this.cache.entries().next().value as [string, Analysed];
      if (oldest === this.currentKey) break;
      this.cache.delete(oldest);
      for (const url of entry.owned) URL.revokeObjectURL(url);
    }
  }

  /** Recomputes the colours and backdrop from the settings and the current artwork. */
  refresh() {
    const current = this.currentKey ? this.cache.get(this.currentKey) : undefined;
    const fromArtwork = this.settings.hue_source.kind === 'artwork' && current?.themes;
    this.themes = fromArtwork ? current!.themes! : this.fixedThemes();
    if (this.currentKey === null) this.artwork = null;
    else if (current) this.artwork = current.display;
    this.apply();
  }

  /**
   * Sets the custom properties and attributes for the appearance in use. A change crossfades
   * (dur-4) when `animate`, unless it follows another within BURST_MS, as while a slider is dragged.
   */
  apply(animate = true) {
    const now = typeof performance === 'object' ? performance.now() : 0;
    const fade = animate && now - this.lastApply > BURST_MS;
    this.lastApply = now;
    const choice = this.settings.appearance;
    this.appearance = choice === 'system' ? (this.system?.matches ? 'dark' : 'light') : choice;
    this.reducedMotion = this.settings.reduce_effects || (this.motion?.matches ?? false);
    const theme: CoreTheme = this.themes[this.appearance];
    // Reduce motion and transparency makes glass solid wherever it is used.
    const transparency = this.settings.reduce_effects ? 0 : this.settings.transparency;
    // The look's glass scales the glass tints' alpha (the Transparency setting and Reduce
    // Transparency still win: solid stays solid).
    const glass = this.settings.reduce_effects ? 1 : this.glassScale;
    const colors = withTransparency(theme.colors, transparency, (a, t) => this.jf.glass_alpha(a, t) * glass);
    let css = ':root{';
    for (const [token, colour] of Object.entries(colors)) css += `${cssName(token)}:${rgba(colour)};`;
    css += `--jf-hue:${theme.hue};`;
    // The lyric inks as opacities of ink-90 (LyricsPane fades lines with opacity, not colour).
    css += `--jf-lyrics-dim:${inkOpacity(colors, 'ink.ink-50').toFixed(3)};`;
    css += `--jf-lyrics-dim-sung:${inkOpacity(colors, 'ink.ink-30').toFixed(3)};}`;
    const root = document.documentElement;
    // Set before the colours change, so the change starts (or skips) the transition.
    root.toggleAttribute('data-theme-fade', fade);
    this.style.textContent = css;
    root.dataset.appearance = this.appearance;
    root.dataset.effects = this.settings.reduce_effects ? 'reduced' : 'full';
    root.dataset.motion = this.reducedMotion ? 'reduced' : 'full';
    root.style.colorScheme = this.appearance;
    const current = this.currentKey ? this.cache.get(this.currentKey) : undefined;
    this.backdrop = current?.backdrops.get(this.appearance) ?? null;
    const sky = theme.colors['field.sky'];
    if (sky) {
      for (const meta of document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]')) {
        meta.content = rgba(sky);
        meta.removeAttribute('media');
      }
    }
  }
}
