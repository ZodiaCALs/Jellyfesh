// What the views do with Jellyfesh playlists (kept in this browser, the core's rules): their cards,
// songs and More menus (docs/ui.md: Play Next, Play Last, Rename…, Export Playlist…, Delete
// Playlist…, which asks first), Import Playlist… and Save as Playlist… for a source's playlist.

import type { App } from './app';
import type { Item, PlaylistSummary, Saved } from './core/types';
import { saveFile } from './download';
import { t, tn } from './i18n/index.svelte';
import { type MenuEntry, savedEntries, savedOf, songsOf } from './items';
import { LIBRARY, trackItem } from './state/library.svelte';
import { sheets } from './state/sheets.svelte';
import { toast } from './state/toast.svelte';

/** What Import Playlist… found in a file (jf_web::m3u::Imported): the songs' paths. */
interface Imported {
  found: string[];
  lines: number;
}

/**
 * Import Playlist… (docs/ui.md): an M3U or M3U8 file becomes a playlist named after it, of the
 * library songs its lines name (jf-web's rules: a page knows no folder paths, so a line matches
 * the song whose path ends like it). Says how many were found; opens the playlist.
 */
export async function importPlaylist(app: App, file: File) {
  const name = file.name.replace(/\.(m3u8?|txt)$/i, '').trim() || file.name;
  const library = app.library.paths();
  const keyOf = new Map(library.map(({ key, path }) => [path, key]));
  let imported: Imported;
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    imported = JSON.parse(app.jf.m3u_import(bytes, JSON.stringify(library.map(({ path }) => path)))) as Imported;
  } catch (error) {
    toast(t('Couldn’t read “{name}”: {reason}', { name: file.name, reason: error instanceof Error ? error.message : String(error) }));
    return;
  }
  if (imported.lines === 0) {
    toast(t('“{name}” lists no songs.', { name: file.name }));
    return;
  }
  const keys = imported.found.flatMap((path) => keyOf.get(path) ?? []);
  const songs: Saved[] = app.library.tracksByKey(keys).flatMap((view) => (view ? [savedOf(LIBRARY, trackItem(view))] : []));
  if (songs.length === 0) {
    toast(t('None of the songs in “{name}” are in your library.', { name: file.name }));
    return;
  }
  const id = app.collections.createPlaylist(name);
  app.collections.addToPlaylist(id, songs);
  toast(
    songs.length === imported.lines
      ? t('Imported “{name}”', { name })
      : tn('Imported “{name}”: {n} of its songs is in your library.', 'Imported “{name}”: {n} of its songs are in your library.', songs.length, {
          name,
        }),
  );
  app.nav.open({ view: 'playlist', id });
}

/**
 * Export Playlist… (docs/ui.md): the playlist's library songs as an M3U8 file, each by its path
 * below its folder's parent, so the file reads back in every app when kept beside the music
 * folder. A source's songs have no file and are left out, as in the apps.
 */
export function exportPlaylist(app: App, playlist: PlaylistSummary) {
  const entries = app.collections.entries(playlist.id);
  const songs = entries.flatMap((entry) => {
    const path = entry.plugin === LIBRARY ? app.library.pathOf(entry.item.id) : null;
    return path ? [{ path, title: entry.item.title, artist: entry.item.artist, duration_ms: entry.item.duration_ms }] : [];
  });
  if (songs.length === 0) {
    toast(t('This playlist has no songs from your library to export.'));
    return;
  }
  const text = app.jf.m3u_export(JSON.stringify(songs));
  saveFile(new Blob([text], { type: 'audio/x-mpegurl' }), app.jf.m3u_file_name(playlist.name));
  const left = entries.filter((entry) => entry.plugin !== LIBRARY).length;
  if (left > 0) toast(tn('{n} song from a server was left out: it has no file.', '{n} songs from servers were left out: they have no file.', left));
}

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
    { label: t('Export Playlist…'), disabled: empty, action: () => exportPlaylist(app, playlist) },
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
