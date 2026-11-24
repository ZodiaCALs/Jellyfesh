// Hearts, Jellyfesh playlists and plays (Favourites, Playlists, Recently Played and Most Played),
// kept in this browser (docs/ui.md: "Hearts, playlists and history are kept in the browser"). The
// rules are the core's (`jf_web::collections`, jf-library's rules); this keeps what the core
// writes in the library's IndexedDB (store `collections`), tells the views when it changes, and
// hands the views the library's songs as the catalogue lists them now (their covers, album and
// artist), or as they were kept when their folder has gone.

import type { Jf } from '../core/wasm';
import type { CollectionError, Item, ItemStats, LibTrackView, PlaylistEntry, PlaylistSummary, Saved } from '../core/types';
import { t } from '../i18n/index.svelte';
import type { LibraryStore } from '../library/store';
import { LIBRARY, trackItem } from './library.svelte';

/** Changes are written this long after the last one (and when the page hides). */
const SAVE_MS = 300;

/** A refusal in words (the core's code is the error's message). */
export function refusalText(error: unknown): string {
  const code = (error instanceof Error ? error.message : String(error)) as CollectionError;
  switch (code) {
    case 'empty-name':
      return t('Enter a name.');
    case 'no-playlist':
      return t('This playlist no longer exists.');
    case 'not-a-song':
      return t('Only songs can be added.');
    default:
      return String(code);
  }
}

/** Looks songs of the library up by key: the catalogue's row, or null when its folder went. */
export type LibraryLookup = (keys: string[]) => (LibTrackView | null)[];

export class Collections {
  /** Bumps on every change, so views read again. */
  revision = $state(0);
  private inner: InstanceType<Jf['Collections']>;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private dirty = false;

  constructor(
    private readonly jf: Jf,
    private readonly store: LibraryStore,
    private readonly lookup: LibraryLookup,
  ) {
    this.inner = new jf.Collections();
    addEventListener('pagehide', () => void this.flush());
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') void this.flush();
    });
  }

  /** Loads what earlier visits kept. */
  async init() {
    const state = await this.store.collections().catch(() => null);
    if (state) {
      this.inner.free();
      this.inner = new this.jf.Collections(state);
    }
    this.revision += 1;
  }

  // ---- reading --------------------------------------------------------------------------------

  playlists(): PlaylistSummary[] {
    return JSON.parse(this.inner.playlists()) as PlaylistSummary[];
  }

  playlist(id: number): PlaylistSummary | null {
    return JSON.parse(this.inner.playlist(id)) as PlaylistSummary | null;
  }

  /** A playlist's entries, the library's songs as the catalogue has them now. */
  entries(id: number, offset = 0, limit = 100_000): PlaylistEntry[] {
    const entries = JSON.parse(this.inner.entries(id, offset, limit)) as PlaylistEntry[];
    const fresh = this.fresh(entries);
    return entries.map((entry, i) => ({ ...entry, item: fresh[i] }));
  }

  favorites(offset = 0, limit = 100_000): ItemStats[] {
    return this.freshStats(JSON.parse(this.inner.favorites(offset, limit)) as ItemStats[]);
  }

  favoriteCount(): number {
    return this.inner.favorite_count();
  }

  isFavorite(plugin: string, id: string): boolean {
    return this.inner.is_favorite(plugin, id);
  }

  recentlyPlayed(limit: number): ItemStats[] {
    return this.freshStats(JSON.parse(this.inner.recently_played(limit)) as ItemStats[]);
  }

  mostPlayed(limit: number): ItemStats[] {
    return this.freshStats(JSON.parse(this.inner.most_played(limit)) as ItemStats[]);
  }

  /** Songs as the views list them: the library's as the catalogue has them now. */
  current(rows: Saved[]): Saved[] {
    const fresh = this.fresh(rows);
    return rows.map((row, i) => ({ plugin: row.plugin, item: fresh[i] }));
  }

  // ---- changing (each throws the core's refusal) ------------------------------------------------

  createPlaylist(name: string): number {
    return this.change(() => this.inner.create_playlist(name, Date.now()));
  }

  renamePlaylist(id: number, name: string) {
    this.change(() => this.inner.rename_playlist(id, name, Date.now()));
  }

  deletePlaylist(id: number) {
    this.change(() => this.inner.delete_playlist(id));
  }

  /** Adds songs at `at` (at the end without it); returns their entry ids. */
  addToPlaylist(id: number, songs: Saved[], at?: number): number[] {
    return this.change(() => JSON.parse(this.inner.add_to_playlist(id, JSON.stringify(songs), at, Date.now())) as number[]);
  }

  removeFromPlaylist(id: number, entryIds: number[]) {
    this.change(() => this.inner.remove_from_playlist(id, JSON.stringify(entryIds), Date.now()));
  }

  moveEntry(id: number, entryId: number, to: number) {
    this.change(() => this.inner.move_playlist_entry(id, entryId, to, Date.now()));
  }

  /** The heart on a song, or not. */
  setFavorite(song: Saved, on: boolean): boolean {
    return this.change(() => this.inner.set_favorite(JSON.stringify(song), on, Date.now()));
  }

  /** A play that ended: counted or a skip, from when it started. */
  recordPlay(song: Saved, startedMs: number, counted: boolean) {
    this.change(() => this.inner.record_play(JSON.stringify(this.withCover(song)), startedMs, counted));
  }

  // ---- internals ------------------------------------------------------------------------------

  private change<T>(run: () => T): T {
    const result = run();
    this.revision += 1;
    this.dirty = true;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), SAVE_MS);
    return result;
  }

  private async flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (!this.dirty) return;
    this.dirty = false;
    try {
      await this.store.saveCollections(this.inner.state());
    } catch {
      // Storage full or blocked: kept for this visit.
    }
  }

  /** The items of `rows`, the library's songs as the catalogue has them now. */
  private fresh(rows: { plugin: string; item: Item }[]): Item[] {
    const keys = rows.filter((r) => r.plugin === LIBRARY).map((r) => r.item.id);
    const found = keys.length > 0 ? this.lookup(keys) : [];
    let next = 0;
    return rows.map((row) => {
      if (row.plugin !== LIBRARY) return row.item;
      const view = found[next++];
      return view ? trackItem(view) : gone(row.item);
    });
  }

  private freshStats(rows: ItemStats[]): ItemStats[] {
    const fresh = this.fresh(rows);
    return rows.map((row, i) => ({ ...row, item: fresh[i] }));
  }

  /** A library song as the catalogue lists it (with its cover), for keeping. */
  private withCover(song: Saved): Saved {
    if (song.plugin !== LIBRARY) return song;
    const [view] = this.lookup([song.item.id]);
    return view ? { plugin: LIBRARY, item: trackItem(view) } : song;
  }
}

/** A kept library song whose folder has gone: listed, but it cannot play. */
function gone(item: Item): Item {
  return { ...item, playable: false, unplayable: { reason: 'Unavailable', message: t('No longer in your library') } };
}
