// A source's library in Jellyfesh Web (plugin API 1.3), against the mock Subsonic server: its
// starred songs, albums and artists on Favourites and its playlists on Playlists (ADR-0024),
// Save as Playlist… from one of them, the quality menu with the levels the plugin lists
// (ADR-0026), and Download, which saves the file through the browser and says so when the server
// refuses the account (Subsonic answers HTTP 200 with error 50 in JSON, which is never saved).

import { readFileSync } from 'node:fs';
import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInSubsonic, waitPlaying } from './helpers';

test.describe.configure({ mode: 'serial' });

let page: Page;

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
  await openApp(page);
  await signInSubsonic(page);
});

test.afterAll(async () => {
  await page.close();
});

test('shows the server’s starred songs, albums and artists on Favourites', async () => {
  await nav(page, 'Favourites').click();
  const section = page.getByRole('region', { name: 'Subsonic' });
  await expect(section.getByText('Updated just now')).toBeVisible();
  // Newest star first.
  const songs = section.getByRole('group', { name: 'Songs' });
  await expect(songs.getByRole('button', { name: /^Play / })).toHaveText([/Rope and Mast/, /Still Water/]);
  await expect(section.getByRole('group', { name: 'Albums' }).getByRole('button', { name: 'Open Paper Lanterns' })).toBeVisible();
  await expect(section.getByRole('group', { name: 'Artists' }).getByRole('button', { name: 'Open Tidal Choir' })).toBeVisible();
  // Nothing is hearted here, so the page has no empty state, and a starred song plays.
  await expect(page.getByRole('heading', { name: 'No Favourites Yet' })).toBeHidden();
  await songs.getByRole('button', { name: 'Play Still Water' }).click();
  await waitPlaying(page, 'Still Water');
});

test('shows the server’s playlists on Playlists, and saves one as a Jellyfesh playlist', async () => {
  await nav(page, 'Playlists').click();
  const section = page.getByRole('region', { name: 'Subsonic' });
  await expect(section.getByRole('button', { name: 'Open Evening Walk' })).toBeVisible();
  // Another account's public playlist, which the person follows.
  await expect(section.getByRole('button', { name: 'Open Harbour Mornings' })).toBeVisible();
  await section.getByRole('button', { name: 'Open Evening Walk' }).click();
  await expect(page.getByRole('heading', { name: 'Evening Walk', level: 1 })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Play Night Ferry' })).toBeVisible();

  await page.getByRole('button', { name: 'More for Evening Walk' }).click();
  await page.getByRole('menuitem', { name: 'Save as Playlist…' }).click();
  const naming = page.getByRole('dialog', { name: 'Save as Playlist' });
  await expect(naming.getByLabel('Name')).toHaveValue('Evening Walk');
  await naming.getByLabel('Name').fill('Evening Walk, Kept');
  await naming.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved as “Evening Walk, Kept”')).toBeVisible();

  await nav(page, 'Playlists').click();
  await page.getByRole('button', { name: 'Open Evening Walk, Kept' }).click();
  const kept = page.getByRole('list', { name: 'Evening Walk, Kept' });
  await expect(kept.getByRole('button', { name: /^Play / })).toHaveText([/Lanterns/, /Night Ferry/, /Still Water/]);
});

test('offers the plugin’s levels in the quality menu, and reopens the song at the new one', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour', 1000);
  await page.getByRole('button', { name: /^Quality: / }).first().click();
  const menu = page.getByRole('menu', { name: 'Streaming Quality' });
  await expect(menu.getByRole('menuitemradio')).toHaveText(['Auto', 'Lossless', 'High', 'Normal', 'Low']);
  await expect(menu.getByRole('menuitemradio', { name: 'Auto' })).toHaveAttribute('aria-checked', 'true');
  await menu.getByRole('menuitemradio', { name: 'High' }).click();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.streaming_quality)).toBe('high');
  // The song opens again at High, where it was (its stream answers ranges).
  await expect
    .poll(() => page.evaluate(() => ((globalThis as any).jellyfesh.player.trace as string[]).some((line) => / reopen \d+ at High$/.test(line))))
    .toBe(true);
  await waitPlaying(page, 'Morning Harbour', 1000);
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.streaming_quality = 'auto')));
});

test('downloads a song’s file, and says when the server refuses one', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'More for Lanterns' }).click();
  await page.getByRole('menuitem', { name: 'Download' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('03 Lanterns.wav');
  expect(readFileSync((await file.path())!).subarray(0, 4).toString('latin1')).toBe('RIFF');

  // 海の灯 may not be downloaded: the server's JSON answer is not saved as a file.
  let saved = 0;
  page.on('download', () => (saved += 1));
  await openAlbum(page, 'Subsonic', 'Paper Lanterns');
  await page.getByRole('button', { name: 'More for 海の灯' }).click();
  await page.getByRole('menuitem', { name: 'Download' }).click();
  await expect(page.getByText('Couldn’t download “海の灯”: Your Subsonic server doesn’t allow downloads for this account.')).toBeVisible();
  expect(saved).toBe(0);
});
