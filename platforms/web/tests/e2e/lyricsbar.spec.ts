// The lyrics bar and Desktop Lyrics (ADR-0022 §6, docs/ui.md "Lyrics, word by word"): with Show
// Lyrics Bar on (the default) and word-timed lyrics, the player bar shows the line being sung under
// the title, its words filling as they are sung, with its translation; Desktop Lyrics shows the line
// and the next one in a Document Picture-in-Picture window, and closing that window turns the
// setting off. Morning Harbour's lyrics (tests/mock/library.mjs) start lines at 0.5, 3.4 and 6.7 s.

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

const LINES = /Morning folds the harbour into light|Every rope and mast remembers night|Slow tide, carry me/;

async function playMorningHarbour() {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
}

async function openAppearance() {
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Appearance' }).click();
}

function sweeping(target: Page): Promise<boolean> {
  return target.evaluate(() =>
    document.getAnimations().some((a) => (a as CSSAnimation).animationName?.startsWith('jf-sweep-') && a.playState === 'running'),
  );
}

test('shows the line being sung in the player bar, word by word, with its translation', async () => {
  await playMorningHarbour();
  const bar = page.getByRole('region', { name: 'Now Playing' });
  const sung = bar.locator('.lyric-line');
  await expect(sung).toContainText(LINES);
  await expect(bar.getByText(/清晨把港湾折进光里|每根绳索与桅杆都还记得夜|慢潮啊，带我走/)).toBeVisible();
  // Split into words; the word being sung sweeps as a CSS animation the browser runs.
  await expect(sung.locator('.word').first()).toBeVisible();
  await expect.poll(() => sweeping(page), { timeout: 10_000 }).toBe(true);
  await expect(sung).toContainText('Slow tide, carry me', { timeout: 10_000 });
});

test('the Lyrics pane sweeps its words too', async () => {
  await playMorningHarbour();
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await expect(page.getByRole('region', { name: 'Lyrics' }).locator('.line.active .word').first()).toBeVisible();
  await expect.poll(() => sweeping(page), { timeout: 10_000 }).toBe(true);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
});

test('Show Lyrics Bar off shows the artist again', async () => {
  await openAppearance();
  const setting = page.getByRole('switch', { name: 'Show Lyrics Bar' });
  await expect(setting).toBeChecked();
  await setting.uncheck();
  await playMorningHarbour();
  const bar = page.getByRole('region', { name: 'Now Playing' });
  await expect(bar.locator('.second')).toBeVisible();
  await expect(bar.locator('.lyric-line')).toHaveCount(0);
  await openAppearance();
  await page.getByRole('switch', { name: 'Show Lyrics Bar' }).check();
});

test('opens Desktop Lyrics in a Picture-in-Picture window; closing it turns the setting off', async () => {
  await playMorningHarbour();
  await openAppearance();
  const setting = page.getByRole('switch', { name: 'Desktop Lyrics' });
  await expect(setting).not.toBeChecked();
  const opened = page.context().waitForEvent('page');
  await setting.check();
  const pip = await opened;
  await expect(pip.locator('.line-now')).toContainText(LINES);
  // The next line under it, until the last line.
  await expect(pip.locator('.line-next')).toContainText(/Every rope and mast remembers night|Slow tide, carry me/);
  // The page's styles and theme came along.
  expect(await pip.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--jf-ink-90').trim())).not.toBe('');
  await expect.poll(() => sweeping(pip), { timeout: 10_000 }).toBe(true);
  // Now Playing's More has it as a check item.
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await page.getByRole('dialog', { name: 'Now Playing' }).getByRole('button', { name: 'More' }).click();
  await expect(page.getByRole('menuitemcheckbox', { name: 'Desktop Lyrics' })).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');
  // Its own Close button turns the setting off, which closes it.
  const closed = pip.waitForEvent('close');
  await pip.getByRole('button', { name: 'Close Desktop Lyrics' }).click();
  await closed;
  await openAppearance();
  await expect(page.getByRole('switch', { name: 'Desktop Lyrics' })).not.toBeChecked();
  expect(await page.evaluate(() => (globalThis as any).jellyfesh.settings.value.desktop_lyrics)).toBe(false);
});

test('opens Desktop Lyrics again when the page opens with it on, at once or at a click', async () => {
  // Saved with the setting on (not through the page, which could open the window at once while a
  // click a moment ago still lets it).
  await page.evaluate(() => {
    const saved = JSON.parse(localStorage.getItem('jf.settings') ?? '{}');
    localStorage.setItem('jf.settings', JSON.stringify({ ...saved, desktop_lyrics: true }));
  });
  // Chromium may still let the page open a window right after a reload; otherwise the player bar
  // offers Show Desktop Lyrics.
  const atLoad = page.context().waitForEvent('page', { timeout: 5_000 }).catch(() => null);
  await page.reload();
  await expect(page.getByRole('navigation', { name: 'Library sections' }).first()).toBeVisible();
  let pip = await atLoad;
  if (!pip) {
    await playMorningHarbour();
    const show = page.getByRole('region', { name: 'Now Playing' }).getByRole('button', { name: 'Show Desktop Lyrics' });
    await expect(show).toBeVisible();
    const opened = page.context().waitForEvent('page');
    await show.click();
    pip = await opened;
    await expect(show).toBeHidden();
  }
  await expect(pip.locator('.line-now, .fallback').first()).toBeVisible();
  // Closed by the browser: the setting follows.
  await pip.close();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.desktop_lyrics)).toBe(false);
});
