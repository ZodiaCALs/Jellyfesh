// Where the person is: a place (Home, Albums, Artists, Songs, Playlists, Favourites, Search,
// Sources, Settings; docs/ui.md)
// with a stack of pages opened from it, and Now Playing over everything. Each step deeper is a
// browser history entry, so the Back button and gesture (and Esc) go back as in the apps.

import { tick } from 'svelte';
import type { Item } from '../core/types';
import { picturesReady, viewTransition } from '../motion';

export type Tab = 'home' | 'albums' | 'artists' | 'songs' | 'playlists' | 'favourites' | 'search' | 'sources' | 'settings';
/** The places the phones' Library tab switches between, in order. */
export const LIBRARY_PLACES: Tab[] = ['albums', 'artists', 'songs', 'playlists', 'favourites'];
/** The sections of Settings, in their order. */
export type SettingsSection = 'appearance' | 'playback' | 'library' | 'sources' | 'extensions' | 'about';
export const SETTINGS_SECTIONS: SettingsSection[] = ['appearance', 'playback', 'library', 'sources', 'extensions', 'about'];
export type Route =
  | { view: Tab }
  /** A source's item; `songsListed` false for a playlist whose songs its service lists only in its
   *  own app (ADR-0024). */
  | { view: 'browse'; plugin: string; item: Item; songsListed?: boolean }
  /** A source's Discover page. */
  | { view: 'source'; plugin: string }
  /** A source's own folders (Browse on its Discover page). */
  | { view: 'root'; plugin: string }
  /** A Jellyfesh playlist (kept in this browser). */
  | { view: 'playlist'; id: number };
export type Pane = 'lyrics' | 'queue' | 'info' | 'comments';
/** How the last page change happened, for its motion: deeper and tabs rise, back fades. */
export type Motion = 'forward' | 'back';

const TABS: Tab[] = ['home', 'albums', 'artists', 'songs', 'playlists', 'favourites', 'search', 'sources', 'settings'];

interface HistoryState {
  jf: true;
  depth: number;
  nowPlaying: boolean;
}

function tabFromHash(): Tab {
  const hash = location.hash.replace(/^#\/?/, '').split('/')[0];
  return (TABS as string[]).includes(hash) ? (hash as Tab) : 'home';
}

function sectionFromHash(): SettingsSection {
  const [tab, section] = location.hash.replace(/^#\/?/, '').split('/');
  return tab === 'settings' && (SETTINGS_SECTIONS as string[]).includes(section) ? (section as SettingsSection) : 'appearance';
}

export class Nav {
  stack = $state.raw<Route[]>([{ view: 'home' }]);
  nowPlaying = $state(false);
  pane = $state<Pane>('lyrics');
  motion = $state<Motion>('forward');
  settingsSection = $state<SettingsSection>('appearance');
  /** Get Info's item, while its sheet is open. */
  info = $state.raw<{ plugin: string; item: Item } | null>(null);

  constructor() {
    this.stack = [{ view: tabFromHash() }];
    this.settingsSection = sectionFromHash();
    history.replaceState({ jf: true, depth: 0, nowPlaying: false } satisfies HistoryState, '');
    addEventListener('popstate', (event) => {
      const state = event.state as HistoryState | null;
      if (!state?.jf) {
        this.stack = [{ view: tabFromHash() }];
        this.nowPlaying = false;
        return;
      }
      if (state.depth < this.stack.length - 1) {
        this.motion = 'back';
        this.stack = this.stack.slice(0, state.depth + 1);
      }
      this.nowPlaying = state.nowPlaying;
    });
  }

  get route(): Route {
    return this.stack[this.stack.length - 1];
  }

  get tab(): Tab {
    return this.stack[0].view as Tab;
  }

  /** Switches tabs (back to its root when it is the current one). */
  switchTab(tab: Tab) {
    this.motion = 'forward';
    this.stack = [{ view: tab }];
    this.nowPlaying = false;
    const hash = tab === 'settings' ? `#settings/${this.settingsSection}` : `#${tab}`;
    history.replaceState({ jf: true, depth: 0, nowPlaying: false } satisfies HistoryState, '', hash);
  }

  /** Opens Settings at a section ("Sources Settings…" opens Sources). */
  openSettings(section: SettingsSection) {
    this.settingsSection = section;
    this.switchTab('settings');
  }

  /** Shows another section of Settings. */
  showSection(section: SettingsSection) {
    this.settingsSection = section;
    if (this.tab === 'settings' && this.stack.length === 1) {
      history.replaceState(history.state, '', `#settings/${section}`);
    }
  }

  /**
   * Opens a page on top of the current one. From a cover (`origin`), where browsers have view
   * transitions, the cover grows into the new page's artwork while the rest crossfades.
   */
  open(route: Route, origin: HTMLElement | null = null) {
    const go = () => {
      this.motion = 'forward';
      this.stack = [...this.stack, route];
      this.nowPlaying = false;
      history.pushState({ jf: true, depth: this.stack.length - 1, nowPlaying: false } satisfies HistoryState, '');
    };
    if (!origin) {
      go();
      return;
    }
    let target: HTMLElement | null = null;
    origin.style.viewTransitionName = 'item-art';
    void viewTransition('open-item', async () => {
      origin.style.viewTransitionName = '';
      go();
      await tick();
      target = document.querySelector<HTMLElement>('.route .hero-art');
      if (target) target.style.viewTransitionName = 'item-art';
      await picturesReady(target);
    }).finally(() => {
      origin.style.viewTransitionName = '';
      if (target) target.style.viewTransitionName = '';
    });
  }

  back() {
    if (this.nowPlaying || this.stack.length > 1) history.back();
  }

  /** Opens Now Playing; where browsers allow, its artwork grows out of the player's. */
  openNowPlaying(pane?: Pane) {
    if (pane) this.pane = pane;
    if (this.nowPlaying) return;
    void viewTransition('now-playing', async () => {
      // Twice in one frame opens once.
      if (this.nowPlaying) return;
      this.nowPlaying = true;
      history.pushState({ jf: true, depth: this.stack.length - 1, nowPlaying: true } satisfies HistoryState, '');
      await tick();
    });
  }

  closeNowPlaying() {
    if (!this.nowPlaying) return;
    if ((history.state as HistoryState | null)?.nowPlaying) history.back();
    else this.nowPlaying = false;
  }
}
