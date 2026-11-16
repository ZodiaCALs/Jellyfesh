// Plugin API 1.1 in Jellyfesh Web (ADR-0018): a source whose manifest has [links] offers Open in
// {Service} for its items, and a source that plays on the person's own devices (playback_mode =
// "remote") is refused at install, since the page has no remote sessions. Both plugins are the
// build's Subsonic package with an invented manifest (tests/e2e/packages.ts), signed by nobody.

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import { openApp, openSourcesSettings, signInSubsonic, sourceCard } from './helpers';
import { repackage, swap } from './packages';

let dir: string;
let linked: string;
let remote: string;

/** Invented: example.com never answers for a real service. */
const LINKS = `
[links]
track = "https://music.example.com/song/{id}"
album = "https://music.example.com/album?id={id}"
`;

test.beforeAll(async ({ request }) => {
  const response = await request.get('plugins/org.jellyfesh.subsonic.jfpkg');
  expect(response.ok()).toBe(true);
  const subsonic = await response.body();
  const common = (manifest: string, id: string, name: string) =>
    swap(
      swap(
        swap(swap(manifest, /^id = ".*"$/m, `id = "${id}"`), /^name = ".*"$/m, `name = "${name}"`),
        /^api_version = ".*"$/m,
        'api_version = "1.1"',
      ),
      /^official = true$/m,
      'official = false',
    );
  dir = mkdtempSync(join(tmpdir(), 'jf-links-'));
  linked = join(dir, 'linked.jfpkg');
  writeFileSync(linked, repackage(subsonic, (m) => common(m, 'org.example.linked', 'Linked Sample') + LINKS));
  remote = join(dir, 'remote.jfpkg');
  writeFileSync(
    remote,
    repackage(subsonic, (m) =>
      swap(
        swap(common(m, 'org.example.remote', 'Remote Sample'), /"resolve_stream", /, ''),
        /^playback_mode = "stream"$/m,
        'playback_mode = "remote"',
      ).replace(/^capabilities = \[(.*)\]$/m, 'capabilities = [$1, "remote"]'),
    ),
  );
});

test.afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

async function installFromFile(page: Page, file: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Install from File…' }).click();
  await (await chooser).setFiles(file);
}

/** Links open through window.open; the tests keep what it was asked instead of opening tabs. */
async function recordOpenedLinks(page: Page) {
  await page.addInitScript(() => {
    const opened: unknown[][] = [];
    (globalThis as any).openedLinks = opened;
    window.open = (...args: unknown[]) => {
      opened.push(args);
      return null;
    };
  });
}

function openedLinks(page: Page): Promise<string[][]> {
  return page.evaluate(() => (globalThis as any).openedLinks as string[][]);
}

test('offers Open in {Service} for the items of a source with links', async ({ page }) => {
  await recordOpenedLinks(page);
  await openApp(page);
  await openSourcesSettings(page);
  await page.getByRole('switch', { name: 'Allow Community Plugins' }).click();
  await page.getByRole('button', { name: 'I Understand, Allow' }).click();
  await installFromFile(page, linked);
  await expect(page.getByText('Installed Linked Sample')).toBeVisible({ timeout: 30_000 });
  await signInSubsonic(page, undefined, 'Linked Sample');

  await page.getByRole('navigation', { name: 'Library sections' }).first().getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('lantern');
  const results = page.getByRole('region', { name: 'Linked Sample' });
  await expect(results.getByRole('button', { name: 'Play Lanterns' })).toBeVisible({ timeout: 30_000 });

  // In the song menu, after Go to Artist.
  await results.getByRole('button', { name: 'More for Lanterns' }).click();
  const items = page.getByRole('menuitem');
  await expect(page.getByRole('menuitem', { name: 'Open in Linked Sample' })).toBeVisible();
  const labels = await items.allTextContents();
  expect(labels.indexOf('Open in Linked Sample')).toBe(labels.length - 1);
  expect(labels.indexOf('Go to Artist')).toBeLessThan(labels.indexOf('Open in Linked Sample'));
  await page.getByRole('menuitem', { name: 'Open in Linked Sample' }).click();
  const opened = await openedLinks(page);
  expect(opened).toHaveLength(1);
  // The core filled in the song's id; a new tab gets no hold on the page.
  expect(opened[0][0]).toMatch(/^https:\/\/music\.example\.com\/song\/[A-Za-z0-9._~%-]+$/);
  expect(opened[0].slice(1)).toEqual(['_blank', 'noopener']);

  // The album page offers it in More too, with the album's link.
  await results.getByRole('button', { name: 'More for Lanterns' }).click();
  await page.getByRole('menuitem', { name: 'Go to Album' }).click();
  await page.getByRole('button', { name: /^More for / }).first().click();
  await page.getByRole('menuitem', { name: 'Open in Linked Sample' }).click();
  const after = await openedLinks(page);
  expect(after).toHaveLength(2);
  expect(after[1][0]).toMatch(/^https:\/\/music\.example\.com\/album\?id=[A-Za-z0-9._~%-]+$/);
});

test('refuses a source that plays on your own devices', async ({ page }) => {
  await openApp(page);
  await openSourcesSettings(page);
  await installFromFile(page, remote);
  await expect(
    page.getByText(
      'Could not install remote.jfpkg: Remote Sample plays on your own devices, which Jellyfesh Web cannot do yet. Use the Jellyfesh app on your computer or phone.',
    ),
  ).toBeVisible({ timeout: 30_000 });
  await expect(sourceCard(page, 'Remote Sample')).toHaveCount(0);
});
