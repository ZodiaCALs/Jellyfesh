// The music library: a folder of generated, tagged sine tones (invented titles) added with the
// folder picker browsers without folder handles use, then browsed, searched, played, kept across a
// reload and removed.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import { taggedWav } from '../mock/media.mjs';
import { nav, openApp, snapshot, waitPlaying } from './helpers';

const LRC = '[00:00.30]Harbour lights\n[00:02.50]Over the water\n';
let root: string;
let folder: string;

test.beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), 'jf-library-'));
  folder = join(root, 'Music');
  mkdirSync(join(folder, 'Tide'), { recursive: true });
  mkdirSync(join(folder, 'Loose'), { recursive: true });
  writeFileSync(
    join(folder, 'Tide', '01 Harbour.wav'),
    taggedWav(4, 440, { title: 'Harbour', artist: 'Aurora', album: 'Tide', track: 1, year: 2021 }),
  );
  writeFileSync(join(folder, 'Tide', '01 Harbour.lrc'), LRC);
  writeFileSync(
    join(folder, 'Tide', '02 Lantern.wav'),
    taggedWav(4, 494, { title: 'Lantern', artist: 'Aurora', album: 'Tide', track: 2, year: 2021 }),
  );
  writeFileSync(join(folder, 'Loose', 'Solo.wav'), taggedWav(4, 523, { title: 'Solo', artist: 'Bo feat. Cy' }));
  writeFileSync(join(folder, 'Loose', 'notes.txt'), 'not music');
});

test.afterAll(() => {
  rmSync(root, { recursive: true, force: true });
});

/** The folder picker of browsers that keep no folders (Playwright cannot drive the native one). */
async function useInputPicker(page: Page) {
  await page.addInitScript(() => {
    delete (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker;
  });
}

test('adds a folder, browses, searches and plays it, and keeps it across a reload', async ({ page }) => {
  await useInputPicker(page);
  await openApp(page);
  await expect(page.getByRole('heading', { name: 'Welcome to Jellyfesh' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add Music Folder…' }).click();
  await (await chooser).setFiles(folder);

  // Home: the size line (Solo has no album; "Bo feat. Cy" is two artists) and Recently Added.
  await expect(page.getByText('3 songs · 1 album · 3 artists')).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole('region', { name: 'Recently Added' }).getByRole('button', { name: 'Open Tide' })).toBeVisible();

  await nav(page, 'Albums').click();
  await expect(page.getByRole('button', { name: 'Open Tide' })).toBeVisible();
  await nav(page, 'Artists').click();
  for (const name of ['Aurora', 'Bo', 'Cy']) await expect(page.getByRole('button', { name: `Open ${name}` })).toBeVisible();
  await nav(page, 'Songs').click();
  for (const title of ['Harbour', 'Lantern', 'Solo']) await expect(page.getByRole('button', { name: `Play ${title}` })).toBeVisible();

  // An album plays in track order, with its lyrics file.
  await nav(page, 'Albums').click();
  await page.getByRole('button', { name: 'Open Tide' }).click();
  await expect(page.getByRole('heading', { name: 'Tide', level: 1 })).toBeVisible();
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await waitPlaying(page, 'Harbour');
  await expect.poll(async () => (await snapshot(page)).lyrics).toBe('ready');

  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  await expect(page.getByRole('region', { name: 'Songs' }).getByRole('button', { name: 'Play Lantern' })).toBeVisible();

  // The catalogue and the queue come back after a reload; this browser asks for the folder again.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Search', level: 1 })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(3);
  expect((await snapshot(page)).title).toBe('Harbour');
  await nav(page, 'Songs').click();
  await expect(page.getByText('Pick “Music” again to play its songs')).toBeVisible();
  const again = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Pick Folder…' }).click();
  await (await again).setFiles(folder);
  await expect(page.getByText('Pick “Music” again to play its songs')).toBeHidden();
  await page.getByRole('button', { name: 'Play Solo' }).click();
  await waitPlaying(page, 'Solo');
});

test('removes a folder after asking', async ({ page }) => {
  await useInputPicker(page);
  await openApp(page);
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Library' }).click();
  if (await page.getByText('No folders yet').isHidden()) {
    await page.getByRole('button', { name: 'Remove Folder…' }).first().click();
    await page.getByRole('button', { name: 'Remove Folder', exact: true }).click();
  }
  await expect(page.getByText('No folders yet')).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add Folder…' }).click();
  await (await chooser).setFiles(folder);
  await expect(page.getByText('Music', { exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(3);
  await page.getByRole('button', { name: 'Remove Folder…' }).click();
  const confirm = page.getByRole('dialog', { name: 'Remove “Music” from the library?' });
  await expect(confirm).toContainText('The files stay where they are.');
  await confirm.getByRole('button', { name: 'Remove Folder', exact: true }).click();
  await expect(page.getByText('No folders yet')).toBeVisible();
  await expect.poll(() => page.evaluate(() => (globalThis as any).jellyfesh.library.stats.tracks)).toBe(0);
});
