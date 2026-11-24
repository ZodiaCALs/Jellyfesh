// What the views do with items: queue entries, the rows' second line, links that open an item in
// its service, and the "More" menus. The library's items (`LIBRARY`) play as files; everything
// else as a source's tracks.

import type { App } from './app';
import type { Item, NewEntry, Row, Saved } from './core/types';
import { t, tn } from './i18n/index.svelte';
import { refusalText } from './state/collections.svelte';
import { LIBRARY, LIBRARY_ID } from './state/library.svelte';
import { sheets } from './state/sheets.svelte';

/** One entry of a "More" menu. */
export interface MenuEntry {
  label: string;
  action: () => void;
  disabled?: boolean;
  /** A check item: whether it is on. */
  checked?: boolean;
}

/** Where Open in {Service} goes: the link the core filled in, and the plugin's name. */
export interface ItemLink {
  url: string;
  service: string;
}

/** The link that opens `item` in its service (API 1.1, docs/ui.md), or null. */
export function linkOf(app: App, plugin: string, item: Item): ItemLink | null {
  if (plugin === LIBRARY) return null;
  const source = app.sources.get(plugin);
  const url = source?.link(item) ?? null;
  return source && url ? { url, service: source.name } : null;
}

/** Opens an item's link in a new tab, which gets no hold on this page. */
export function openLink(url: string) {
  window.open(url, '_blank', 'noopener');
}

/** The Open in {Service} entry of a menu. */
function openInEntry(link: ItemLink): MenuEntry {
  return { label: t('Open in {service}', { service: link.service }), action: () => openLink(link.url) };
}

export function entryOf(plugin: string, item: Item): NewEntry {
  if (plugin === LIBRARY) return { kind: 'file', id: `${LIBRARY_ID}${item.id}` };
  return { kind: 'source', plugin, item };
}

/** A song as hearts and playlists keep it. */
export function savedOf(plugin: string, item: Item): Saved {
  return { plugin, item };
}

/** Songs kept by hearts and playlists (each with its own source) as queue entries, and where the
 *  one at `index` is among them. */
export function savedEntries(rows: Saved[], index?: number): { entries: NewEntry[]; start: number } {
  const playable = rows.map((row, i) => ({ row, i })).filter(({ row }) => row.item.kind === 'Track' && row.item.playable);
  const start = index === undefined ? 0 : Math.max(0, playable.findIndex(({ i }) => i === index));
  return { entries: playable.map(({ row }) => entryOf(row.plugin, row.item)), start };
}

/** Add to Favourites or Remove from Favourites, for a song. */
export function heartEntry(app: App, plugin: string, item: Item, toast: (text: string) => void): MenuEntry {
  const on = app.collections.isFavorite(plugin, item.id);
  return {
    label: on ? t('Remove from Favourites') : t('Add to Favourites'),
    action: () => {
      try {
        app.collections.setFavorite(savedOf(plugin, item), !on);
      } catch (error) {
        toast(refusalText(error));
      }
    },
  };
}

/** Add to Playlist…: picks a playlist for `songs`. */
export function addToPlaylistEntry(songs: () => Promise<Saved[]>, disabled = false): MenuEntry {
  return { label: t('Add to Playlist…'), disabled, action: () => (sheets.picking = { songs }) };
}

/** The tracks of a list as queue entries, and where `item` is among them. */
export function tracksFrom(plugin: string, items: Item[], item?: Item): { entries: NewEntry[]; start: number } {
  const tracks = items.filter((i) => i.kind === 'Track' && i.playable);
  const start = item ? Math.max(0, tracks.indexOf(item)) : 0;
  return { entries: tracks.map((i) => entryOf(plugin, i)), start };
}

export function secondary(item: Item): string {
  switch (item.kind) {
    case 'Track':
      return [item.artist ?? t('Unknown Artist'), item.album].filter(Boolean).join(' · ');
    case 'Album':
      return [item.artist, item.year?.toString()].filter(Boolean).join(' · ');
    case 'Artist':
      return item.child_count !== null ? tn('{n} album', '{n} albums', item.child_count) : '';
    case 'Playlist':
      return item.child_count !== null ? tn('{n} song', '{n} songs', item.child_count) : '';
    default:
      return item.child_count !== null ? tn('{n} item', '{n} items', item.child_count) : '';
  }
}

/** A length in whole minutes (at least one), with hours from an hour on: "48 minutes", "1 hour 5 minutes". */
export function minutesText(ms: number): string {
  const minutes = Math.max(1, Math.round(ms / 60_000));
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  const minutePart = tn('{n} minute', '{n} minutes', rest);
  if (hours === 0) return minutePart;
  const hourPart = tn('{n} hour', '{n} hours', hours);
  return rest === 0 ? hourPart : t('{hours} {minutes}', { hours: hourPart, minutes: minutePart });
}

/** The footer of a list of songs, after Apple Music: "12 songs, 48 minutes". */
export function summary(tracks: Item[]): string {
  const songs = tracks.filter((i) => i.kind === 'Track');
  const count = tn('{n} song', '{n} songs', songs.length);
  const ms = songs.reduce((sum, i) => sum + (i.duration_ms ?? 0), 0);
  return ms > 0 ? t('{songs}, {duration}', { songs: count, duration: minutesText(ms) }) : count;
}

function stub(kind: 'Album' | 'Artist', id: string, title: string, artist: string | null): Item {
  return {
    id,
    kind,
    title,
    artist,
    album: null,
    album_id: null,
    artist_id: null,
    duration_ms: null,
    track_number: null,
    disc_number: null,
    year: null,
    child_count: null,
    isrc: null,
    artwork: null,
    explicit: false,
    playable: true,
    unplayable: null,
  };
}

export function albumOf(item: Item): Item | null {
  return item.album_id ? stub('Album', item.album_id, item.album ?? t('Unknown Album'), item.artist) : null;
}

export function artistOf(item: Item): Item | null {
  return item.artist_id ? stub('Artist', item.artist_id, item.artist ?? t('Unknown Artist'), null) : null;
}

/**
 * The "More" menu of a row or page (docs/ui.md, "Song menu"): Play Next and Play Last (left out
 * with `queue: false`, for a page with nothing to play), Add to Playlist…, the heart (songs), Go to
 * Album, Go to Artist, Get Info, then Open in {Service} when the item has a link. `extra` comes
 * after Add to Playlist… (a playlist's own entries).
 */
export function itemMenu(
  app: App,
  plugin: string,
  item: Item,
  toast: (text: string) => void,
  { queue = true, extra = [] }: { queue?: boolean; extra?: MenuEntry[] } = {},
): MenuEntry[] {
  const menu: MenuEntry[] = [];
  if (item.kind === 'Track') {
    menu.push(
      {
        label: t('Play Next'),
        disabled: !item.playable,
        action: () => {
          app.player.enqueue([entryOf(plugin, item)], true);
          toast(t('Plays next'));
        },
      },
      {
        label: t('Play Last'),
        disabled: !item.playable,
        action: () => {
          app.player.enqueue([entryOf(plugin, item)], false);
          toast(t('Added to the queue'));
        },
      },
      addToPlaylistEntry(async () => [savedOf(plugin, item)]),
      ...extra,
      heartEntry(app, plugin, item, toast),
    );
    const album = albumOf(item);
    if (album) menu.push({ label: t('Go to Album'), action: () => app.nav.open({ view: 'browse', plugin, item: album }) });
  } else if (queue) {
    menu.push(
      { label: t('Play Next'), action: () => void queueContainer(app, plugin, item, true, toast) },
      { label: t('Play Last'), action: () => void queueContainer(app, plugin, item, false, toast) },
      addToPlaylistEntry(async () => (await songsOf(app, plugin, item)).map((song) => savedOf(plugin, song))),
      ...extra,
    );
  } else {
    menu.push(...extra);
  }
  const artist = item.kind !== 'Artist' ? artistOf(item) : null;
  if (artist) menu.push({ label: t('Go to Artist'), action: () => app.nav.open({ view: 'browse', plugin, item: artist }) });
  if (offersInfo(app, plugin, item)) menu.push({ label: t('Get Info'), action: () => (app.nav.info = { plugin, item }) });
  const link = linkOf(app, plugin, item);
  if (link) menu.push(openInEntry(link));
  return menu;
}

/** Get Info is offered for songs, and for albums and artists the library or the source describes. */
function offersInfo(app: App, plugin: string, item: Item): boolean {
  if (item.kind === 'Track') return true;
  if (item.kind !== 'Album' && item.kind !== 'Artist') return false;
  return plugin !== LIBRARY && (app.sources.get(plugin)?.declares('Details') ?? false);
}

/**
 * Now Playing's and the player bar's "More": Add to Playlist… and the heart for the song playing
 * (a dropped file has neither), Go to Album, Go to Artist and Open in {Service} for a source's
 * song, the made lyrics' entries, then Desktop Lyrics. Going to a page closes Now Playing; Back
 * returns to it.
 */
export function nowPlayingMenu(app: App, row: Row | null, toast: (text: string) => void = () => {}): MenuEntry[] {
  const menu: MenuEntry[] = [];
  const saved = row ? (JSON.parse(app.core.saved(row.uid)) as Saved | null) : null;
  if (saved) {
    // The library's songs as the catalogue lists them now (with their album and artist).
    const [song] = app.collections.current([saved]);
    menu.push(addToPlaylistEntry(async () => [song]), heartEntry(app, song.plugin, song.item, toast));
    if (song.plugin === LIBRARY) {
      const album = albumOf(song.item);
      if (album) menu.push({ label: t('Go to Album'), action: () => app.nav.open({ view: 'browse', plugin: LIBRARY, item: album }) });
      const artist = artistOf(song.item);
      if (artist) menu.push({ label: t('Go to Artist'), action: () => app.nav.open({ view: 'browse', plugin: LIBRARY, item: artist }) });
    }
  }
  const found = row ? app.player.sourceItem(row) : null;
  if (found) {
    const plugin = found.plugin.id;
    const album = albumOf(found.item);
    if (album) menu.push({ label: t('Go to Album'), action: () => app.nav.open({ view: 'browse', plugin, item: album }) });
    const artist = artistOf(found.item);
    if (artist) menu.push({ label: t('Go to Artist'), action: () => app.nav.open({ view: 'browse', plugin, item: artist }) });
    const link = linkOf(app, plugin, found.item);
    if (link) menu.push(openInEntry(link));
  }
  // Made lyrics: Make Again, and Delete Made Lyrics (asks first, in Now Playing).
  if (row && app.aiLyrics.hasMade(row)) {
    menu.push(
      { label: t('Make Again'), action: () => void app.aiLyrics.make(row) },
      { label: t('Delete Made Lyrics'), action: () => (app.aiLyrics.deleting = row) },
    );
  }
  // At the end, as in the apps: the setting as a check item.
  if (row && app.desktopLyrics.supported) {
    const on = app.settings.value.desktop_lyrics;
    menu.push({
      label: t('Desktop Lyrics'),
      checked: on && app.desktopLyrics.shown,
      action: () => {
        if (on && !app.desktopLyrics.shown) void app.desktopLyrics.show();
        else app.settings.update((d) => (d.desktop_lyrics = !on));
      },
    });
  }
  return menu;
}

/** Every song in a container (first pages), for Play, Shuffle, Add to Playlist… and the queue commands. */
export async function songsOf(app: App, plugin: string, container: Item, limit = 500): Promise<Item[]> {
  if (plugin === LIBRARY) return app.library.browse(container).items.filter((i) => i.kind === 'Track');
  const source = app.sources.get(plugin);
  if (!source) return [];
  const songs: Item[] = [];
  let cursor: string | null = null;
  do {
    const page = await source.browse(container, cursor, 100);
    for (const item of page.items) if (item.kind === 'Track') songs.push(item);
    cursor = page.next;
  } while (cursor && songs.length < limit);
  return songs;
}

/** Every playable track in a container (first pages), as queue entries. */
export async function tracksOf(app: App, plugin: string, container: Item, limit = 500): Promise<NewEntry[]> {
  return (await songsOf(app, plugin, container, limit)).filter((i) => i.playable).map((i) => entryOf(plugin, i));
}

async function queueContainer(app: App, plugin: string, item: Item, next: boolean, toast: (text: string) => void) {
  const entries = await tracksOf(app, plugin, item);
  app.player.enqueue(entries, next);
  toast(next ? t('Plays next') : t('Added to the queue'));
}
