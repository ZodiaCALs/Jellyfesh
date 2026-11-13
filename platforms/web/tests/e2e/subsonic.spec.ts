// Jellyfesh Web against the mock Subsonic server: sign in, browse, search, play, lyrics, next and
// previous, crossfade on and off, and the gap between gapless tracks.

import { expect, type Page, test } from '@playwright/test';
import {
  nav,
  openAlbum,
  openApp,
  setCrossfade,
  signInSubsonic,
  snapshot,
  transitions,
  waitPlaying,
} from './helpers';

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

test('signs in and shows the server on Home', async () => {
  await nav(page, 'Home').click();
  const shelf = page.getByRole('region', { name: 'Subsonic' });
  await expect(shelf.getByRole('button', { name: 'Open Albums' })).toBeVisible();
  await expect(shelf.getByRole('button', { name: 'Open Artists' })).toBeVisible();
  await nav(page, 'Sources').click();
  await expect(page.locator('li.source', { hasText: 'Subsonic' })).toContainText('Signed in as demo');
});

test('browses to an album and plays it, with word-timed lyrics advancing', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await expect(page.getByRole('button', { name: 'Play Morning Harbour' })).toBeVisible();
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Morning Harbour');
  await expect(page.getByRole('region', { name: 'Now Playing' }).getByText('Morning Harbour')).toBeVisible();

  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  const lyrics = page.getByRole('region', { name: 'Lyrics' });
  await expect(lyrics).toBeVisible();
  // The translation (same-time CJK lines) shows under its line.
  await expect(lyrics.getByText('清晨把港湾折进光里')).toBeVisible();
  const active = lyrics.locator('button[aria-current="true"]');
  await expect(active).toHaveText(/Morning folds the harbour into light|Every rope and mast remembers night/);
  // Word by word: the active line is split into words, and the line moves on.
  await expect(lyrics.locator('.line.active .word').first()).toBeVisible();
  await expect(active).toHaveText(/Every rope and mast remembers night/, { timeout: 10_000 });
  await expect(active).toHaveText(/Slow tide, carry me/, { timeout: 10_000 });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Now Playing' })).toBeHidden();
});

test('goes to the next and previous songs', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  const mini = page.getByRole('region', { name: 'Now Playing' });
  await page.keyboard.press('Control+ArrowRight');
  await waitPlaying(page, 'Rope and Mast', 100);
  await expect(mini).toContainText('Rope and Mast');
  // Previous within three seconds goes back a song...
  await page.evaluate(() => (globalThis as any).jellyfesh.player.previous());
  await waitPlaying(page, 'Morning Harbour', 100);
  // ...and after three seconds restarts the song.
  await waitPlaying(page, 'Morning Harbour', 3500);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.previous());
  await expect.poll(async () => (await snapshot(page)).positionMs, { timeout: 5_000 }).toBeLessThan(2500);
  expect((await snapshot(page)).title).toBe('Morning Harbour');
});

test('searches the server', async () => {
  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  const results = page.getByRole('region', { name: 'Subsonic' });
  await expect(results.getByRole('button', { name: 'Play Lanterns' })).toBeVisible();
  await expect(results.getByRole('button', { name: 'Open Paper Lanterns' })).toBeVisible();
  await results.getByRole('button', { name: 'Play Lanterns' }).click();
  await waitPlaying(page, 'Lanterns');
  await page.getByRole('searchbox', { name: 'Search' }).fill('no such words');
  await expect(page.getByRole('heading', { name: 'No Results' })).toBeVisible();
});

test('joins songs gaplessly with crossfade off, and measures the gap', async () => {
  await setCrossfade(page, false);
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  // Listen to the output: the longest run of silence once sound has started.
  await page.evaluate(() => {
    const out = (globalThis as any).jellyfesh.player.engine.output as { context: AudioContext; node: AudioNode };
    const probe = out.context.createScriptProcessor(1024, 1, 1);
    const mute = out.context.createGain();
    mute.gain.value = 0;
    const stats = { started: false, run: 0, longest: 0, rate: out.context.sampleRate };
    probe.onaudioprocess = (event) => {
      const data = event.inputBuffer.getChannelData(0);
      for (let i = 0; i < data.length; i++) {
        const loud = Math.abs(data[i]) > 1e-3;
        if (!stats.started) {
          stats.started = loud;
          continue;
        }
        if (loud) {
          stats.longest = Math.max(stats.longest, stats.run);
          stats.run = 0;
        } else {
          stats.run++;
        }
      }
    };
    out.node.connect(probe);
    probe.connect(mute);
    mute.connect(out.context.destination);
    (globalThis as any).gapStats = stats;
  });
  // Skip to near the end of the first song, then through the join.
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(5000));
  await waitPlaying(page, 'Rope and Mast', 1500);
  const stats = await page.evaluate(() => (globalThis as any).gapStats);
  const gapMs = (stats.longest / stats.rate) * 1000;
  const joins = await transitions(page);
  const last = joins.at(-1)!;
  console.log(`gapless join: longest silence ${gapMs.toFixed(2)} ms (${stats.longest} samples at ${stats.rate} Hz), started ${last.leadMs.toFixed(1)} ms early`);
  test.info().annotations.push({ type: 'gap-ms', description: gapMs.toFixed(2) });
  expect(last.fadeMs).toBe(0);
  // Near-gapless: a gap under 50 ms (the join is timed on the element clock, not sample-exact).
  expect(gapMs).toBeLessThan(50);
});

test('crossfades between songs when crossfade is on', async () => {
  await setCrossfade(page, true, 2, false);
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(4000));
  await waitPlaying(page, 'Rope and Mast', 500);
  const last = (await transitions(page)).at(-1)!;
  expect(last.fadeMs).toBeGreaterThan(1500);
  expect(last.fadeMs).toBeLessThanOrEqual(2100);

  // Album continuity: songs of one album join without a fade.
  await setCrossfade(page, true, 2, true);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(4000));
  await waitPlaying(page, 'Lanterns', 500);
  expect((await transitions(page)).at(-1)!.fadeMs).toBe(0);

  // Off again: gapless.
  await setCrossfade(page, false);
  await page.evaluate(() => (globalThis as any).jellyfesh.player.seek(5000));
  await waitPlaying(page, 'Still Water', 500);
  expect((await transitions(page)).at(-1)!.fadeMs).toBe(0);
});
