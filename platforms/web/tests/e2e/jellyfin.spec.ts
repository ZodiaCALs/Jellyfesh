// Jellyfesh Web against the mock Jellyfin server: Quick Connect sign-in (a code approved
// elsewhere), browsing, streams that need an Authorization header, lyrics, search and sign-out.

import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInJellyfinQuickConnect, snapshot, waitPlaying } from './helpers';

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
  await nav(page, 'Sources').click();
  await expect(page.locator('li.source', { hasText: 'Jellyfin' })).toContainText('Signed in as demo');
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

test('searches, and signs out', async () => {
  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('still');
  await expect(page.getByRole('region', { name: 'Jellyfin' }).getByRole('button', { name: 'Play Still Water' })).toBeVisible();
  await nav(page, 'Sources').click();
  const card = page.locator('li.source', { hasText: 'Jellyfin' });
  await card.getByRole('button', { name: 'Sign Out' }).click();
  await expect(card).toContainText('Not signed in');
});
