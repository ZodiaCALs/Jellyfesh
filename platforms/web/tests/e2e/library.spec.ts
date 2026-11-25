// The music library: a folder of generated, tagged sine tones (invented titles) added with the
// folder picker of browsers without a native one, then browsed, searched, played, kept across a
// reload (picked again to play) and removed; a playlist file imported against it and exported
// again; and a folder read through a folder handle, as Chrome and Edge's picker gives.

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import { taggedWav } from '../mock/media.mjs';
import { nav, openApp, snapshot, waitPlaying } from './helpers';

const LRC = '[00:00.30]Harbour lights\n[00:02.50]Over the water\n';
let root: string;
let folder: string;

test.beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), 'jf-library-'));
  folder = join(root, 'Music');
  mkdirSync(join(folder, 'Tide'), { recursive: true });
  mkdirSync(join(folder, 'Loose'), { recursive: true });
  writeFileSync(
    join(folder, 'Tide', '01 Harbour.wav'),
    taggedWav(4, 440, { title: 'Harbour', artist: 'Aurora', album: 'Tide', track: 1, year: 2021 }),
  );
  writeFileSync(join(folder, 'Tide', '01 Harbour.lrc'), LRC);
  writeFileSync(
    join(folder, 'Tide', '02 Lantern.wav'),
    taggedWav(4, 494, { title: 'Lantern', artist: 'Aurora', album: 'Tide', track: 2, year: 2021 }),
  );
  writeFileSync(join(folder, 'Loose', 'Solo.wav'), taggedWav(4, 523, { title: 'Solo', artist: 'Bo feat. Cy' }));
  writeFileSync(join(folder, 'Loose', 'notes.txt'), 'not music');
});

test.afterAll(() => {
  rmSync(root, { recursive: true, force: true });
});

/** The folder picker of browsers that keep no folders (Playwright cannot drive the native one). */
async function useInputPicker(page: Page) {
  await page.addInitScript(() => {
    delete (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker;
  });
}

test('adds a folder, browses, searches and plays it, and keeps it across a reload', async ({ page }) => {
  await useInputPicker(page);
  await openApp(page);
  await expect(page.getByRole('heading', { name: 'Welcome to Jellyfesh' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add Music Folder…' }).click();
  await (await chooser).setFiles(folder);

  // Home: the size line (Solo has no album; "Bo feat. Cy" is two artists) and Recently Added.
  await expect(page.getByText('3 songs · 1 album · 3 artists')).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole('region', { name: 'Recently Added' }).getByRole('button', { name: 'Open Tide' })).toBeVisible();

  await nav(page, 'Albums').click();
  await expect(page.getByRole('button', { name: 'Open Tide' })).toBeVisible();
  await nav(page, 'Artists').click();
  for (const name of ['Aurora', 'Bo', 'Cy']) await expect(page.getByRole('button', { name: `Open ${name}` })).toBeVisible();
  await nav(page, 'Songs').click();
  for (const title of ['Harbour', 'Lantern', 'Solo']) await expect(page.getByRole('button', { name: `Play ${title}` })).toBeVisible();

  // An album plays in track order, with its lyrics file.
  await nav(page, 'Albums').click();
  await page.getByRole('button', { name: 'Open Tide' }).click();
  await expect(page.getByRole('heading', { name: 'Tide', level: 1 })).toBeVisible();
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Harbour');
  await expect.poll(async () => (await snapshot(page)).lyrics).toBe('ready');

  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  await expect(page.getByRole('region', { name: 'Songs' }).getByRole('button', { name: 'Play Lantern' })).toBeVisible();

  // The catalogue and the queue come back after a reload; the folder is picked again to play.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Search', level: 1 })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(3);
  expect((await snapshot(page)).title).toBe('Harbour');
  await nav(page, 'Songs').click();
  await expect(page.getByText('Pick “Music” again to play its songs on this visit.')).toBeVisible();
  const again = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Pick Folder…' }).click();
  await (await again).setFiles(folder);
  await expect(page.getByText('Pick “Music” again to play its songs on this visit.')).toBeHidden();
  await page.getByRole('button', { name: 'Play Solo' }).click();
  await waitPlaying(page, 'Solo');
});

test('removes a folder after asking', async ({ page }) => {
  await useInputPicker(page);
  await openApp(page);
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  if (await page.getByText('No folders yet').isHidden()) {
    await page.getByRole('button', { name: 'Remove Folder…' }).first().click();
    await page.getByRole('button', { name: 'Remove Folder', exact: true }).click();
  }
  await expect(page.getByText('No folders yet')).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add Folder…' }).click();
  await (await chooser).setFiles(folder);
  await expect(page.getByText('Music', { exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(3);
  await page.getByRole('button', { name: 'Remove Folder…' }).click();
  const confirm = page.getByRole('dialog', { name: 'Remove “Music” from the library?' });
  await expect(confirm).toContainText('The files stay where they are.');
  await confirm.getByRole('button', { name: 'Remove Folder', exact: true }).click();
  await expect(page.getByText('No folders yet')).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(0);
});

test('imports a playlist file of library songs, and exports it again', async ({ page }) => {
  await useInputPicker(page);
  await openApp(page);
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  if (await page.getByText('No folders yet').isVisible()) {
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('button', { name: 'Add Folder…' }).click();
    await (await chooser).setFiles(folder);
  }
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(3);

  // Written on Windows and elsewhere: two of its lines name songs of the library; a stream and a
  // missing file are left out.
  const list = join(root, 'Road Trip.m3u8');
  writeFileSync(
    list,
    [
      '#EXTM3U',
      '#EXTINF:4,Aurora - Lantern',
      'C:\\Users\\me\\Music\\Tide\\02 Lantern.wav',
      '../Music/Loose/Solo.wav',
      'https://radio.example/stream.mp3',
      'Music/Tide/09 Missing.wav',
      '',
    ].join('\r\n'),
  );
  await nav(page, 'Playlists').click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Import Playlist…' }).first().click();
  await (await chooser).setFiles(list);
  await expect(page.getByRole('heading', { name: 'Road Trip', level: 1 })).toBeVisible();
  await expect(page.getByText('Imported “Road Trip”: 2 of its songs are in your library.')).toBeVisible();
  const songs = page.getByRole('list', { name: 'Road Trip' });
  await expect(songs.getByRole('listitem')).toHaveCount(2);
  await expect(songs.getByRole('listitem').first().getByRole('button', { name: 'Play Lantern' })).toBeVisible();

  // Export Playlist… names each song by its folder and its path in it.
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'More for Road Trip' }).click();
  await page.getByRole('menuitem', { name: 'Export Playlist…' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('Road Trip.m3u8');
  expect(readFileSync((await file.path())!, 'utf8')).toBe(
    '#EXTM3U\n#EXTINF:4,Aurora - Lantern\nMusic/Tide/02 Lantern.wav\n#EXTINF:4,Bo feat. Cy - Solo\nMusic/Loose/Solo.wav\n',
  );
});

test('reads and plays a folder through a folder handle', async ({ page }) => {
  // Chromium's origin private file system gives real folder handles, like the ones the folder
  // picker gives in Chrome and Edge.
  const song = taggedWav(3, 392, { title: 'Kept', artist: 'Dana', album: 'Handles', track: 1 }).toString('base64');
  await page.addInitScript(() => {
    (window as unknown as { showDirectoryPicker: () => Promise<FileSystemDirectoryHandle> }).showDirectoryPicker = async () => {
      const root = await navigator.storage.getDirectory();
      return root.getDirectoryHandle('Kept Music');
    };
  });
  await openApp(page);
  await page.evaluate(async (data) => {
    const root = await navigator.storage.getDirectory();
    await root.removeEntry('Kept Music', { recursive: true }).catch(() => undefined);
    const folder = await root.getDirectoryHandle('Kept Music', { create: true });
    const album = await folder.getDirectoryHandle('Handles', { create: true });
    const writable = await (await album.getFileHandle('01 Kept.wav', { create: true })).createWritable();
    await writable.write(Uint8Array.from(atob(data), (c) => c.charCodeAt(0)));
    await writable.close();
  }, song);
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  await page.getByRole('button', { name: 'Add Folder…' }).click();
  await expect(page.getByText('Kept Music', { exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(1);
  // Picked this visit: nothing to pick again.
  await expect(page.getByRole('button', { name: 'Pick Folder…' })).toHaveCount(0);

  await nav(page, 'Songs').click();
  await page.getByRole('button', { name: 'Play Kept' }).click();
  await waitPlaying(page, 'Kept');

  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  await page.getByRole('button', { name: 'Remove Folder…' }).click();
  await page.getByRole('button', { name: 'Remove Folder', exact: true }).click();
  await expect(page.getByText('No folders yet')).toBeVisible();
});
