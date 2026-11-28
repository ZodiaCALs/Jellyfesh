// The v0.0.9 rules behind rows, cards, Favourites and Search (docs/ui.md, "Rows and cards",
// "Favourites", "Search"; ADR-0027): which sources a search asks, how Favourites shows a source
// whose likes came back empty or failed, the words of a plugin's error, a card's menu, and the
// song menu of a source's song wherever it shows (its album, artist and link).

import { describe, expect, it } from 'vitest';
import type { App } from '../../src/lib/app';
import type { Capability, Item, Row, SourceLibrary, SourceLibraryList } from '../../src/lib/core/types';
import { cardMenu, itemMenu, rowLinks } from '../../src/lib/items';
import { pluginErrorText } from '../../src/lib/plugins/convert';
import { LIBRARY } from '../../src/lib/state/library.svelte';
import { shownOn } from '../../src/lib/state/sourceLibrary.svelte';
import { isReady, type ReadySource } from '../../src/lib/state/sources.svelte';

function item(kind: Item['kind'], id: string, more: Partial<Item> = {}): Item {
  return {
    id,
    kind,
    title: `Invented ${kind} ${id}`,
    artist: null,
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
    ...more,
  };
}

describe('the sources a search asks (ADR-0027 §3)', () => {
  function source(capabilities: Capability[], { blocked = null as unknown, account = null as string | null } = {}): ReadySource {
    return { blocked, declares: (c) => capabilities.includes(c), state: { account } };
  }

  it('asks a source that is on, searches and is signed in', () => {
    expect(isReady(source(['Auth', 'Search'], { account: 'demo' }), 'Search')).toBe(true);
  });

  it('asks a source that needs no sign-in', () => {
    expect(isReady(source(['Search']), 'Search')).toBe(true);
  });

  it('leaves out a source that needs a sign-in and has none', () => {
    expect(isReady(source(['Auth', 'Search']), 'Search')).toBe(false);
  });

  it('leaves out a source that is off, or a community plugin while they are not allowed', () => {
    expect(isReady(source(['Search'], { blocked: new Error('Turned off in Sources.'), account: 'demo' }), 'Search')).toBe(false);
  });

  it('leaves out a source that does not search', () => {
    expect(isReady(source(['Auth', 'Browse'], { account: 'demo' }), 'Search')).toBe(false);
  });
});

describe('a plugin’s error, in words', () => {
  it('shows the text a plugin writes for people without a code’s prefix', () => {
    expect(pluginErrorText({ InvalidInput: 'Enter your server’s address in Sources.' })).toBe('Enter your server’s address in Sources.');
    expect(pluginErrorText({ Service: 'The server is busy.' })).toBe('The server is busy.');
  });

  it('keeps saying what kind of problem the others are', () => {
    expect(pluginErrorText({ Network: 'timed out' })).toBe('network error: timed out');
    expect(pluginErrorText('AuthRequired')).toBe('you need to log in');
  });
});

describe('a source on Favourites', () => {
  const NOW = Date.UTC(2026, 10, 27, 12);

  function list(part: SourceLibraryList['part'], more: Partial<SourceLibraryList> = {}): SourceLibraryList {
    return { part, entries: [], total: null, next: null, refreshed_at_ms: NOW, loading: false, error: null, ...more };
  }

  function library(lists: SourceLibraryList[]): SourceLibrary {
    return { plugin: 'org.example.music', name: 'Example Music', attribution: null, lists };
  }

  const liked = { item: item('Track', 's1'), owned: false, songs_listed: true };

  it('shows the lists with something in them', () => {
    const shown = shownOn('favourites', library([list('liked_songs', { entries: [liked] }), list('liked_albums'), list('playlists')]));
    expect(shown?.empty).toBe(false);
    expect(shown?.lists.map((l) => l.part)).toEqual(['liked_songs']);
  });

  it('still shows a source whose likes all came back empty, as empty', () => {
    const shown = shownOn('favourites', library([list('liked_songs'), list('liked_albums'), list('liked_artists')]));
    expect(shown).not.toBeNull();
    expect(shown?.empty).toBe(true);
    expect(shown?.name).toBe('Example Music');
  });

  it('shows why its likes failed, not that it likes nothing', () => {
    const shown = shownOn('favourites', library([list('liked_songs'), list('liked_albums', { error: 'The server is busy.' })]));
    expect(shown?.empty).toBe(false);
    expect(shown?.lists.map((l) => [l.part, l.error])).toEqual([['liked_albums', 'The server is busy.']]);
  });

  it('shows a refresh on its way', () => {
    const shown = shownOn('favourites', library([list('liked_songs', { loading: true, refreshed_at_ms: null })]));
    expect(shown?.empty).toBe(false);
    expect(shown?.lists).toHaveLength(1);
  });

  it('shows nothing of a source that keeps no likes, or before it was asked', () => {
    expect(shownOn('favourites', library([list('playlists', { entries: [liked] })]))).toBeNull();
    expect(shownOn('favourites', library([list('liked_songs', { refreshed_at_ms: null })]))).toBeNull();
  });

  it('leaves a source without playlists off Playlists', () => {
    expect(shownOn('playlists', library([list('liked_songs'), list('playlists')]))).toBeNull();
  });
});

describe('menus', () => {
  const SERVICE = 'Example Music';
  const PLUGIN = 'org.example.music';

  /** An app with one source, which links its songs, and no hearts. */
  function app(): App {
    const source = {
      id: PLUGIN,
      name: SERVICE,
      blocked: null,
      declares: (c: string) => ['Auth', 'Search', 'Browse'].includes(c),
      link: (i: Item) => `https://music.example.com/${i.kind.toLowerCase()}/${i.id}`,
    };
    return {
      sources: { get: (id: string) => (id === PLUGIN ? source : undefined) },
      collections: { isFavorite: () => false },
      nav: { open: () => {} },
      player: {
        sourceItem: (row: Row) => (row.kind === 'source' ? { plugin: source, item: song } : null),
      },
    } as unknown as App;
  }

  const song = item('Track', 't1', { artist: 'Someone', artist_id: 'ar1', album: 'Somewhere', album_id: 'al1' });

  it('offers a source’s song its album, artist and link wherever it shows', () => {
    const labels = itemMenu(app(), PLUGIN, song, () => {}).map((e) => e.label);
    expect(labels).toEqual([
      'Play Next',
      'Play Last',
      'Add to Playlist…',
      'Add to Favourites',
      'Go to Album',
      'Go to Artist',
      'Get Info',
      `Open in ${SERVICE}`,
    ]);
  });

  it('offers a queue entry’s album, artist and link', () => {
    const row: Row = {
      uid: 7,
      kind: 'source',
      plugin: PLUGIN,
      id: 't1',
      title: song.title,
      artist: song.artist,
      album: song.album,
      duration_ms: null,
      artwork_url: null,
      album_key: null,
    };
    expect(rowLinks(app(), row).map((e) => e.label)).toEqual(['Go to Album', 'Go to Artist', `Open in ${SERVICE}`]);
  });

  it('starts an album card’s menu with Play and Shuffle, then its More entries', () => {
    const album = item('Album', 'al1', { artist: 'Someone', artist_id: 'ar1' });
    expect(cardMenu(app(), LIBRARY, album, () => {}).map((e) => e.label)).toEqual([
      'Play',
      'Shuffle',
      'Play Next',
      'Play Last',
      'Add to Playlist…',
      'Go to Artist',
    ]);
    expect(cardMenu(app(), PLUGIN, item('Playlist', 'p1'), () => {}).map((e) => e.label)).toEqual([
      'Play',
      'Shuffle',
      'Play Next',
      'Play Last',
      'Add to Playlist…',
      `Open in ${SERVICE}`,
    ]);
  });

  it('adds Play and Shuffle before a card’s own entries, and nothing to an artist', () => {
    const more = () => [{ label: 'Rename…', action: () => {} }];
    expect(cardMenu(app(), LIBRARY, item('Playlist', 'jellyfesh-playlist:1'), () => {}, { more }).map((e) => e.label)).toEqual([
      'Play',
      'Shuffle',
      'Rename…',
    ]);
    expect(cardMenu(app(), LIBRARY, item('Artist', 'ar1'), () => {}).map((e) => e.label)).not.toContain('Play');
  });
});
