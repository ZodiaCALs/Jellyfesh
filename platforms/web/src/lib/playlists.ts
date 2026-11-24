// What the views do with Jellyfesh playlists (kept in this browser, the core's rules): their cards,
// songs and More menus (docs/ui.md: Play Next, Play Last, Rename…, Delete Playlist…, which asks
// first), and Save as Playlist… for a source's playlist.

import type { App } from './app';
import type { Item, PlaylistSummary, Saved } from './core/types';
import { t } from './i18n/index.svelte';
import { type MenuEntry, savedEntries, savedOf, songsOf } from './items';
import { sheets } from './state/sheets.svelte';
import { toast } from './state/toast.svelte';

/** A Jellyfesh playlist as a card shows it. */
export function playlistItem(playlist: PlaylistSummary): Item {
  return {
    id: `jellyfesh-playlist:${playlist.id}`,
    kind: 'Playlist',
    title: playlist.name,
    artist: null,
    album: null,
    album_id: null,
    artist_id: null,
    duration_ms: playlist.duration_ms || null,
    track_number: null,
    disc_number: null,
    year: null,
    child_count: playlist.entry_count,
    isrc: null,
    artwork: null,
    explicit: false,
    playable: true,
    unplayable: null,
  };
}

/** A playlist's songs, in order. */
export function playlistSongs(app: App, id: number): Saved[] {
  return app.collections.entries(id).map((entry) => ({ plugin: entry.plugin, item: entry.item }));
}

/** Rename…: asks for the new name. */
export function renamePlaylist(app: App, playlist: PlaylistSummary) {
  sheets.naming = {
    title: t('Rename Playlist'),
    action: t('Rename'),
    initial: playlist.name,
    submit: (name) => app.collections.renamePlaylist(playlist.id, name),
  };
}

/** A playlist's More menu; `then` runs after Delete Playlist… (its own page goes back). */
export function playlistMenu(app: App, playlist: PlaylistSummary, then?: () => void): MenuEntry[] {
  const queue = (next: boolean) => {
    const { entries } = savedEntries(playlistSongs(app, playlist.id));
    app.player.enqueue(entries, next);
    toast(next ? t('Plays next') : t('Added to the queue'));
  };
  const empty = playlist.entry_count === 0;
  return [
    { label: t('Play Next'), disabled: empty, action: () => queue(true) },
    { label: t('Play Last'), disabled: empty, action: () => queue(false) },
    { label: t('Rename…'), action: () => renamePlaylist(app, playlist) },
    { label: t('Delete Playlist…'), action: () => (sheets.deleting = { id: playlist.id, name: playlist.name, then }) },
  ];
}

/** Save as Playlist… (a source's playlist, ADR-0024): asks for the name, then keeps its songs. */
export function saveAsPlaylist(app: App, plugin: string, playlist: Item) {
  sheets.naming = {
    title: t('Save as Playlist'),
    action: t('Save'),
    initial: playlist.title,
    submit: (name) => {
      const id = app.collections.createPlaylist(name);
      void songsOf(app, plugin, playlist, 10_000)
        .then((songs) => {
          app.collections.addToPlaylist(
            id,
            songs.map((song) => savedOf(plugin, song)),
          );
          toast(t('Saved as “{name}”', { name: app.collections.playlist(id)?.name ?? name }));
        })
        .catch((error: unknown) => toast(error instanceof Error ? error.message : String(error)));
    },
  };
}
