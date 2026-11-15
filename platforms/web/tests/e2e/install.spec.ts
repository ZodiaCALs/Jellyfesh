// Sources › Install from File… in the browser: a plugin package (the build's own Subsonic package,
// saved to a file) is checked, transpiled in a worker, kept, run after a reload, and uninstalled
// after asking. Unsigned builds make it a community plugin, refused until community plugins are
// allowed, as in the native apps.

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import { openApp, openSourcesSettings, signInSubsonic, sourceCard } from './helpers';

let dir: string;
let file: string;

test.beforeAll(async ({ request }) => {
  const response = await request.get('plugins/org.jellyfesh.subsonic.jfpkg');
  expect(response.ok()).toBe(true);
  dir = mkdtempSync(join(tmpdir(), 'jf-install-'));
  file = join(dir, 'subsonic.jfpkg');
  writeFileSync(file, await response.body());
});

test.afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

async function installFromFile(page: Page) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Install from File…' }).click();
  await (await chooser).setFiles(file);
}

test('uninstalls a bundled plugin, installs it from a file and runs it', async ({ page }) => {
  await openApp(page);
  await openSourcesSettings(page);
  const card = sourceCard(page, 'Subsonic');
  await expect(card).toBeVisible();

  await card.getByRole('button', { name: 'Uninstall…' }).click();
  const confirm = page.getByRole('dialog', { name: 'Uninstall Subsonic?' });
  await expect(confirm).toContainText('Its settings and sign-in are removed.');
  await confirm.getByRole('button', { name: 'Uninstall', exact: true }).click();
  await expect(card).toBeHidden();

  // A bundled plugin the person uninstalled stays uninstalled.
  await page.reload();
  await openSourcesSettings(page);
  await expect(card).toBeHidden();

  await installFromFile(page);
  const official = await page.evaluate(() =>
    fetch('plugins/index.json')
      .then((r) => r.json())
      .then((index) => index.plugins.find((p: { id: string }) => p.id === 'org.jellyfesh.subsonic').official as boolean),
  );
  if (!official) {
    await expect(page.getByText(/community plugins are turned off: allow them in Sources settings first/)).toBeVisible({
      timeout: 30_000,
    });
    await expect(card).toBeHidden();
    await page.getByRole('switch', { name: 'Allow Community Plugins' }).click();
    await page.getByRole('button', { name: 'I Understand, Allow' }).click();
    await expect(page.getByRole('switch', { name: 'Allow Community Plugins' })).toBeChecked();
    await installFromFile(page);
  }
  await expect(page.getByText('Installed Subsonic')).toBeVisible({ timeout: 30_000 });
  await expect(card).toContainText(/Version \d+\.\d+\.\d+/);

  // Kept across a reload, and it runs: sign in to the mock server and browse it.
  await page.reload();
  await openSourcesSettings(page);
  await expect(card).toBeVisible();
  await signInSubsonic(page);
  await page.getByRole('navigation', { name: 'Library sections' }).first().getByRole('button', { name: 'Home', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Subsonic' }).getByRole('button', { name: 'Open Albums' })).toBeVisible();
});

test('refuses a file that is not a plugin package', async ({ page }) => {
  await openApp(page);
  await openSourcesSettings(page);
  const bogus = join(dir, 'not-a-plugin.jfpkg');
  writeFileSync(bogus, 'this is not a zip file');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Install from File…' }).click();
  await (await chooser).setFiles(bogus);
  await expect(page.getByText(/Could not install not-a-plugin\.jfpkg: This is not a Jellyfesh plugin package/)).toBeVisible({
    timeout: 30_000,
  });
});
