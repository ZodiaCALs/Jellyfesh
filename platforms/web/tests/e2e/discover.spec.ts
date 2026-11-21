// Discover, details and Settings › Extensions in Jellyfesh Web (v0.0.7; docs/ui.md "Discover,
// details, comments and the lyrics bar" and "AI Lyrics, Customize and Pro"), against the mock
// Subsonic server, whose plugin (API 1.2) makes its own Discover shelves and describes songs.

import { expect, type Page, test } from '@playwright/test';
import { nav, openAlbum, openApp, signInSubsonic, sourceCard, waitPlaying } from './helpers';

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

test('opens a source’s Discover page with its shelves, See All and Browse', async () => {
  await nav(page, 'Sources').click();
  await page.getByRole('region', { name: 'Your Sources' }).getByRole('button', { name: /^Subsonic/ }).click();
  await expect(page.getByRole('heading', { name: 'Subsonic', level: 1 })).toBeVisible();
  await expect(page.getByText('Signed in as demo')).toBeVisible();
  // The plugin's own shelves, in its words: rows, then a grid.
  for (const title of ['Recently Added', 'Most Played', 'Recently Played', 'Random Albums']) {
    await expect(page.getByRole('region', { name: title })).toBeVisible();
  }
  const recent = page.getByRole('region', { name: 'Recently Added' });
  await expect(recent.getByRole('button', { name: 'Open Slow Tide' })).toBeVisible();
  // See All browses the shelf's node.
  await recent.getByRole('button', { name: 'See All Recently Added' }).click();
  await expect(page.getByRole('button', { name: 'Open Slow Tide' })).toBeVisible();
  await page.getByRole('button', { name: 'Back' }).click();
  // Browse: the source's own folders.
  await page.getByRole('button', { name: 'Browse', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open Albums' })).toBeVisible();
});

test('Get Info shows About This Song for a source’s song', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'More for Morning Harbour' }).click();
  await page.getByRole('menuitem', { name: 'Get Info' }).click();
  const sheet = page.getByRole('dialog', { name: 'About This Song' });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByText('Morning Harbour')).toBeVisible();
  await expect(sheet.getByText('Source', { exact: true })).toBeVisible();
  await expect(sheet.getByText('Subsonic', { exact: true })).toBeVisible();
  await expect(sheet.getByText('Length', { exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();
});

test('Now Playing’s Info shows the Audio rows with the quality label', async () => {
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  await page.getByRole('button', { name: 'Play Morning Harbour' }).click();
  await waitPlaying(page, 'Morning Harbour');
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  const dialog = page.getByRole('dialog', { name: 'Now Playing' });
  await dialog.getByRole('button', { name: 'Info', exact: true }).click();
  const info = dialog.getByRole('region', { name: 'Info' });
  await expect(info.getByText('Audio', { exact: true })).toBeVisible();
  // The mock streams WAV: lossless, at 706 kbit/s.
  await expect(info.getByText('Lossless', { exact: true })).toBeVisible();
  await expect(info.getByText('WAV', { exact: true })).toBeVisible();
  await expect(info.getByText('706 kbit/s')).toBeVisible();
  await page.keyboard.press('Escape');
});

test('Settings › Extensions: Jellyfesh Pro, its licence key, and the extensions', async () => {
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Extensions' }).click();
  await expect(page.getByRole('heading', { name: 'Jellyfesh Pro' })).toBeVisible();
  // This build has no Pro Preview and no release keys: Pro is off, and a key is refused.
  await expect(page.getByText('AI Lyrics and Customize are part of Jellyfesh Pro.')).toBeVisible();
  await page.getByRole('button', { name: 'Enter Licence Key…' }).click();
  const sheet = page.getByRole('dialog', { name: 'Enter Licence Key' });
  await sheet.getByLabel('Licence Key').fill('not a key');
  await sheet.getByRole('button', { name: 'Activate' }).click();
  await expect(sheet.getByRole('alert')).toHaveText('That doesn’t look like a Jellyfesh licence key.');
  await sheet.getByRole('button', { name: 'Cancel' }).click();
  // The bundled extensions, each with its version and author and Uninstall….
  const list = page.getByRole('tabpanel', { name: 'Extensions' });
  await expect(list.getByRole('heading', { name: 'AI Lyrics' })).toBeVisible();
  await expect(list.getByRole('heading', { name: 'Customize' })).toBeVisible();
  await expect(list.getByText(/Version 0\.0\.7 · Jellyfesh/).first()).toBeVisible();
  await expect(list.getByRole('button', { name: 'Uninstall…' }).first()).toBeVisible();
  // Extensions are not sources.
  await page.getByRole('tab', { name: 'Sources' }).click();
  await expect(page.getByRole('heading', { name: 'AI Lyrics' })).toHaveCount(0);
});

test('Create Synced Lyrics opens the Pro sheet while Pro is off', async () => {
  // The build's packages are unsigned here: AI Lyrics runs once the community disclosure is accepted.
  await nav(page, 'Settings').click();
  await page.getByRole('tab', { name: 'Extensions' }).click();
  const card = sourceCard(page, 'AI Lyrics');
  const enabled = card.getByRole('switch', { name: 'Enabled' });
  if (!(await enabled.isChecked())) {
    await enabled.click();
    const allow = page.getByRole('button', { name: 'I Understand, Allow' });
    if (await allow.isVisible()) await allow.click();
  }
  await expect(enabled).toBeChecked();
  await openAlbum(page, 'Subsonic', 'Slow Tide');
  // Rope and Mast has no lyrics on the mock server.
  await page.getByRole('button', { name: 'Play Rope and Mast' }).click();
  await waitPlaying(page, 'Rope and Mast');
  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  const dialog = page.getByRole('dialog', { name: 'Now Playing' });
  await dialog.getByRole('button', { name: 'Lyrics', exact: true }).click();
  await expect(dialog.getByText('No Lyrics', { exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'Create Synced Lyrics' }).click();
  const sheet = page.getByRole('dialog', { name: 'Jellyfesh Pro' });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByText('AI Lyrics makes synced lyrics for any song.', { exact: false })).toBeVisible();
  await expect(sheet.getByRole('button', { name: 'Enter Licence Key…' })).toBeVisible();
  await sheet.getByRole('button', { name: 'Not Now' }).click();
  await expect(sheet).toBeHidden();
  await page.keyboard.press('Escape');
});
