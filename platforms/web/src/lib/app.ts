// Builds the app's state once: the core, the plugin host, the player and the views' stores.

import { getContext, setContext } from 'svelte';
import { loadCore, type Jf } from './core/wasm';
import { setLanguage } from './i18n/index.svelte';
import { PluginHost } from './plugins/host';
import { openStore, type Store } from './plugins/storage';
import { Artworks } from './state/artwork';
import { Files } from './state/files';
import { Nav } from './state/nav.svelte';
import { Player } from './state/player.svelte';
import { Search } from './state/search.svelte';
import { Settings } from './state/settings.svelte';
import { Sources } from './state/sources.svelte';
import { Theme } from './state/theme.svelte';

export interface App {
  jf: Jf;
  core: InstanceType<Jf['Core']>;
  store: Store;
  settings: Settings;
  theme: Theme;
  nav: Nav;
  sources: Sources;
  files: Files;
  artworks: Artworks;
  player: Player;
  search: Search;
}

const KEY = Symbol('jellyfesh');

export async function createApp(): Promise<App> {
  const jf = await loadCore();
  jf.start();
  const seed = crypto.getRandomValues(new Uint32Array(2));
  const core = new jf.Core(seed[0], seed[1]);
  const settings = new Settings(core);
  const [store] = await Promise.all([openStore(), setLanguage(settings.value.language)]);
  const theme = new Theme(jf, settings.value);
  const host = new PluginHost(jf, store);
  const sources = new Sources(jf, host);
  const files = new Files(jf, core);
  const artworks = new Artworks();
  const player = new Player(jf, core, sources, files, theme, artworks, settings.value);
  const search = new Search(jf, sources);
  const nav = new Nav();
  settings.onChange((value) => {
    theme.setSettings(value);
    player.applySettings(value);
    void setLanguage(value.language);
  });
  await sources.init();
  player.restore();
  const app: App = { jf, core, store, settings, theme, nav, sources, files, artworks, player, search };
  if (new URLSearchParams(location.search).has('e2e')) {
    // A handle for the end-to-end tests (gap measurement, state checks); only with ?e2e.
    (globalThis as unknown as { jellyfesh: App }).jellyfesh = app;
  }
  return app;
}

export function provideApp(app: App) {
  setContext(KEY, app);
}

export function useApp(): App {
  return getContext<App>(KEY);
}
