// The sources' likes and playlists on the page: the age line of a kept list, and the API 1.3
// answers as the plugin host reads them from a component.

import { describe, expect, it } from 'vitest';
import { updatedText } from '../../src/lib/i18n/index.svelte';
import { downloadFromJco, playlistPageFromJco } from '../../src/lib/plugins/convert';

describe('the age of a kept list', () => {
  const at = Date.UTC(2026, 10, 24, 12, 0, 0);
  it('says just now within a minute, then minutes, hours and days', () => {
    expect(updatedText(at, at + 20_000)).toBe('Updated just now');
    expect(updatedText(at, at - 5_000)).toBe('Updated just now');
    expect(updatedText(at, at + 5 * 60_000 + 30_000)).toBe('Updated 5 minutes ago');
    expect(updatedText(at, at + 61_000)).toBe('Updated 1 minute ago');
    expect(updatedText(at, at + 3 * 3_600_000)).toBe('Updated 3 hours ago');
    expect(updatedText(at, at + 2 * 86_400_000 + 5_000)).toBe('Updated 2 days ago');
  });
});

describe('API 1.3 answers', () => {
  const item = { id: 'p1', kind: 'playlist', title: 'Road Trip', explicit: false, playable: true };
  it('reads a page of playlists', () => {
    const page = playlistPageFromJco({ entries: [{ item, owned: false, songsListed: false }], next: 'c2', total: 7 });
    expect(page.next).toBe('c2');
    expect(page.total).toBe(7);
    expect(page.entries[0].item.kind).toBe('Playlist');
    expect(page.entries[0].item.title).toBe('Road Trip');
    expect(page.entries[0].owned).toBe(false);
    expect(page.entries[0].songs_listed).toBe(false);
    expect(playlistPageFromJco({ entries: [] }).next).toBeNull();
  });

  it('reads where to download a file', () => {
    const info = downloadFromJco({
      url: 'https://music.example/rest/download?id=1',
      headers: [{ name: 'X-Token', value: 't' }],
      container: 'flac',
      size: 12345678n,
      original: true,
      fileName: undefined,
    });
    expect(info).toEqual({
      url: 'https://music.example/rest/download?id=1',
      headers: [{ name: 'X-Token', value: 't' }],
      container: 'flac',
      size: 12345678,
      original: true,
      file_name: null,
    });
  });
});
