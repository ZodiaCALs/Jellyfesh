// The v0.0.4 design (docs/design-controls.md, "v0.0.4: the controls as built") against the mock
// Subsonic server: the straight seek bar (its geometry, hover, dragging, the keyboard), the player
// bar at 1280 px and the capsule at 390 px, the touch seek bar, transparency on the glass, a
// swatch fixing the accent hue, lyrics size and alignment, reduced motion, and nothing animating
// or redrawing while paused.

import { expect, type Locator, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInSubsonic, snapshot, waitPlaying } from './helpers';

test.describe.configure({ mode: 'serial' });

let page: Page;

async function play(target: Page) {
  await openAlbum(target, 'Subsonic', 'Slow Tide');
  await target.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(target, 'Morning Harbour');
}

async function pauseAt(target: Page, ms: number) {
  await target.evaluate(() => (globalThis as any).jellyfesh.player.pause());
  await target.evaluate((at) => (globalThis as any).jellyfesh.player.seek(at), ms);
  await expect.poll(async () => Math.abs((await snapshot(target)).positionMs - ms)).toBeLessThan(150);
}

function height(locator: Locator): Promise<number> {
  return locator.evaluate((element) => element.getBoundingClientRect().height);
}

function style(locator: Locator, property: string): Promise<string> {
  return locator.evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), property);
}

/** The alpha of a CSS colour as the browser computes it. */
function alphaOf(colour: string): number {
  const numbers = colour.match(/[\d.]+/g)?.map(Number) ?? [];
  return numbers.length >= 4 ? numbers[3] : 1;
}

/** Counts view transitions from now on (Now Playing's artwork growing out of the player's). */
async function countViewTransitions(target: Page) {
  await target.evaluate(() => {
    const doc = document as any;
    doc.viewTransitions = 0;
    if (doc.countingViewTransitions || typeof doc.startViewTransition !== 'function') return;
    const original = doc.startViewTransition.bind(doc);
    doc.startViewTransition = (update: () => Promise<void>) => {
      doc.viewTransitions += 1;
      return original(update);
    };
    doc.countingViewTransitions = true;
  });
}

function viewTransitions(target: Page): Promise<number> {
  return target.evaluate(() => (document as any).viewTransitions as number);
}

async function openPreferences() {
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Appearance' }).click();
  await expect(page.getByRole('heading', { name: 'Theme' })).toBeVisible();
}

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
  await openApp(page);
  await signInSubsonic(page);
  await play(page);
});

test.afterAll(async () => {
  await page.close();
});

test('shows the player bar at 1280 px', async () => {
  await pauseAt(page, 1000);
  const bar = page.getByRole('region', { name: 'Now Playing' });
  await expect(bar).toBeVisible();
  expect(await height(bar)).toBe(80);
  // A Veil capsule (Luminous): r-2xl.
  expect(await style(bar.locator('.inner'), 'border-radius')).toBe('24px');
  await expect(bar).toContainText('Morning Harbour');
  for (const name of ['Previous', 'Next', 'Lyrics', 'Queue', 'Shuffle, off', 'Repeat, off']) {
    await expect(bar.getByRole('button', { name, exact: true })).toBeVisible();
  }
  await expect(bar.getByRole('slider', { name: 'Position' })).toBeVisible();
  await expect(bar.getByRole('slider', { name: 'Volume' })).toBeVisible();
  // Play is the bar's one accent fill (read once any theme crossfade has settled); the capsule is not drawn.
  await expect
    .poll(() =>
      bar.getByRole('button', { name: 'Play', exact: true }).evaluate((button) => {
        const accent = getComputedStyle(document.documentElement).getPropertyValue('--jf-accent').trim();
        return getComputedStyle(button).backgroundColor === accent;
      }),
    )
    .toBe(true);
  await expect(page.locator('.mini')).toHaveCount(0);
  // Lyrics opens Now Playing on the lyrics.
  await bar.getByRole('button', { name: 'Lyrics', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Lyrics' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
});

/** The visible elements that blur what is behind them (live backdrop-filter layers). */
function liveBlurLayers(target: Page): Promise<string[]> {
  return target.evaluate(() =>
    [...document.querySelectorAll('*')]
      .filter((element) => {
        const style = getComputedStyle(element);
        const box = element.getBoundingClientRect();
        return style.backdropFilter !== 'none' && style.display !== 'none' && box.width > 0 && box.height > 0;
      })
      .map((element) => `${element.tagName.toLowerCase()}.${[...element.classList].join('.')}`),
  );
}

test('follows Luminous: type, covers and the glass budget', async () => {
  // Page titles 48 px (type.text-h1); the sidebar is Clear glass.
  await nav(page, 'Home').click();
  expect(await style(page.locator('.page-title').first(), 'font-size')).toBe('48px');
  expect(await style(page.locator('nav.sidebar'), 'backdrop-filter')).toContain('blur(24px)');
  // Grid covers: radius 16 with shadow-2.
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  // The artist's page shows its albums in a grid.
  await page.locator('button.artist').click();
  const cover = page.locator('.card .cover').first();
  await expect(cover).toBeVisible();
  expect(await style(cover, 'border-top-left-radius')).toBe('16px');
  expect(await style(cover, 'box-shadow')).not.toBe('none');
  // At most three live blur layers on a desktop, in the library and in Now Playing (40 px lyrics).
  const library = await liveBlurLayers(page);
  expect(library.length, library.join(', ')).toBeLessThanOrEqual(3);
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await expect(page.getByRole('region', { name: 'Lyrics' })).toBeVisible();
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset.viewTransition ?? '')).toBe('');
  const nowPlaying = await liveBlurLayers(page);
  expect(nowPlaying.length, nowPlaying.join(', ')).toBeLessThanOrEqual(3);
  expect(await style(page.locator('.lyrics .line .text').first(), 'font-size')).toBe('40px');
  // Reduce Motion and Transparency: no blur at all.
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.reduce_effects = true)));
  try {
    await expect.poll(() => liveBlurLayers(page)).toEqual([]);
  } finally {
    await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.reduce_effects = false)));
  }
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
  // Two on a phone.
  await page.setViewportSize({ width: 390, height: 844 });
  try {
    await nav(page, 'Home').click();
    await expect(page.getByRole('region', { name: 'Now Playing' })).toBeVisible();
    const phone = await liveBlurLayers(page);
    expect(phone.length, phone.join(', ')).toBeLessThanOrEqual(2);
    await page.getByRole('button', { name: 'Open Now Playing' }).click();
    await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeVisible();
    const phoneNowPlaying = await liveBlurLayers(page);
    expect(phoneNowPlaying.length, phoneNowPlaying.join(', ')).toBeLessThanOrEqual(2);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
  } finally {
    await page.setViewportSize({ width: 1280, height: 800 });
  }
});

test('shows the capsule with a straight 2 px line at 390 px', async () => {
  await page.setViewportSize({ width: 390, height: 844 });
  try {
    const capsule = page.getByRole('region', { name: 'Now Playing' });
    await expect(capsule).toBeVisible();
    await expect(page.locator('.bar')).toHaveCount(0);
    expect(await height(capsule)).toBe(64);
    // The artwork and title (which open Now Playing), play and next (docs/ui.md, v0.0.8), and no
    // seek bar.
    await expect(capsule.getByRole('slider')).toHaveCount(0);
    await expect(capsule.getByRole('button')).toHaveCount(3);
    await expect(capsule.getByRole('button', { name: 'Next', exact: true })).toBeVisible();
    const box = (await capsule.boundingBox())!;
    const line = (await capsule.locator('.progress').boundingBox())!;
    expect(line.height).toBe(2);
    // Along the bottom edge, inset by the capsule's radius from both ends.
    expect(line.y + line.height).toBeCloseTo(box.y + box.height, 0);
    expect(line.x - box.x).toBeCloseTo(32, 0);
    expect(box.x + box.width - (line.x + line.width)).toBeCloseTo(32, 0);
  } finally {
    await page.setViewportSize({ width: 1280, height: 800 });
  }
});

test('draws the seek bar as a straight line that thickens on hover and while dragged', async () => {
  await pauseAt(page, 1000);
  const bar = page.getByRole('region', { name: 'Now Playing' });
  const slider = bar.getByRole('slider', { name: 'Position' });
  const line = slider.locator('.line');
  const thumb = slider.locator('.thumb');
  // At rest: a 4 px straight line with round caps, no wave, no thumb.
  await page.mouse.move(10, 10);
  await expect.poll(() => height(line)).toBe(4);
  expect(Number.parseFloat(await style(line, 'border-top-left-radius'))).toBeGreaterThanOrEqual(2);
  expect(await style(slider.locator('.played'), 'mask-image')).toBe('none');
  expect(await height(slider.locator('.played'))).toBe(4);
  await expect.poll(() => style(thumb, 'opacity')).toBe('0');

  // Hover: 6 px and the 14 px thumb.
  await slider.hover();
  await expect.poll(() => height(line)).toBe(6);
  await expect.poll(() => style(thumb, 'opacity')).toBe('1');
  expect(await height(thumb)).toBe(14);

  // Dragging: 8 px, the times follow the thumb, and the song seeks on release.
  const box = (await slider.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.25, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.75, y, { steps: 6 });
  await expect.poll(() => height(line)).toBe(8);
  await expect(bar.locator('.time.start')).toHaveText('0:06');
  await expect(bar.locator('.time.end')).toHaveText('−0:02');
  await expect(slider).toHaveAttribute('aria-valuetext', '0:06 of 0:08');
  // Not yet: dragging only shows where it would go.
  expect(Math.abs((await snapshot(page)).positionMs - 1000)).toBeLessThan(150);
  await page.mouse.up();
  await expect.poll(async () => (await snapshot(page)).positionMs).toBeGreaterThan(5800);
  expect((await snapshot(page)).positionMs).toBeLessThan(6200);

  // The thumb sits where the fill ends.
  const lineBox = (await line.boundingBox())!;
  const thumbBox = (await thumb.boundingBox())!;
  const fraction = (await snapshot(page)).positionMs / 8000;
  expect(thumbBox.x + thumbBox.width / 2).toBeCloseTo(lineBox.x + lineBox.width * fraction, -1);
});

test('steps the seek bar with the keyboard and reads its value', async () => {
  await pauseAt(page, 2000);
  const slider = page.getByRole('region', { name: 'Now Playing' }).getByRole('slider', { name: 'Position' });
  await page.mouse.move(10, 10);
  await slider.focus();
  const position = async () => (await snapshot(page)).positionMs;
  await page.keyboard.press('Home');
  await expect.poll(position).toBeLessThan(150);
  await expect(slider).toHaveAttribute('aria-valuetext', '0:00 of 0:08');
  await page.keyboard.press('ArrowRight');
  await expect.poll(position).toBeGreaterThan(4850);
  expect(await position()).toBeLessThan(5150);
  await expect(slider).toHaveAttribute('aria-valuetext', '0:05 of 0:08');
  await page.keyboard.press('ArrowLeft');
  await expect.poll(position).toBeLessThan(150);
  await page.keyboard.press('ArrowRight');
  await expect.poll(position).toBeGreaterThan(4850);
  // Page Down goes back 30 s, which is the start here.
  await page.keyboard.press('PageDown');
  await expect.poll(position).toBeLessThan(150);
  // Focus shows the thumb as hover does.
  await expect.poll(() => style(slider.locator('.thumb'), 'opacity')).toBe('1');
});

test('sets the glass tints from the transparency setting', async () => {
  await openPreferences();
  const slider = page.getByRole('slider', { name: 'Transparency', exact: true });
  const veil = () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--jf-tint-veil'));
  const expected = (t: number) =>
    page.evaluate((transparency) => {
      const app = (globalThis as any).jellyfesh;
      const token = app.theme.themes[app.theme.appearance].colors['glass.tint-veil'][3] / 255;
      return Math.round(app.jf.glass_alpha(token, transparency) * 255) / 255;
    }, t);
  // 50 %: the token's own alpha.
  await expect.poll(async () => Math.abs(alphaOf(await veil()) - (await expected(0.5)))).toBeLessThan(0.005);
  // Solid: opaque glass.
  await slider.focus();
  await page.keyboard.press('Home');
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.transparency)).toBe(0);
  await expect.poll(async () => alphaOf(await veil())).toBe(1);
  // Clear: half the token's alpha, as glass_alpha says.
  await page.keyboard.press('End');
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.transparency)).toBe(1);
  const clear = await expected(1);
  await expect.poll(async () => Math.abs(alphaOf(await veil()) - clear)).toBeLessThan(0.005);
  // The player bar is that glass.
  await expect
    .poll(async () => Math.abs(alphaOf(await style(page.locator('.bar .inner'), 'background-color')) - clear))
    .toBeLessThan(0.005);
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.transparency = 0.5)));
});

test('fixes the accent hue with a swatch', async () => {
  await openPreferences();
  const follow = page.getByRole('switch', { name: 'Follow the Artwork' });
  if (await follow.isChecked()) await follow.click();
  await expect(follow).not.toBeChecked();
  await page.getByRole('radio', { name: 'Teal' }).check();
  await expect
    .poll(() => page.evaluate(() => JSON.stringify((globalThis as any).jellyfesh.settings.value.hue_source)))
    .toBe(JSON.stringify({ kind: 'fixed', hue: 180 }));
  await expect.poll(() => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--jf-hue').trim())).toBe('180');
  // The accent is the core's for hue 180.
  const accent = await page.evaluate(() => {
    const app = (globalThis as any).jellyfesh;
    const [r, g, b] = JSON.parse(app.jf.theme_for_hue(180))[app.theme.appearance].colors['accent.accent'];
    return `rgb(${r}, ${g}, ${b})`;
  });
  await expect.poll(() => style(page.locator('.pv-play'), 'background-color')).toBe(accent);
  await expect(page.getByRole('radio', { name: 'Teal' })).toBeChecked();
  // The custom slider moves the hue too, live.
  const hue = page.getByRole('slider', { name: 'Custom Hue' });
  await hue.focus();
  await page.keyboard.press('ArrowRight');
  await expect
    .poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.hue_source.hue))
    .toBe(181);
  await expect(page.getByRole('radio', { name: 'Teal' })).not.toBeChecked();
  await follow.click();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.settings.value.hue_source.kind)).toBe('artwork');
});

test('sizes and aligns lyrics from the settings', async () => {
  const fontSize = async () => {
    await page.getByRole('button', { name: 'Open Now Playing' }).click();
    const text = page.getByRole('region', { name: 'Lyrics' }).locator('.line .text').first();
    await expect(text).toBeVisible();
    const size = Number.parseFloat(await style(text, 'font-size'));
    const align = await style(page.locator('.lyrics .line').first(), 'text-align');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
    return { size, align };
  };
  const medium = await fontSize();
  expect(medium.align).toBe('start');
  await openPreferences();
  await page.getByRole('radio', { name: 'Large' }).check();
  await page.getByRole('radio', { name: 'Centre' }).check();
  const large = await fontSize();
  expect(large.size).toBeCloseTo(medium.size * 1.2, 0);
  expect(large.align).toBe('center');
  await openPreferences();
  await page.getByRole('radio', { name: 'Small' }).check();
  const small = await fontSize();
  expect(small.size).toBeCloseTo(medium.size * 0.8, 0);
  await page.evaluate(() =>
    (globalThis as any).jellyfesh.settings.update((d: any) => {
      d.lyrics_size = 'medium';
      d.lyrics_align = 'start';
    }),
  );
});

test('stops moving with reduced motion', async () => {
  await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.reduce_effects = true)));
  try {
    await expect(page.locator('html')).toHaveAttribute('data-motion', 'reduced');
    await countViewTransitions(page);
    await page.getByRole('button', { name: 'Open Now Playing' }).click();
    // Now Playing just fades in: the artwork does not grow out of the player's.
    await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeVisible();
    expect(await viewTransitions(page)).toBe(0);
    // Paused: the artwork does not shrink.
    const cover = page.locator('.hero .cover, .hero .art').first();
    await expect(cover).toBeVisible();
    await expect.poll(() => style(cover, 'scale')).toBe('none');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
    // The level glyph stands still.
    await openAlbum(page, 'Subsonic', 'Slow Tide');
    await expect(page.locator('.row.playing .level span').first()).toHaveCSS('animation-name', 'none');
  } finally {
    await page.evaluate(() => (globalThis as any).jellyfesh.settings.update((d: any) => (d.reduce_effects = false)));
  }
  // Without it, the artwork grows out of the player's (where the browser has view transitions,
  // as Chromium does) and settles at 0.88 while paused.
  await countViewTransitions(page);
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  await expect.poll(() => viewTransitions(page)).toBe(1);
  await expect.poll(() => style(page.locator('.hero .cover, .hero .art').first(), 'scale')).toBe('0.88');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
});

test('animates and redraws nothing while paused', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour', 1000);
  // While playing, the playing row's level glyph moves.
  await expect
    .poll(() => page.evaluate(() => document.getAnimations().filter((a) => a.playState === 'running').length))
    .toBeGreaterThan(0);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.pause());
  for (const where of ['library', 'now-playing'] as const) {
    if (where === 'now-playing') {
      await page.getByRole('button', { name: 'Open Now Playing' }).click();
      await expect(page.getByRole('region', { name: 'Lyrics' })).toBeVisible();
    }
    // Let opening motion, the artwork's scale and any buffering settle first (slow machines too).
    await expect
      .poll(() =>
        page.evaluate(() => {
          const app = (globalThis as any).jellyfesh;
          const moving = document.getAnimations().some((a) => a.playState === 'running');
          return !document.documentElement.dataset.viewTransition && !app.player.loading && !moving;
        }),
      )
      .toBe(true);
    await page.waitForTimeout(1000);
    const counts = await page.evaluate(
      () =>
        new Promise<{ frames: number; mutations: number; running: number; what: string[] }>((resolve) => {
          let frames = 0;
          let mutations = 0;
          const what: string[] = [];
          const original = window.requestAnimationFrame.bind(window);
          window.requestAnimationFrame = (callback) =>
            original((time) => {
              frames += 1;
              callback(time);
            });
          const observer = new MutationObserver((records) => {
            mutations += records.length;
            for (const record of records) {
              const target = record.target as Element;
              what.push(
                `${Math.round(performance.now())} ${record.type} ${target.nodeName}.${target.className ?? ''} ${record.attributeName ?? ''} ` +
                  `${record.attributeName ? JSON.stringify(target.getAttribute?.(record.attributeName)) : ''} +${record.addedNodes.length} -${record.removedNodes.length}`,
              );
            }
          });
          observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
          setTimeout(() => {
            observer.disconnect();
            window.requestAnimationFrame = original;
            const running = document.getAnimations().filter((a) => a.playState === 'running').length;
            resolve({ frames, mutations, running, what });
          }, 5000);
        }),
    );
    console.log(`paused, ${where}, 5 s: ${counts.frames} animation frames, ${counts.mutations} DOM changes, ${counts.running} running animations`);
    if (counts.what.length) console.log(`  changes:\n  ${counts.what.join('\n  ')}`);
    const { what: _what, ...measured } = counts;
    test.info().annotations.push({ type: `paused-${where}`, description: JSON.stringify(measured) });
    expect(measured).toEqual({ frames: 0, mutations: 0, running: 0 });
  }
  await page.keyboard.press('Escape');
});

test('uses the touch seek bar on touch screens', async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  const phone = await context.newPage();
  try {
    await openApp(phone);
    expect(await phone.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
    await signInSubsonic(phone);
    await play(phone);
    await phone.evaluate(() => (globalThis as any).jellyfesh.player.pause());
    await phone.getByRole('button', { name: 'Open Now Playing' }).click();
    const slider = phone.getByRole('slider', { name: 'Position' });
    await expect(slider).toBeVisible();
    // The page takes input again once the artwork has grown into place (a view transition).
    await expect.poll(() => phone.evaluate(() => document.documentElement.dataset.viewTransition ?? '')).toBe('');
    // 6 px (once Now Playing has finished opening), and no thumb.
    await expect.poll(() => height(slider.locator('.line'))).toBe(6);
    expect(await style(slider.locator('.thumb'), 'display')).toBe('none');
    // 12 px while dragged.
    const box = (await slider.boundingBox())!;
    await phone.mouse.move(box.x + box.width * 0.2, box.y + box.height / 2);
    await phone.mouse.down();
    await phone.mouse.move(box.x + box.width * 0.4, box.y + box.height / 2, { steps: 3 });
    await expect.poll(() => height(slider.locator('.line'))).toBe(12);
    await phone.mouse.up();
    await expect.poll(() => height(slider.locator('.line'))).toBe(6);
  } finally {
    await context.close();
  }
});
