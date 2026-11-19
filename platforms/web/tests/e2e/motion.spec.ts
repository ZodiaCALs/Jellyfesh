// Motion (docs/web.md, "Motion"): the aurora drifts only while music plays, the first rows and
// covers of a list rise in one after another, a cover grows into its page where the browser has
// view transitions, and Reduce Motion and Transparency stops all of it. The ?e2e page keeps the
// aurora still (so other tests do not depend on time); these tests let it move.

import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInSubsonic, waitPlaying } from './helpers';

/** The aurora layer's drift (body::before), as the browser runs it, or null when it has none. */
function aurora(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const drift = document
      .getAnimations()
      .find((a) => (a.effect as KeyframeEffect | null)?.pseudoElement === '::before' && (a.effect as KeyframeEffect).target === document.body);
    return drift?.playState ?? null;
  });
}

async function letItMove(page: Page) {
  await page.evaluate(() => delete document.documentElement.dataset.ambient);
}

async function reduce(page: Page, on: boolean) {
  await page.evaluate((on) => (globalThis as any).jellyfesh.settings.update((d: any) => (d.reduce_effects = on)), on);
}

test('the aurora drifts while music plays, and rests when it stops', async ({ page }) => {
  await openApp(page);
  await letItMove(page);
  await signInSubsonic(page);
  expect(await aurora(page)).toBe('paused');
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await expect.poll(() => aurora(page)).toBe('running');
  await page.evaluate(() => (globalThis as any).jellyfesh.player.pause());
  await expect.poll(() => aurora(page)).toBe('paused');
  // Reduce Motion and Transparency: no drift at all.
  await reduce(page, true);
  await expect.poll(() => aurora(page)).toBeNull();
});

test('lists rise in, covers grow into their page, and Reduce Motion stops both', async ({ page }) => {
  await openApp(page);
  await signInSubsonic(page);
  await nav(page, 'Home').click();
  await page.getByRole('region', { name: 'Subsonic' }).getByRole('button', { name: 'Open Albums' }).click();
  await page.getByRole('button', { name: 'Open Recently added' }).click();
  await expect(page.getByRole('button', { name: 'Open Slow Tide' })).toBeVisible();
  // The first cards rise in one after another: each a little later than the one before.
  const delays = await page.locator('.card.enter').evaluateAll((cards) => cards.map((c) => getComputedStyle(c).animationDelay));
  expect(delays).toEqual(['0s', '0.03s']);

  // Opening an album from its cover is one view transition, the cover morphing into the page's.
  await page.evaluate(() => {
    const doc = document as Document & { startViewTransition?: (update: () => unknown) => unknown };
    const start = doc.startViewTransition?.bind(document);
    (globalThis as any).transitions = [];
    if (start) {
      doc.startViewTransition = (update: () => unknown) => {
        (globalThis as any).transitions.push(document.documentElement.dataset.viewTransition ?? '');
        return start(update);
      };
    }
  });
  await page.getByRole('button', { name: 'Open Slow Tide' }).click();
  await expect(page.getByRole('heading', { name: 'Slow Tide', level: 1 })).toBeVisible();
  expect(await page.evaluate(() => (globalThis as any).transitions)).toEqual(['open-item']);
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset.viewTransition ?? null)).toBeNull();

  // Reduce Motion and Transparency: rows appear at once, and pages open without a morph.
  await reduce(page, true);
  await page.goBack();
  await expect(page.getByRole('button', { name: 'Open Slow Tide' })).toBeVisible();
  expect(await page.locator('.card.enter').first().evaluate((c) => getComputedStyle(c).animationName)).toBe('none');
  await page.getByRole('button', { name: 'Open Slow Tide' }).click();
  await expect(page.getByRole('heading', { name: 'Slow Tide', level: 1 })).toBeVisible();
  expect(await page.evaluate(() => (globalThis as any).transitions)).toEqual(['open-item']);
  expect(await page.locator('.row.enter').first().evaluate((r) => getComputedStyle(r).animationName)).toBe('none');
});
