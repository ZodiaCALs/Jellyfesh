// Servers a browser cannot use are explained, not reported as a generic failure.

import { expect, test } from '@playwright/test';
import { nav, openApp, SUBSONIC_NO_CORS } from './helpers';

test('explains a server without CORS', async ({ page }) => {
  await openApp(page);
  await nav(page, 'Sources').click();
  const card = page.locator('li.source', { has: page.getByRole('heading', { name: 'Subsonic' }) });
  await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  const allow = page.getByRole('button', { name: 'I Understand, Allow' });
  const sheet = page.getByRole('dialog', { name: 'Sign In to Subsonic' });
  await expect(allow.or(sheet)).toBeVisible();
  if (await allow.isVisible()) {
    await allow.click();
    await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  }
  await sheet.getByLabel('Server address').fill(SUBSONIC_NO_CORS);
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await expect(sheet.getByRole('alert')).toContainText('does not allow requests from');
  await expect(sheet.getByRole('alert')).toContainText('CORS');
});

test('explains an address nothing answers at', async ({ page }) => {
  await openApp(page);
  await nav(page, 'Sources').click();
  const card = page.locator('li.source', { has: page.getByRole('heading', { name: 'Jellyfin' }) });
  await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  const allow = page.getByRole('button', { name: 'I Understand, Allow' });
  const sheet = page.getByRole('dialog', { name: 'Sign In to Jellyfin' });
  await expect(allow.or(sheet)).toBeVisible();
  if (await allow.isVisible()) {
    await allow.click();
    await card.getByRole('button', { name: 'Sign In', exact: true }).click();
  }
  await sheet.getByLabel('Server address').fill('http://127.0.0.1:4699');
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await expect(sheet.getByRole('alert')).toContainText('Could not reach http://127.0.0.1:4699');
});
