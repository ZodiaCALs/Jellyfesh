// What the views do with items: queue entries, the rows' second line, and the "More" menu.

import type { App } from './app';
import type { Item, NewEntry } from './core/types';
import { t, tn } from './i18n/index.svelte';

export function entryOf(plugin: string, item: Item): NewEntry {
  return { kind: 'source', plugin, item };
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

/** The "More" menu of a row. */
export function itemMenu(app: App, plugin: string, item: Item, toast: (text: string) => void) {
  const menu: { label: string; action: () => void; disabled?: boolean }[] = [];
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
    );
    const album = albumOf(item);
    if (album) menu.push({ label: t('Go to Album'), action: () => app.nav.open({ view: 'browse', plugin, item: album }) });
  } else {
    menu.push(
      { label: t('Play Next'), action: () => void queueContainer(app, plugin, item, true, toast) },
      { label: t('Play Last'), action: () => void queueContainer(app, plugin, item, false, toast) },
    );
  }
  const artist = item.kind !== 'Artist' ? artistOf(item) : null;
  if (artist) menu.push({ label: t('Go to Artist'), action: () => app.nav.open({ view: 'browse', plugin, item: artist }) });
  return menu;
}

/** Every track in a container (first pages), for Play, Shuffle and the queue commands. */
export async function tracksOf(app: App, plugin: string, container: Item, limit = 500): Promise<NewEntry[]> {
  const source = app.sources.get(plugin);
  if (!source) return [];
  const entries: NewEntry[] = [];
  let cursor: string | null = null;
  do {
    const page = await source.browse(container, cursor, 100);
    for (const item of page.items) if (item.kind === 'Track' && item.playable) entries.push(entryOf(plugin, item));
    cursor = page.next;
  } while (cursor && entries.length < limit);
  return entries;
}

async function queueContainer(app: App, plugin: string, item: Item, next: boolean, toast: (text: string) => void) {
  const entries = await tracksOf(app, plugin, item);
  app.player.enqueue(entries, next);
  toast(next ? t('Plays next') : t('Added to the queue'));
}
