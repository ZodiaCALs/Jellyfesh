// Favourites and Search with the sources as they are (docs/ui.md, "Favourites" and "Search";
// ADR-0027 §3), against the mock Subsonic server's second account, which has starred nothing:
// Favourites still names the signed-in source, saying it likes nothing yet; Search asks only the
// sources that are ready, so Jellyfin, switched on but not signed in, is left out.

import { expect, type Page, test } from '@playwright/test';
import { nav, NEWCOMER, openApp, openSourcesSettings, signInSubsonic, SUBSONIC, sourceCard } from './helpers';

test.describe.configure({ mode: 'serial' });

let page: Page;

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
  await openApp(page);
  await signInSubsonic(page, SUBSONIC, 'Subsonic', NEWCOMER);
});

test.afterAll(async () => {
  await page.close();
});

test('Favourites names a signed-in source whose likes came back empty', async () => {
  await nav(page, 'Favourites').click();
  const section = page.getByRole('region', { name: 'Subsonic' });
  await expect(section.getByText('Nothing liked in Subsonic yet.')).toBeVisible();
  await expect(section.getByText('Updated just now')).toBeVisible();
  await expect(section.getByRole('group')).toHaveCount(0);
  // Nothing is hearted either.
  await expect(page.getByRole('heading', { name: 'No Favourites Yet' })).toBeVisible();
  if (process.env.JF_SHOTS) await page.screenshot({ path: `${process.env.JF_SHOTS}/favourites-nothing-liked.png` });
});

test('Search leaves out a source that is switched on but not signed in', async () => {
  await openSourcesSettings(page);
  const jellyfin = sourceCard(page, 'Jellyfin');
  const enabled = jellyfin.getByRole('switch', { name: 'Enabled' });
  if (!(await enabled.isChecked())) {
    await enabled.click();
    // Unsigned builds make it a community plugin, which asks first.
    if (await jellyfin.getByText(/Community/).first().isVisible()) await page.getByRole('button', { name: 'I Understand, Allow' }).click();
  }
  await expect(enabled).toBeChecked();
  await expect(jellyfin.getByText('Not signed in')).toBeVisible();

  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  const panel = page.getByRole('tabpanel');
  await expect(panel.getByRole('region', { name: 'Subsonic' }).getByRole('button', { name: 'Play Lanterns' })).toBeVisible();
  await expect(panel).toHaveAttribute('aria-busy', 'false');
  // Only the ready source was asked: no tab, section or failure for Jellyfin.
  await expect(page.getByRole('tablist', { name: 'Results' }).getByRole('tab')).toHaveText(['All', 'Subsonic']);
  await expect(panel.getByRole('region', { name: 'Jellyfin' })).toHaveCount(0);
  await expect(page.getByText('This Source Did Not Answer')).toHaveCount(0);
});
