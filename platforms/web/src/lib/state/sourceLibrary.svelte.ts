// The sources' likes and playlists (ADR-0024 §2; docs/ui.md "Favourites" and "Playlists"), with
// the core's rules (jf-web's SourceLibraries): every enabled, signed-in source with the library
// capability (API 1.3), its liked songs, albums and artists and its playlists, the first 100 of
// each and more a page at a time. The lists are kept in this browser, so they show at once (offline
// too) with their age. Favourites refreshes the likes and Playlists the playlists: all of them the
// first time the page opens in a visit, then those older than ten minutes when it opens or shows
// again; nothing polls. Read-only: nothing changes on the services.

import { untrack } from 'svelte';
import type { App } from '../app';
import type { Jf } from '../core/wasm';
import type { ItemKind, SourceLibrary, SourceLibraryList, SourceLibraryPart } from '../core/types';
import type { LibraryStore } from '../library/store';
import { CallError, type Plugin } from '../plugins/host';
import { explain, type Sources } from './sources.svelte';
import { toast } from './toast.svelte';

/** The pages that show the lists. */
export type SourceLibraryPage = 'favourites' | 'playlists';

const LIKED: Record<Exclude<SourceLibraryPart, 'playlists'>, ItemKind> = {
  liked_songs: 'Track',
  liked_albums: 'Album',
  liked_artists: 'Artist',
};

function pageOf(part: SourceLibraryPart): SourceLibraryPage {
  return part === 'playlists' ? 'playlists' : 'favourites';
}

/** A page shows a list that has entries, is loading or failed. */
function shows(list: SourceLibraryList): boolean {
  return list.entries.length > 0 || list.loading || list.error !== null;
}

export class SourceLibraries {
  /** Every covered source with its lists, in the order of the sources. */
  all = $state.raw<SourceLibrary[]>([]);
  /** More of a list on its way, by `plugin|part`. */
  loadingMore = $state.raw<ReadonlySet<string>>(new Set());
  /** The sources with likes to show (Favourites): their liked songs, albums and artists. */
  readonly likes = $derived(this.forPage('favourites'));
  /** The sources with playlists to show (Playlists). */
  readonly playlists = $derived(this.forPage('playlists'));
  private inner: InstanceType<Jf['SourceLibraries']>;
  private readonly first: number;
  /** The pages opened in this visit. */
  private readonly opened = new Set<SourceLibraryPage>();
  private saving: Promise<void> = Promise.resolve();

  constructor(
    private readonly jf: Jf,
    private readonly sources: Sources,
    private readonly store: LibraryStore,
  ) {
    this.inner = new jf.SourceLibraries();
    this.first = jf.source_library_first_page();
  }

  /** Reads the lists this browser kept. */
  async init() {
    let cache: string | null = null;
    try {
      cache = await this.store.sourceLibraries();
    } catch {
      // Unreadable: the lists come with the next refresh.
    }
    this.inner.free();
    this.inner = new this.jf.SourceLibraries(cache ?? undefined);
    this.cover();
  }

  /** The sources covered now: on, signed in and with the library capability (call when they change). */
  cover() {
    const covered = this.sources.usable('Library').map((plugin) => ({
      plugin: plugin.id,
      name: plugin.name,
      attribution: plugin.info?.content_policy.attribution ?? null,
    }));
    this.inner.cover(JSON.stringify(covered));
    this.publish();
  }

  /** Favourites or Playlists opened or showed again: their lists refresh as the module says. */
  shown(page: SourceLibraryPage) {
    const first = !this.opened.has(page);
    this.opened.add(page);
    const now = Date.now();
    for (const plugin of this.sources.usable('Library')) {
      const parts = JSON.parse(this.inner.stale(plugin.id, now, first)) as SourceLibraryPart[];
      for (const part of parts) if (pageOf(part) === page) void this.load(plugin, part);
    }
  }

  private async load(plugin: Plugin, part: SourceLibraryPart) {
    this.inner.begin(plugin.id, part);
    this.publish();
    try {
      if (part === 'playlists') {
        const page = await plugin.playlists(null, this.first);
        this.inner.playlists(plugin.id, JSON.stringify(page), Date.now());
      } else {
        const page = await plugin.likes(LIKED[part], null, this.first);
        this.inner.likes(plugin.id, part, JSON.stringify(page), Date.now());
      }
    } catch (error) {
      if (error instanceof CallError && error.plugin === 'Unsupported') this.inner.unsupported(plugin.id, part);
      else this.inner.fail(plugin.id, part, await explain(error, plugin));
    }
    this.publish();
    this.save();
  }

  /** More of a list (Show More). */
  async more(plugin: string, part: SourceLibraryPart) {
    const key = `${plugin}|${part}`;
    const source = this.sources.get(plugin);
    const cursor = this.inner.next(plugin, part);
    if (!source || !cursor || this.loadingMore.has(key)) return;
    this.loadingMore = new Set([...this.loadingMore, key]);
    try {
      if (part === 'playlists') {
        const page = await source.playlists(cursor, this.first);
        this.inner.more_playlists(plugin, JSON.stringify(page));
      } else {
        const page = await source.likes(LIKED[part], cursor, this.first);
        this.inner.more_likes(plugin, part, JSON.stringify(page));
      }
      this.publish();
      this.save();
    } catch (error) {
      toast(await explain(error, source));
    } finally {
      const loading = new Set(this.loadingMore);
      loading.delete(key);
      this.loadingMore = loading;
    }
  }

  private forPage(page: SourceLibraryPage): SourceLibrary[] {
    return this.all.flatMap((source) => {
      const lists = source.lists.filter((list) => pageOf(list.part) === page && shows(list));
      return lists.length > 0 ? [{ ...source, lists }] : [];
    });
  }

  private publish() {
    this.all = JSON.parse(this.inner.sources()) as SourceLibrary[];
  }

  private save() {
    const cache = this.inner.cache();
    this.saving = this.saving.then(() => this.store.saveSourceLibraries(cache)).catch(() => undefined);
  }
}

/**
 * For Favourites and Playlists, from their setup: the page's lists refresh as it opens, when the
 * sources change (one signs in or is turned off) and when the browser tab shows again.
 */
export function useSourceLibrary(app: App, page: SourceLibraryPage) {
  $effect(() => {
    void app.sources.revision;
    untrack(() => {
      app.sourceLibraries.cover();
      app.sourceLibraries.shown(page);
    });
  });
  $effect(() => {
    const again = () => {
      if (document.visibilityState === 'visible') app.sourceLibraries.shown(page);
    };
    document.addEventListener('visibilitychange', again);
    return () => document.removeEventListener('visibilitychange', again);
  });
}
