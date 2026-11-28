// Rows and cards (docs/ui.md, v0.0.9), against the mock Subsonic server: right-click opens a row's
// or a card's menu at the pointer, the same menu as its More button, which the keyboard moves
// through and Esc closes; a song row shows a play button over its artwork and More while the
// pointer is over it or it has keyboard focus, and the play button plays from that row; Up Next
// offers a source's song its album and artist (ADR-0027 §2), also after a reload.

import { expect, type Locator, type Page, test } from '@playwright/test';
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

/** The menu that is open (closed menus are hidden). */
function openMenu(): Locator {
  return page.getByRole('menu');
}

/** The labels of the open menu's items. */
async function labels(): Promise<string[]> {
  return openMenu().getByRole('menuitem').allTextContents();
}

/** The items of the menu its More button opens, then closes it. */
async function moreLabels(more: Locator): Promise<string[]> {
  await more.click();
  await expect(openMenu()).toBeVisible();
  const items = await labels();
  await page.keyboard.press('Escape');
  await expect(openMenu()).toBeHidden();
  return items;
}

test('right-click on a song row opens its More menu at the pointer; the keyboard moves through it and Esc closes it', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  const row = page.getByRole('button', { name: 'Play Lanterns' });
  const more = await moreLabels(page.getByRole('button', { name: 'More for Lanterns' }));
  expect(more).toEqual(expect.arrayContaining(['Play Next', 'Play Last', 'Add to Playlist…', 'Go to Album', 'Go to Artist']));

  const box = (await row.boundingBox())!;
  await row.click({ button: 'right', position: { x: 160, y: box.height / 2 } });
  await expect(openMenu()).toBeVisible();
  expect(await labels()).toEqual(more);
  if (process.env.JF_SHOTS) await page.screenshot({ path: `${process.env.JF_SHOTS}/row-right-click.png`, animations: 'disabled' });
  // At the pointer: below it where it fits, else above it.
  const placed = (await openMenu().boundingBox())!;
  const pointer = box.y + box.height / 2;
  expect(Math.abs(placed.x - (box.x + 160))).toBeLessThan(2);
  expect(Math.min(Math.abs(placed.y - pointer), Math.abs(placed.y + placed.height - pointer))).toBeLessThan(2);
  // The keyboard: the first item has the focus, ↓ moves, Esc closes and gives the focus back.
  const items = openMenu().getByRole('menuitem');
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(items.nth(1)).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(openMenu()).toBeHidden();
  await expect(row).toBeFocused();

  // A click elsewhere (beside the menu, on the page's title line) closes it.
  await row.click({ button: 'right' });
  await expect(openMenu()).toBeVisible();
  const menuBox = (await openMenu().boundingBox())!;
  const heading = (await page.getByRole('heading', { name: 'Slow Tide', level: 1 }).boundingBox())!;
  await page.mouse.click(menuBox.x + menuBox.width + 40, heading.y + heading.height / 2);
  await expect(openMenu()).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Slow Tide', level: 1 })).toBeVisible();

  // Its items do what More's do.
  await row.click({ button: 'right' });
  await openMenu().getByRole('menuitem', { name: 'Play Next' }).click();
  await expect(page.getByText('Plays next')).toBeVisible();
  await expect(openMenu()).toBeHidden();
});

test('the keyboard’s menu key opens a row’s menu under it', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  const row = page.getByRole('button', { name: 'Play Still Water' });
  await row.focus();
  await page.keyboard.press('Shift+F10');
  await expect(openMenu()).toBeVisible();
  await expect(openMenu().getByRole('menuitem').first()).toBeFocused();
  const box = (await row.boundingBox())!;
  const placed = (await openMenu().boundingBox())!;
  // Under the row where it fits, else over it; never on it.
  expect(placed.y >= box.y + box.height || placed.y + placed.height <= box.y).toBe(true);
  await page.keyboard.press('Escape');
  await expect(row).toBeFocused();
});

test('a long press on a touch screen opens a row’s menu by the finger, and the press plays nothing', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  const row = page.getByRole('button', { name: 'Play Still Water' });
  const box = (await row.boundingBox())!;
  const x = Math.round(box.x + 160);
  const y = Math.round(box.y + box.height / 2);
  // Touches reach the row once the page's opening transition is over.
  await expect.poll(() => page.evaluate(({ x, y }) => !!document.elementFromPoint(x, y)?.closest('.row'), { x, y })).toBe(true);
  // Counts what the row's own action would start.
  await page.evaluate(() => {
    const player = (globalThis as any).jellyfesh.player;
    const play = player.playEntries.bind(player);
    player.starts = 0;
    player.playEntries = (...args: unknown[]) => {
      player.starts += 1;
      return play(...args);
    };
  });
  const touch = await page.context().newCDPSession(page);
  await touch.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
  await page.waitForTimeout(800);
  await touch.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await expect(openMenu()).toBeVisible();
  expect(await labels()).toEqual(expect.arrayContaining(['Play Next', 'Play Last', 'Go to Album', 'Go to Artist']));
  // Beside the finger, a little away from it (below or above), not under it.
  const placed = (await openMenu().boundingBox())!;
  expect(Math.abs(placed.x - x)).toBeLessThan(2);
  expect(placed.y > y + 4 || placed.y + placed.height < y - 4).toBe(true);
  expect(await page.evaluate(() => (globalThis as any).jellyfesh.player.starts)).toBe(0);
  await page.keyboard.press('Escape');
  await expect(openMenu()).toBeHidden();
  // A short tap still plays the row.
  await touch.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await waitPlaying(page, 'Still Water');
  expect(await page.evaluate(() => (globalThis as any).jellyfesh.player.starts)).toBe(1);
  await touch.detach();
});

test('right-click on a card opens the card’s menu: Play, Shuffle, then its More entries', async () => {
  await nav(page, 'Home').click();
  await page.getByRole('region', { name: 'Subsonic' }).getByRole('button', { name: 'Open Albums' }).click();
  await page.getByRole('button', { name: 'Open Recently added' }).click();
  const card = page.getByRole('button', { name: 'Open Paper Lanterns' });
  await expect(card).toBeVisible();
  const more = await moreLabels(page.getByRole('button', { name: 'More for Paper Lanterns' }));
  expect(more.slice(0, 4)).toEqual(['Play', 'Shuffle', 'Play Next', 'Play Last']);
  expect(more).toContain('Go to Artist');

  await card.click({ button: 'right' });
  await expect(openMenu()).toBeVisible();
  expect(await labels()).toEqual(more);
  if (process.env.JF_SHOTS) await page.screenshot({ path: `${process.env.JF_SHOTS}/card-right-click.png`, animations: 'disabled' });
  await page.keyboard.press('Escape');
  await expect(card).toBeFocused();

  // Keyboard focus on the card shows its play button and More, as the pointer over it does.
  await page.mouse.move(1, 1);
  const shell = page.locator('.card', { has: card });
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(card).toBeFocused();
  await expect(shell.locator('.play')).toHaveCSS('opacity', '1');
  await expect(shell.locator('div.more')).toHaveCSS('opacity', '1');

  // Play plays the album from its first song.
  await card.click({ button: 'right' });
  await openMenu().getByRole('menuitem', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Night Ferry');
  // A plain click still opens the album.
  await card.click();
  await expect(page.getByRole('heading', { name: 'Paper Lanterns', level: 1 })).toBeVisible();
});

test('a song row shows a play button over its artwork and More on hover or keyboard focus; the play button plays from that row', async () => {
  await nav(page, 'Favourites').click();
  const songs = page.getByRole('region', { name: 'Subsonic' }).getByRole('group', { name: 'Songs' });
  const rowOf = (title: string) => songs.locator('.row', { has: page.getByRole('button', { name: `Play ${title}`, exact: true }) });
  const still = rowOf('Still Water');
  await expect(still).toBeVisible();

  // At rest: neither shows.
  await page.mouse.move(1, 1);
  await expect(still.locator('.art-play')).toHaveCSS('opacity', '0');
  await expect(still.locator('span.more')).toHaveCSS('opacity', '0');
  // The pointer over the row shows both.
  await still.hover();
  await expect(still.locator('.art-play')).toHaveCSS('opacity', '1');
  await expect(still.locator('span.more')).toHaveCSS('opacity', '1');
  // The play button plays from that row, as activating the row does: the list from it on.
  await still.locator('.art-play').click();
  await waitPlaying(page, 'Still Water');
  const now = await snapshot(page);
  expect(now.queueLength).toBe(2);
  expect(await page.evaluate(() => (globalThis as any).jellyfesh.player.queue.position)).toBe(1);

  // Keyboard focus shows them too.
  await page.mouse.move(1, 1);
  const rope = rowOf('Rope and Mast');
  await rope.getByRole('button', { name: 'Play Rope and Mast' }).focus();
  await page.keyboard.press('Tab');
  await expect(rope.getByRole('button', { name: 'More for Rope and Mast' })).toBeFocused();
  await expect(rope.locator('span.more')).toHaveCSS('opacity', '1');
  await page.keyboard.press('Shift+Tab');
  await expect(rope.getByRole('button', { name: 'Play Rope and Mast' })).toBeFocused();
  await expect(rope.locator('.art-play')).toHaveCSS('opacity', '1');
  await page.keyboard.press('Enter');
  await waitPlaying(page, 'Rope and Mast');
});

test('Up Next offers a source’s song its album and artist; right-click opens its menu', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await page.getByRole('region', { name: 'Now Playing' }).getByRole('button', { name: 'Queue' }).click();
  const nowPlaying = page.getByRole('dialog', { name: 'Now Playing' });
  const entry = nowPlaying.getByRole('button', { name: 'Play Rope and Mast' });
  await expect(entry).toBeVisible();
  await entry.click({ button: 'right' });
  await expect(openMenu()).toBeVisible();
  expect(await labels()).toEqual(['Play', 'Move Up', 'Move Down', 'Remove', 'Go to Album', 'Go to Artist']);
  await openMenu().getByRole('menuitem', { name: 'Go to Artist' }).click();
  await expect(page.getByRole('heading', { name: 'Glass Harbour', level: 1 })).toBeVisible();
  await expect(nowPlaying).toBeHidden();
});

test('a source’s song keeps its album and artist in the queue kept across a reload', async () => {
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Home', level: 1 })).toBeVisible();
  // The album's songs are short: whichever of them is current now.
  const bar = page.getByRole('region', { name: 'Now Playing' });
  await expect(bar).toBeVisible();
  await bar.getByRole('button', { name: 'More', exact: true }).click();
  await expect(openMenu().getByRole('menuitem', { name: 'Go to Artist' })).toBeVisible();
  await openMenu().getByRole('menuitem', { name: 'Go to Album' }).click();
  await expect(page.getByRole('heading', { name: 'Slow Tide', level: 1 })).toBeVisible();
});
