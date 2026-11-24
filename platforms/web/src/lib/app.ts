// Builds the app's state once: the core, the plugin host, the player and the views' stores.

import { getContext, setContext } from 'svelte';
import { loadCore, type Jf } from './core/wasm';
import { setLanguage } from './i18n/index.svelte';
import { filesOfInput, type PickedFolder } from './library/folders';
import { openLibraryStore } from './library/store';
import { PluginHost } from './plugins/host';
import { openStore, type Store } from './plugins/storage';
import { Artworks } from './state/artwork';
import { Collections } from './state/collections.svelte';
import { DesktopLyricsWindow } from './state/desktopLyrics.svelte';
import { AiLyrics } from './state/aiLyrics.svelte';
import { Look } from './state/look.svelte';
import { Pro } from './state/pro.svelte';
import { Files } from './state/files';
import { Library } from './state/library.svelte';
import { MiniPlayerWindow } from './state/miniPlayer.svelte';
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
  library: Library;
  /** Hearts, Jellyfesh playlists and plays, kept in this browser. */
  collections: Collections;
  artworks: Artworks;
  player: Player;
  search: Search;
  desktopLyrics: DesktopLyricsWindow;
  /** The mini player window (Document Picture-in-Picture, where the browser has it). */
  miniPlayer: MiniPlayerWindow;
  pro: Pro;
  look: Look;
  aiLyrics: AiLyrics;
}

const KEY = Symbol('jellyfesh');

/** A folder picked with `<input webkitdirectory>`, where the browser keeps no folders. */
function pickWithInput(): Promise<PickedFolder | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.webkitdirectory = true;
    input.multiple = true;
    input.hidden = true;
    input.dataset.role = 'folder-picker';
    document.body.append(input);
    const finish = (picked: PickedFolder | null) => {
      input.remove();
      resolve(picked);
    };
    input.addEventListener('change', () => finish(filesOfInput(input.files ?? [])), { once: true });
    input.addEventListener('cancel', () => finish(null), { once: true });
    input.click();
  });
}

export async function createApp(): Promise<App> {
  const jf = await loadCore();
  jf.start();
  const seed = crypto.getRandomValues(new Uint32Array(2));
  const core = new jf.Core(seed[0], seed[1]);
  const settings = new Settings(core);
  const [store, libraryStore] = await Promise.all([openStore(), openLibraryStore(), setLanguage(settings.value.language)]);
  const theme = new Theme(jf, settings.value);
  const host = new PluginHost(jf, store);
  const sources = new Sources(jf, host);
  const files = new Files(jf, core);
  const library = new Library(jf, core, libraryStore, pickWithInput);
  files.attach(library);
  const collections = new Collections(jf, libraryStore, (keys) => library.tracksByKey(keys));
  const artworks = new Artworks();
  const player = new Player(jf, core, sources, files, theme, artworks, settings.value);
  // Every play that ends counts or is a skip (the core decides); Recently Played and Most Played
  // come from them. Dropped files are kept nowhere.
  player.onPlayed = (play) => {
    if (play.saved) collections.recordPlay(play.saved, play.started_ms, play.counted);
  };
  const search = new Search(jf, sources, library);
  const nav = new Nav();
  const desktopLyrics = new DesktopLyricsWindow();
  const miniPlayer = new MiniPlayerWindow();
  const pro = new Pro(jf);
  host.proActive = pro.active;
  pro.onChange((active) => {
    host.proActive = active;
  });
  const look = new Look(jf, host, pro);
  const aiLyrics = new AiLyrics(jf, host, pro, player);
  look.onChange((value) => theme.setGlass(value.glass));
  settings.onChange((value) => {
    theme.setSettings(value);
    player.applySettings(value);
    void setLanguage(value.language);
    desktopLyrics.follow(value.desktop_lyrics);
  });
  await Promise.all([sources.init(), library.init().then(() => collections.init())]);
  player.restore(() => library.restoreQueue());
  const app: App = {
    jf,
    core,
    store,
    settings,
    theme,
    nav,
    sources,
    files,
    library,
    collections,
    artworks,
    player,
    search,
    desktopLyrics,
    miniPlayer,
    pro,
    look,
    aiLyrics,
  };
  desktopLyrics.attach(app);
  miniPlayer.attach(app);
  void look.refresh();
  void aiLyrics.refresh();
  if (new URLSearchParams(location.search).has('e2e')) {
    // A handle for the end-to-end tests (gap measurement, state checks); only with ?e2e. The
    // aurora's drift stands still there, so screenshots and motion counts do not depend on time.
    (globalThis as unknown as { jellyfesh: App }).jellyfesh = app;
    document.documentElement.dataset.ambient = 'still';
  }
  return app;
}

export function provideApp(app: App) {
  setContext(KEY, app);
}

export function useApp(): App {
  return getContext<App>(KEY);
}
