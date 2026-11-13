// Shared steps for the end-to-end tests. The servers are the mocks in tests/mock (invented data).

import { expect, type Page } from '@playwright/test';

export const SUBSONIC = 'http://127.0.0.1:4610';
export const JELLYFIN = 'http://127.0.0.1:4620';
export const SUBSONIC_NO_CORS = 'http://127.0.0.1:4630';
export const USER = 'demo';
export const PASSWORD = 'invented-password';

/** What the app's state says, through the ?e2e handle. */
export interface Snapshot {
  title: string | null;
  playing: boolean;
  positionMs: number;
  lyrics: string;
  problem: string | null;
  queueLength: number;
}

export async function openApp(page: Page) {
  page.on('pageerror', (error) => console.log(`[pageerror] ${error.message}`));
  await page.goto('./?e2e');
  await expect(page.getByRole('heading', { name: 'Home', level: 1 })).toBeVisible();
}

export async function snapshot(page: Page): Promise<Snapshot> {
  return page.evaluate(() => {
    const app = (globalThis as any).jellyfesh;
    const player = app.player;
    return {
      title: player.current?.title ?? null,
      playing: player.playing,
      // 0 until the current entry's own stream plays (not the one before it, still sounding).
      positionMs: player.positionMs,
      lyrics: player.lyrics.status,
      problem: player.problem,
      queueLength: player.queue.length,
    };
  });
}

/** Waits until the given track plays and its clock has moved past `pastMs`. */
export async function waitPlaying(page: Page, title: string, pastMs = 200) {
  try {
    await expect
      .poll(async () => {
        const s = await snapshot(page);
        return s.title === title && s.playing && s.positionMs > pastMs;
      }, { timeout: 30_000 })
      .toBe(true);
  } catch (error) {
    const trace = await page.evaluate(() => (globalThis as any).jellyfesh.player.trace.join('\n'));
    console.log(`player trace while waiting for ${title}:\n${trace}\nstate: ${JSON.stringify(await snapshot(page))}`);
    throw error;
  }
}

export function nav(page: Page, name: string) {
  return page.getByRole('navigation', { name: 'Library sections' }).first().getByRole('button', { name, exact: true });
}

/** Opens the sign-in sheet of a source, accepting the community disclosure when the build's packages are unsigned. */
async function openSignIn(page: Page, source: string) {
  await nav(page, 'Sources').click();
  const card = page.locator('li.source', { has: page.getByRole('heading', { name: source }) });
  await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  const allow = page.getByRole('button', { name: 'I Understand, Allow' });
  const sheet = page.getByRole('dialog', { name: `Sign In to ${source}` });
  await expect(allow.or(sheet)).toBeVisible();
  if (await allow.isVisible()) {
    await allow.click();
    await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  }
  await expect(sheet).toBeVisible();
  return sheet;
}

export async function signInSubsonic(page: Page, server = SUBSONIC) {
  const sheet = await openSignIn(page, 'Subsonic');
  await sheet.getByLabel('Server address').fill(server);
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await sheet.getByRole('button', { name: 'Sign In', exact: true }).click();
  await sheet.getByLabel('User name').fill(USER);
  await sheet.getByLabel('Password').fill(PASSWORD);
  await sheet.getByRole('button', { name: 'Sign In', exact: true }).click();
  await expect(sheet.getByText(`Signed in as ${USER}.`)).toBeVisible();
  await sheet.getByRole('button', { name: 'Done' }).click();
}

/** Quick Connect: the app shows a code, the "person" approves it on the mock server. */
export async function signInJellyfinQuickConnect(page: Page) {
  const sheet = await openSignIn(page, 'Jellyfin');
  await sheet.getByLabel('Server address').fill(JELLYFIN);
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await sheet.getByRole('button', { name: 'Sign In with QR Code' }).click();
  const code = sheet.locator('.code');
  // The plugin shows the code in two groups ("482 920") for reading aloud.
  await expect(code).toHaveText(/^\d{3} ?\d{3}$/);
  const text = (await code.textContent())!.replace(/\s/g, '');
  const approved = await page.request.post(`${JELLYFIN}/__mock/approve`, { data: { Code: text } });
  expect(approved.status()).toBe(204);
  await expect(sheet.getByText(`Signed in as ${USER}.`)).toBeVisible({ timeout: 30_000 });
  await sheet.getByRole('button', { name: 'Done' }).click();
}

/** Home → a source's albums → an album. */
export async function openAlbum(page: Page, source: string, album: string) {
  await nav(page, 'Home').click();
  const shelf = page.getByRole('region', { name: source });
  if (source === 'Subsonic') {
    await shelf.getByRole('button', { name: 'Open Albums' }).click();
    await page.getByRole('button', { name: 'Open Recently added' }).click();
  } else {
    await shelf.getByRole('button', { name: 'Open Albums' }).click();
  }
  await page.getByRole('button', { name: `Open ${album}` }).click();
  await expect(page.getByRole('heading', { name: album, level: 1 })).toBeVisible();
}

/** Sets a preference through the core (as the Preferences page does). */
export async function setCrossfade(page: Page, enabled: boolean, seconds = 2, albumContinuity = false) {
  await page.evaluate(
    ({ enabled, seconds, albumContinuity }) => {
      (globalThis as any).jellyfesh.settings.update((d: any) => {
        d.crossfade = { enabled, duration_ms: seconds * 1000, album_continuity: albumContinuity };
      });
    },
    { enabled, seconds, albumContinuity },
  );
}

export async function transitions(page: Page): Promise<{ from: number; to: number; fadeMs: number; leadMs: number }[]> {
  return page.evaluate(() => (globalThis as any).jellyfesh.player.engine.transitions.map((t: any) => ({ ...t })));
}
