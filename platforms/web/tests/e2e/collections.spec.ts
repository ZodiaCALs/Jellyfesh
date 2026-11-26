// Hearts, Jellyfesh playlists and plays in Jellyfesh Web, against the mock Subsonic server (kept
// in this browser; docs/ui.md "Favourites", "Playlists" and Home): the heart in a song's More
// menu, Favourites, New Playlist… from Add to Playlist…, a playlist's page with Play, Rename… and
// Delete Playlist… (which asks first), Home's Recently Played and Most Played, and the player
// bar's quality badge with Streaming Quality in Settings.

import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInSubsonic, snapshot, waitPlaying } from './helpers';

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

/** Chooses `entry` in the More menu of the song row `title`. */
async function songMenu(title: string, entry: string) {
  await page.getByRole('button', { name: `More for ${title}` }).first().click();
  await page.getByRole('menuitem', { name: entry }).click();
}

test('marks songs with the heart, and lists them in Favourites', async () => {
  // Nothing hearted yet (the server's own stars are a section of their own: sources-library.spec.ts).
  await nav(page, 'Favourites').click();
  await expect(page.getByRole('region', { name: 'Subsonic' })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Favourites' })).toHaveCount(0);
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await songMenu('Lanterns', 'Add to Favourites');
  await songMenu('Morning Harbour', 'Add to Favourites');
  // The menu now offers the other way.
  await page.getByRole('button', { name: 'More for Lanterns' }).first().click();
  await expect(page.getByRole('menuitem', { name: 'Remove from Favourites' })).toBeVisible();
  await page.keyboard.press('Escape');

  await nav(page, 'Favourites').click();
  const list = page.getByRole('list', { name: 'Favourites' });
  // The latest first.
  await expect(list.getByRole('listitem')).toHaveCount(2);
  await expect(list.getByRole('listitem').first().getByRole('button', { name: 'Play Morning Harbour' })).toBeVisible();
  await list.getByRole('button', { name: 'Play Lanterns' }).click();
  await waitPlaying(page, 'Lanterns');

  await songMenu('Morning Harbour', 'Remove from Favourites');
  await expect(list.getByRole('listitem')).toHaveCount(1);
  // Hearts are kept in this browser, across a reload (which comes back to Favourites).
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Favourites', level: 1 })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Favourites' }).getByRole('button', { name: 'Play Lanterns' })).toBeVisible();
});

test('makes a playlist from Add to Playlist…, plays, renames and deletes it', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await songMenu('Rope and Mast', 'Add to Playlist…');
  const picker = page.getByRole('dialog', { name: 'Add to Playlist' });
  await picker.getByRole('button', { name: 'New Playlist…' }).click();
  const naming = page.getByRole('dialog', { name: 'New Playlist' });
  // A name is needed.
  await naming.getByRole('button', { name: 'Create' }).click();
  await expect(naming.getByRole('alert')).toHaveText('Enter a name.');
  await naming.getByLabel('Name').fill('Harbour Songs');
  await naming.getByRole('button', { name: 'Create' }).click();
  await expect(naming).toBeHidden();
  await songMenu('Still Water', 'Add to Playlist…');
  await page.getByRole('dialog', { name: 'Add to Playlist' }).getByRole('button', { name: /^Harbour Songs/ }).click();

  await nav(page, 'Playlists').click();
  await page.getByRole('button', { name: 'Open Harbour Songs' }).click();
  await expect(page.getByRole('heading', { name: 'Harbour Songs', level: 1 })).toBeVisible();
  const songs = page.getByRole('list', { name: 'Harbour Songs' });
  await expect(songs.getByRole('listitem')).toHaveCount(2);
  await expect(songs.getByRole('listitem').first().getByRole('button', { name: 'Play Rope and Mast' })).toBeVisible();
  // ↓ on a song's handle moves it down.
  await songs.getByRole('button', { name: 'Move Rope and Mast' }).press('ArrowDown');
  await expect(songs.getByRole('listitem').first().getByRole('button', { name: 'Play Still Water' })).toBeVisible();
  await page.getByRole('region', { name: 'Harbour Songs' }).getByRole('button', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Still Water');

  await page.getByRole('button', { name: 'More for Harbour Songs' }).click();
  await page.getByRole('menuitem', { name: 'Rename…' }).click();
  const rename = page.getByRole('dialog', { name: 'Rename Playlist' });
  await rename.getByLabel('Name').fill('Tide Songs');
  await rename.getByRole('button', { name: 'Rename' }).click();
  await expect(page.getByRole('heading', { name: 'Tide Songs', level: 1 })).toBeVisible();

  await page.getByRole('button', { name: 'More for Tide Songs' }).click();
  await page.getByRole('menuitem', { name: 'Delete Playlist…' }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete “Tide Songs”?' });
  await expect(confirm).toContainText('The songs stay in your library.');
  await confirm.getByRole('button', { name: 'Delete Playlist' }).click();
  await nav(page, 'Playlists').click();
  await expect(page.getByRole('region', { name: 'Subsonic' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open Tide Songs' })).toHaveCount(0);
});

test('shows what played on Home', async () => {
  // Lanterns plays to its end, which counts as a play.
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Lanterns' }).click();
  await waitPlaying(page, 'Lanterns');
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(7000));
  await waitPlaying(page, 'Still Water');
  await nav(page, 'Home').click();
  await expect(page.getByRole('region', { name: 'Recently Played' }).getByRole('button', { name: 'Play Lanterns' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Most Played' }).getByRole('button', { name: 'Play Lanterns' })).toBeVisible();
});

test('shows the quality badge, and Streaming Quality offers Auto', async () => {
  expect((await snapshot(page)).playing).toBe(true);
  const badge = page.getByRole('button', { name: /^Quality: / }).first();
  await badge.click();
  const menu = page.getByRole('menu', { name: 'Streaming Quality' });
  await expect(menu).toBeVisible();
  // A source's levels, or why there are none.
  await expect(menu.getByRole('menuitemradio', { name: 'Auto' }).or(menu.getByText(/^Quality is set in /))).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Playback' }).click();
  await expect(page.getByRole('combobox', { name: /^Streaming Quality/ })).toHaveValue('auto');
});
