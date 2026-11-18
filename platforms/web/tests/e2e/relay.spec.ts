// Services that refuse browsers, and the relay the person runs (ADR-0022 §7, docs/web-relay.md):
// the unofficial QQ Music plugin (its mock-service build, tests/e2e/relay-fixtures.ts) against a
// mock of WeChat's QR sign-in and QQ Music's web API that sends no CORS headers and wants the
// headers a page cannot set (tests/mock/qqmusic.mjs). Before sign-in Jellyfesh Web says the
// service refuses browsers; with the relay (tools/jf-relay, started here) set up in Settings ›
// Sources › Relay, the QR sign-in goes through end to end, the session cookie reaches the service,
// and covers load through the relay.

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, type Page, test } from '@playwright/test';
import { nav, openApp, openSourcesSettings, sourceCard } from './helpers';
import { mockQqMusicPackage, type Relay, relayBinary, startRelay } from './relay-fixtures';

const QQ_MOCK = 'http://127.0.0.1:4640';
const SOURCE = 'QQ Music (Unofficial)';

interface Seen {
  host: string;
  path: string;
  referer: string | null;
  userAgent: string | null;
  origin: string | null;
  hasCookie: boolean;
  signedIn?: boolean;
  calls?: string[];
  refused?: boolean;
}

let dir: string;
let packagePath: string;
let relay: Relay;

test.beforeAll(async ({ baseURL }) => {
  // Builds the relay and the plugin with cargo (seconds when they are up to date).
  test.setTimeout(15 * 60_000);
  dir = mkdtempSync(join(tmpdir(), 'jf-relay-'));
  packagePath = join(dir, 'community.unofficial.qqmusic.jfpkg');
  writeFileSync(packagePath, mockQqMusicPackage(dir));
  relay = await startRelay(relayBinary(), new URL(baseURL!).origin, [QQ_MOCK]);
});

test.afterAll(() => {
  relay?.stop();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

async function seen(page: Page): Promise<Seen[]> {
  return (await (await page.request.get(`${QQ_MOCK}/__mock/requests`)).json()) as Seen[];
}

/** Installs the plugin from its file with community plugins allowed, and turns it on. */
async function install(page: Page) {
  await openSourcesSettings(page);
  await page.getByRole('switch', { name: 'Allow Community Plugins' }).click();
  await page.getByRole('button', { name: 'I Understand, Allow' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Install from File…' }).click();
  await (await chooser).setFiles(packagePath);
  await expect(page.getByText(`Installed ${SOURCE}`)).toBeVisible({ timeout: 60_000 });
  await sourceCard(page, SOURCE).getByRole('switch', { name: 'Enabled' }).click();
  await page.getByRole('button', { name: 'I Understand, Allow' }).click();
  await expect(sourceCard(page, SOURCE).getByRole('switch', { name: 'Enabled' })).toBeChecked();
}

test('says before sign-in that the service refuses browsers, then signs in and shows covers through the relay', async ({ page }) => {
  await openApp(page);
  await install(page);
  const before = (await seen(page)).length;

  // Without a relay: the reason, in docs/ui.md's words, before anything is sent.
  await sourceCard(page, SOURCE).getByRole('button', { name: 'Sign In', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: `Sign In to ${SOURCE}` });
  await expect(
    sheet.getByText(
      'QQ Music doesn’t let browsers connect to it directly. Use the Jellyfesh app on your computer or phone, or set up a relay in Settings › Sources › Relay.',
    ),
  ).toBeVisible();
  await sheet.getByRole('button', { name: 'Close' }).last().click();
  expect((await seen(page)).length).toBe(before);

  // Settings › Sources › Relay.
  const group = page.getByRole('region', { name: 'Relay' });
  await expect(group.getByText('A relay you run yourself lets Jellyfesh Web reach services that browsers can’t. Jellyfesh doesn’t run one for you.')).toBeVisible();
  await group.getByLabel('Relay Address').fill(relay.address);
  await group.getByLabel('Relay Token').fill('not-the-token-at-all');
  await group.getByRole('button', { name: 'Save' }).click();
  await expect(group.getByText('The relay refused the token. Enter the token the relay was started with.')).toBeVisible();
  await group.getByLabel('Relay Token').fill(relay.token);
  await group.getByRole('button', { name: 'Save' }).click();
  await expect(group.getByText('Connected to the relay.')).toBeVisible();

  // The QR sign-in, through the relay: WeChat's page, its long poll (waiting, scanned,
  // confirmed), then QQ Music's sign-in and the account's name with the session cookie.
  await sourceCard(page, SOURCE).getByRole('button', { name: 'Sign In', exact: true }).click();
  await expect(sheet).toBeVisible();
  // The test build's own setting, among the plugin's optional ones.
  await sheet.getByText('More Settings').click();
  await sheet.getByLabel('Mock service (tests only)').fill(QQ_MOCK);
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await expect(sheet.getByText('Signed in as Mock QQ Listener.')).toBeVisible({ timeout: 60_000 });
  await sheet.getByRole('button', { name: 'Done' }).click();
  await expect(sourceCard(page, SOURCE).getByText('Signed in as Mock QQ Listener')).toBeVisible();

  const requests = (await seen(page)).slice(before);
  const wechat = requests.find((r) => r.host === 'open.weixin.qq.com');
  expect(wechat).toMatchObject({ referer: 'https://y.qq.com/' });
  expect(wechat!.userAgent).toMatch(/^Mozilla\/5\.0/);
  expect(requests.filter((r) => r.host === 'lp.open.weixin.qq.com').length).toBeGreaterThanOrEqual(3);
  const login = requests.find((r) => r.calls?.includes('music.login.LoginServer.Login'));
  expect(login).toMatchObject({ referer: 'https://y.qq.com/', origin: 'https://y.qq.com' });
  // The session went in Cookie, which a page may not set: the service saw the account signed in.
  const name = requests.find((r) => r.calls?.includes('music.UserInfo.userInfoServer.GetLoginUserInfo'));
  expect(name).toMatchObject({ hasCookie: true, signedIn: true });
  // The page's own origin never reached the service.
  expect(requests.some((r) => r.origin === new URL(page.url()).origin)).toBe(false);
  expect(requests.some((r) => r.refused)).toBe(false);
  // The relay logged methods, hosts and statuses only.
  const log = relay.log.join('\n');
  expect(log).toMatch(/POST 127\.0\.0\.1 -> 200/);
  expect(log).not.toContain(relay.token);
  expect(log).not.toContain('musics.fcg');
  expect(log).not.toContain('qm_keyst');

  // Search, signed in, and its covers through the relay.
  await nav(page, 'Search').click();
  await page.getByRole('searchbox', { name: 'Search' }).fill('mock song');
  const results = page.getByRole('region', { name: SOURCE });
  await expect(results.getByRole('button', { name: 'Play Mock Song One' })).toBeVisible({ timeout: 30_000 });
  // The cover came through the relay into memory, and it decodes.
  const cover = results.locator('img').first();
  await expect(cover).toHaveAttribute('src', /^blob:/, { timeout: 30_000 });
  await expect.poll(() => cover.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth)).toBeGreaterThan(0);
  const pictures = (await seen(page)).filter((r) => r.host === 'y.gtimg.cn');
  expect(pictures.length).toBeGreaterThan(0);
});
