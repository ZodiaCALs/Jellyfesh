// The reference screenshots in docs/screenshots/web/ (ADR-0023 §5: every reference page, light and
// dark, desktop and phone), taken from the app running against the mock servers and a generated
// music folder (invented names, generated tones and gradient covers): every file name ends in
// -mock-server.png, and the website captions the ones it shows as sample music
// (tools/release/build_site.py). Only with JF_SCREENSHOTS=1:
//
//   JF_SCREENSHOTS=1 npx playwright test screenshots --trace off

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, expect, type Page, test } from '@playwright/test';
import { png, taggedWav } from '../mock/media.mjs';
import { nav, openAlbum, openApp, signInJellyfinQuickConnect, signInSubsonic, waitPlaying } from './helpers';

test.skip(!process.env.JF_SCREENSHOTS, 'screenshots only with JF_SCREENSHOTS=1');
test.describe.configure({ mode: 'serial' });

const OUT = fileURLToPath(new URL('../../../../docs/screenshots/web/', import.meta.url));
const DESKTOP = { width: 1280, height: 800 };
/** Tall enough for the whole Appearance section of Settings. */
const DESKTOP_TALL = { width: 1280, height: 1380 };
const PHONE = { width: 390, height: 844 };
const FORMS = { desktop: DESKTOP, phone: PHONE } as const;
type Form = keyof typeof FORMS;
const APPEARANCES = ['light', 'dark'] as const;

const SUBSONIC_ID = 'org.jellyfesh.subsonic';

/** The generated music folder: two albums with covers and a song of its own (invented names). */
const FOLDER_SONGS = [
  { path: 'Quiet Reef/01 Driftwood Radio.wav', title: 'Driftwood Radio', artist: 'Paper Moth', album: 'Quiet Reef', track: 1, hz: 392, cover: [[64, 140, 150], [30, 50, 110]] },
  { path: 'Quiet Reef/02 Salt Lamp.wav', title: 'Salt Lamp', artist: 'Paper Moth', album: 'Quiet Reef', track: 2, hz: 440, cover: [[64, 140, 150], [30, 50, 110]] },
  { path: 'Quiet Reef/03 Low Clouds.wav', title: 'Low Clouds', artist: 'Paper Moth', album: 'Quiet Reef', track: 3, hz: 494, cover: [[64, 140, 150], [30, 50, 110]] },
  { path: 'Neon Fjord/01 Night Kiosk.wav', title: 'Night Kiosk', artist: 'Lumen Drift', album: 'Neon Fjord', track: 1, hz: 523, cover: [[200, 90, 150], [60, 40, 120]] },
  { path: 'Neon Fjord/02 Glass Orchard.wav', title: 'Glass Orchard', artist: 'Lumen Drift', album: 'Neon Fjord', track: 2, hz: 587, cover: [[200, 90, 150], [60, 40, 120]] },
  { path: 'Singles/Morning Static.wav', title: 'Morning Static', artist: 'Ora Vale', album: null, track: null, hz: 659, cover: [[230, 170, 90], [150, 60, 70]] },
];

let page: Page;
let root: string;

async function setLook(appearance: 'light' | 'dark', style: 'artwork' | 'misty' = 'artwork', language: string | null = null) {
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

/** Goes to a place (Home, Albums, … Settings) the way either layout's navigation does. */
async function go(tab: string) {
  await page.evaluate((tab) => (globalThis as any).jellyfesh.nav.switchTab(tab), tab);
  await page.waitForTimeout(300);
}

/** Plays Morning Harbour and stops on the second lyric line, two words sung. */
async function holdOnLyrics() {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(4600));
  await page.waitForTimeout(300);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.pause());
  await page.waitForTimeout(500);
}

/** Closes an open popover menu (Escape would also go back a page). */
async function closeMenu() {
  await page.evaluate(() => document.querySelector<HTMLElement>('[popover]:popover-open')?.hidePopover());
  await page.waitForTimeout(200);
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

/** A page in both appearances on both forms: `<stem>-<light|dark>-<desktop|phone>`. */
async function everyLook(stem: string, open: (form: Form) => Promise<void>, forms: Form[] = ['desktop', 'phone']) {
  for (const form of forms) {
    await page.setViewportSize(FORMS[form]);
    await open(form);
    for (const appearance of APPEARANCES) {
      await setLook(appearance);
      await shot(`${stem}-${appearance}-${form}`);
    }
  }
  await page.setViewportSize(DESKTOP);
  await setLook('light');
}

test.beforeAll(async ({ browser }: { browser: Browser }) => {
  root = mkdtempSync(join(tmpdir(), 'jf-shots-'));
  for (const song of FOLDER_SONGS) {
    const file = join(root, 'Music', song.path);
    mkdirSync(join(file, '..'), { recursive: true });
    const cover = png(600, song.cover[0], song.cover[1]);
    writeFileSync(
      file,
      taggedWav(4, song.hz, { title: song.title, artist: song.artist, album: song.album ?? undefined, track: song.track ?? undefined, year: 2026, cover }),
    );
  }
  page = await browser.newPage({ viewport: DESKTOP, deviceScaleFactor: 1 });
  // The folder picker of browsers that keep no folders (Playwright cannot drive the native one).
  await page.addInitScript(() => {
    delete (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker;
  });
  await openApp(page);
  await signInSubsonic(page);
  await signInJellyfinQuickConnect(page);
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add Folder…' }).click();
  await (await chooser).setFiles(join(root, 'Music'));
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks), { timeout: 30_000 }).toBe(FOLDER_SONGS.length);

  // Hearts, a Jellyfesh playlist and plays, as someone who has used Jellyfesh a while has them.
  await page.evaluate(async (subsonic) => {
    const app = (globalThis as any).jellyfesh;
    const library = app.library.songs('title', false).map((item: any) => ({ plugin: 'library', item }));
    const byTitle = (title: string) => library.find((s: any) => s.item.title === title);
    const server = new Map<string, any>();
    for (const title of ['Night Ferry', 'Lanterns', 'Rope and Mast', 'Still Water', 'Morning Harbour']) {
      const item = (await app.sources.get(subsonic).search(title, 5, 8000, ['Track'])).items.find((i: any) => i.title === title);
      if (item) server.set(title, { plugin: subsonic, item });
    }
    const fromServer = (title: string) => server.get(title);
    const hearted = [byTitle('Salt Lamp'), fromServer('Night Ferry'), byTitle('Glass Orchard'), fromServer('Lanterns')];
    for (const song of hearted) if (song) app.collections.setFavorite(song, true);
    const id = app.collections.createPlaylist('Evening Drive');
    app.collections.addToPlaylist(
      id,
      [byTitle('Driftwood Radio'), fromServer('Rope and Mast'), byTitle('Night Kiosk'), fromServer('Still Water'), byTitle('Morning Static')].filter(Boolean),
    );
    app.collections.createPlaylist('Rainy Mornings');
    let at = Date.now() - 3 * 86_400_000;
    const plays: [any, number][] = [
      [byTitle('Low Clouds'), 3],
      [fromServer('Morning Harbour'), 5],
      [byTitle('Night Kiosk'), 2],
      [fromServer('Night Ferry'), 1],
      [byTitle('Salt Lamp'), 4],
    ];
    for (const [song, times] of plays) {
      for (let n = 0; n < times && song; n++) {
        at += 3_600_000;
        app.collections.recordPlay(song, at, true);
      }
    }
  }, SUBSONIC_ID);
});

test.afterAll(async () => {
  await page.close();
  rmSync(root, { recursive: true, force: true });
});

test('the library places', async () => {
  await everyLook('home', async () => {
    await go('home');
    for (const source of ['Subsonic', 'Jellyfin']) {
      await expect(page.getByRole('region', { name: source }).getByRole('button', { name: 'Open Albums' })).toBeVisible();
    }
    await expect(page.getByRole('region', { name: 'Recently Played' })).toBeVisible();
    await expect(page.getByRole('status')).toHaveCount(0);
  });
  await everyLook('albums', async () => {
    await go('albums');
    await expect(page.getByRole('button', { name: 'Open Quiet Reef' })).toBeVisible();
  });
  await everyLook('artists', async () => {
    await go('artists');
    await expect(page.getByRole('button', { name: 'Open Paper Moth' })).toBeVisible();
  });
  await everyLook('songs', async () => {
    await go('songs');
    await expect(page.getByRole('button', { name: 'Play Driftwood Radio' })).toBeVisible();
  });
  await everyLook('playlists', async () => {
    await go('playlists');
    await expect(page.getByRole('button', { name: 'Open Evening Drive' })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Subsonic' }).getByRole('button', { name: 'Open Evening Walk' })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Jellyfin' }).getByRole('button', { name: 'Open Evening Walk' })).toBeVisible();
  });
  await everyLook('playlist', async () => {
    await go('playlists');
    await page.getByRole('button', { name: 'Open Evening Drive' }).click();
    await expect(page.getByRole('list', { name: 'Evening Drive' }).getByRole('listitem')).toHaveCount(5);
  });
  await everyLook('favourites', async () => {
    await go('favourites');
    await expect(page.getByRole('list', { name: 'Favourites' }).getByRole('listitem')).toHaveCount(4);
    await expect(page.getByRole('region', { name: 'Subsonic' }).getByRole('group', { name: 'Songs' })).toBeVisible();
  });
  await everyLook('search', async () => {
    await go('search');
    await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
    // The same song on both servers shows once, where it answered first, "also in" the other.
    await expect(page.getByText(/Also in (Subsonic|Jellyfin)/).first()).toBeVisible();
    await expect(page.getByRole('tab', { name: 'Jellyfin' })).toBeVisible();
    await page.getByRole('searchbox', { name: 'Search' }).blur();
  });
  await everyLook('search-source', async () => {
    await go('search');
    await page.getByRole('tab', { name: 'Subsonic' }).click();
    await expect(page.getByRole('tabpanel').getByRole('region', { name: 'Songs' })).toBeVisible();
  });
  await everyLook('sources', async () => {
    await go('sources');
    await expect(page.getByRole('heading', { name: 'Sources', level: 1 })).toBeVisible();
  });
});

test('an album', async () => {
  await everyLook('album', async (form) => {
    await openAlbum(page, 'Jellyfin', 'Slow Tide');
    if (form === 'desktop') await page.getByRole('button', { name: 'Play Rope and Mast' }).hover();
  });
  // A track number turns into a play glyph under the pointer.
  await openAlbum(page, 'Jellyfin', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Rope and Mast' }).hover();
  await shot('album-light-desktop', true);
});

test('Settings', async () => {
  // Show the swatches: a fixed accent, the default hue's swatch.
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.hue_source = { kind: 'fixed', hue: 250 })));
  for (const form of ['desktop', 'phone'] as const) {
    await page.setViewportSize(form === 'desktop' ? DESKTOP_TALL : PHONE);
    await go('settings');
    await page.getByRole('tab', { name: 'Appearance' }).click();
    await expect(page.getByRole('heading', { name: 'Theme' })).toBeVisible();
    for (const appearance of APPEARANCES) {
      await setLook(appearance);
      await shot(`preferences-appearance-${appearance}-${form}`);
    }
    await page.getByRole('tab', { name: 'Playback' }).click();
    await expect(page.getByRole('heading', { name: 'Streaming' })).toBeVisible();
    for (const appearance of APPEARANCES) {
      await setLook(appearance);
      await shot(`preferences-playback-${appearance}-${form}`);
    }
  }
  await page.setViewportSize(DESKTOP);
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.hue_source = { kind: 'artwork' })));
  await setLook('light');
});

test('the player bar and its quality menu', async () => {
  await holdOnLyrics();
  await expect(page.getByRole('region', { name: 'Now Playing' })).toBeVisible();
  for (const appearance of APPEARANCES) {
    await setLook(appearance);
    await shot(`player-bar-${appearance}-desktop`);
  }
  for (const appearance of APPEARANCES) {
    await setLook(appearance);
    await page.getByRole('button', { name: /^Quality: / }).first().click();
    await expect(page.getByRole('menu', { name: 'Streaming Quality' })).toBeVisible();
    await shot(`quality-menu-${appearance}-desktop`, true);
    await closeMenu();
  }
  await setLook('light');
});

test('Now Playing on a desktop', async () => {
  await holdOnLyrics();
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await expect(page.locator('.line.active')).toBeVisible();
  for (const style of ['artwork', 'misty'] as const) {
    for (const appearance of APPEARANCES) {
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
  // The quality menu, from Now Playing (the capsule has no badge).
  for (const appearance of APPEARANCES) {
    await setLook(appearance);
    await page.getByRole('button', { name: /^Quality: / }).first().click();
    await expect(page.getByRole('menu', { name: 'Streaming Quality' })).toBeVisible();
    await shot(`quality-menu-${appearance}-phone`, true);
    await closeMenu();
  }
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
  await page.setViewportSize(DESKTOP);
  await setLook('light');
});
