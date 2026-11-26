// Jellyfesh Web against the mock Jellyfin server: Quick Connect sign-in (a code approved
// elsewhere), browsing, streams that need an Authorization header, lyrics, the account's
// favourites and a download with its header (plugin API 1.3), search and sign-out.

import { readFileSync } from 'node:fs';
import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, openSourcesSettings, signInJellyfinQuickConnect, snapshot, sourceCard, waitPlaying } from './helpers';

test.describe.configure({ mode: 'serial' });

let page: Page;

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
  await openApp(page);
});

test.afterAll(async () => {
  await page.close();
});

test('signs in with Quick Connect', async () => {
  await signInJellyfinQuickConnect(page);
  await openSourcesSettings(page);
  await expect(sourceCard(page, 'Jellyfin')).toContainText('Signed in as demo');
});

test('plays an album whose streams need the Authorization header', async () => {
  // With the service worker in control, the element loads a same-origin __stream/ URL and the
  // worker adds the header; the token never appears in a URL.
  const streamed: string[] = [];
  page.on('request', (request) => {
    if (request.url().includes('/__stream/')) streamed.push(request.url());
    expect(request.url()).not.toMatch(/api_key=|Token=/i);
  });
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true);
  await openAlbum(page, 'Jellyfin', 'Paper Lanterns');
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Night Ferry');
  await page.keyboard.press('Control+ArrowRight');
  await waitPlaying(page, '海の灯', 300);
  await page.keyboard.press('Control+l');
  const lyrics = page.getByRole('region', { name: 'Lyrics' });
  // Line-timed Japanese with the English lines as their translation.
  await expect(lyrics.locator('button[aria-current="true"]')).toHaveText(/灯台のあかり|波にゆれて/, { timeout: 10_000 });
  await expect(lyrics.getByText('Swaying on the waves')).toBeVisible();
  await page.keyboard.press('Escape');
  expect((await snapshot(page)).problem).toBeNull();
  expect(streamed.length).toBeGreaterThan(0);
});

test('lists the account’s favourites, and downloads a song with its Authorization header', async () => {
  await nav(page, 'Favourites').click();
  const section = page.getByRole('region', { name: 'Jellyfin' });
  // Jellyfin keeps no date for a favourite: by name, as in its own apps.
  await expect(section.getByRole('group', { name: 'Songs' }).getByRole('button', { name: /^Play / })).toHaveText([/Rope and Mast/, /Still Water/]);
  await expect(section.getByRole('group', { name: 'Albums' }).getByRole('button', { name: 'Open Paper Lanterns' })).toBeVisible();
  await expect(section.getByRole('group', { name: 'Artists' }).getByRole('button', { name: 'Open Tidal Choir' })).toBeVisible();
  // The original from /Items/{id}/Download, which answers only with the session's header.
  const download = page.waitForEvent('download');
  await section.getByRole('button', { name: 'More for Still Water' }).click();
  await page.getByRole('menuitem', { name: 'Download' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('04 Still Water.wav');
  expect(readFileSync((await file.path())!).subarray(0, 4).toString('latin1')).toBe('RIFF');
});

test('searches, and signs out', async () => {
  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('still');
  await expect(page.getByRole('region', { name: 'Jellyfin' }).getByRole('button', { name: 'Play Still Water' })).toBeVisible();
  await openSourcesSettings(page);
  const card = sourceCard(page, 'Jellyfin');
  await card.getByRole('button', { name: 'Sign Out' }).click();
  await expect(card).toContainText('Not signed in');
});
