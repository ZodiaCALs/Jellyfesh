// Screenshots for docs/screenshots/web/, taken from the app running against the mock servers
// (invented library, generated covers): every file name ends in -mock-server.png. Only with
// JF_SCREENSHOTS=1:  JF_SCREENSHOTS=1 npx playwright test screenshots --trace off

import { fileURLToPath } from 'node:url';
import { type Browser, expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInJellyfinQuickConnect, signInSubsonic, waitPlaying } from './helpers';

test.skip(!process.env.JF_SCREENSHOTS, 'screenshots only with JF_SCREENSHOTS=1');
test.describe.configure({ mode: 'serial' });

const OUT = fileURLToPath(new URL('../../../../docs/screenshots/web/', import.meta.url));
const DESKTOP = { width: 1280, height: 800 };
/** Tall enough for the whole Appearance section of Settings. */
const DESKTOP_TALL = { width: 1280, height: 1380 };
const PHONE = { width: 390, height: 844 };

let page: Page;

async function setLook(appearance: 'light' | 'dark', style: 'artwork' | 'misty', language: string | null = null) {
  await page.evaluate(
    ({ appearance, style, language }) =>
      (globalThis as any).jellyfesh.settings.update((d: any) => {
        d.appearance = appearance;
        d.style = style;
        d.language = language;
      }),
    { appearance, style, language },
  );
  await page.waitForTimeout(700);
}

/** Plays Morning Harbour and stops on the second lyric line, two words sung. */
async function holdOnLyrics() {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await page.evaluate(() => {
    const player = (globalThis as any).jellyfesh.player;
    player.seek(4600);
  });
  await page.waitForTimeout(300);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.pause());
  await page.waitForTimeout(500);
}

/** Takes a screenshot, with the pointer parked where it hovers nothing unless `hovering`. */
async function shot(name: string, hovering = false) {
  if (!hovering) {
    const size = page.viewportSize()!;
    await page.mouse.move(size.width - 1, 1);
  }
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${OUT}${name}-mock-server.png` });
}

test.beforeAll(async ({ browser }: { browser: Browser }) => {
  page = await browser.newPage({ viewport: DESKTOP, deviceScaleFactor: 1 });
  await openApp(page);
  await signInSubsonic(page);
  await signInJellyfinQuickConnect(page);
});

test.afterAll(async () => {
  await page.close();
});

test('library views', async () => {
  await setLook('light', 'artwork');
  await nav(page, 'Home').click();
  await expect(page.getByRole('region', { name: 'Jellyfin' }).getByRole('button', { name: 'Open Albums' })).toBeVisible();
  await shot('home-light-desktop');
  await openAlbum(page, 'Jellyfin', 'Slow Tide');
  // A track number turns into a play glyph under the pointer.
  await page.getByRole('button', { name: 'Play Rope and Mast' }).hover();
  await shot('album-light-desktop', true);
  await setLook('dark', 'artwork');
  await shot('album-dark-desktop');
  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  // The same song on both servers shows once, where it answered first, "also in" the other.
  await expect(page.getByRole('button', { name: 'Play Lanterns' })).toHaveCount(1);
  await expect(page.getByText(/Also in (Subsonic|Jellyfin)/).first()).toBeVisible();
  await shot('search-dark-desktop');
  await setLook('light', 'artwork');
  await nav(page, 'Sources').click();
  await shot('sources-light-desktop');
});

test('the player bar', async () => {
  await holdOnLyrics();
  await expect(page.getByRole('region', { name: 'Now Playing' })).toBeVisible();
  for (const appearance of ['light', 'dark'] as const) {
    await setLook(appearance, 'artwork');
    await shot(`player-bar-${appearance}-desktop`);
  }
  await setLook('light', 'artwork');
});

test('Settings: Appearance', async () => {
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Appearance' }).click();
  await expect(page.getByRole('heading', { name: 'Theme' })).toBeVisible();
  // Show the swatches: a fixed accent, the default hue's swatch.
  await page.evaluate(() =>
    (globalThis as any).jellyfesh.settings.update((d: any) => (d.hue_source = { kind: 'fixed', hue: 250 })),
  );
  await page.setViewportSize(DESKTOP_TALL);
  for (const appearance of ['light', 'dark'] as const) {
    await setLook(appearance, 'artwork');
    await shot(`preferences-appearance-${appearance}-desktop`);
  }
  await page.setViewportSize(DESKTOP);
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.hue_source = { kind: 'artwork' })));
  await setLook('light', 'artwork');
});

test('Now Playing on a desktop', async () => {
  await holdOnLyrics();
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await expect(page.locator('.line.active')).toBeVisible();
  for (const style of ['artwork', 'misty'] as const) {
    for (const appearance of ['light', 'dark'] as const) {
      await setLook(appearance, style);
      await shot(`now-playing-${style}-${appearance}-desktop`);
    }
  }
  await setLook('dark', 'artwork', 'zh-Hans');
  await page.getByRole('button', { name: '播放队列' }).click();
  await shot('now-playing-queue-zh-hans-dark-desktop');
  await setLook('light', 'artwork', null);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
});

test('Now Playing on a phone', async () => {
  await page.setViewportSize(PHONE);
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  for (const [style, appearance] of [
    ['artwork', 'light'],
    ['artwork', 'dark'],
    ['misty', 'light'],
    ['misty', 'dark'],
  ] as const) {
    await setLook(appearance, style);
    await shot(`now-playing-${style}-${appearance}-phone`);
  }
  await setLook('dark', 'artwork');
  await page.getByRole('button', { name: 'Lyrics' }).click();
  await shot('now-playing-lyrics-artwork-dark-phone');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
  await setLook('light', 'misty');
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await shot('album-light-phone');
  await page.setViewportSize(DESKTOP);
});
