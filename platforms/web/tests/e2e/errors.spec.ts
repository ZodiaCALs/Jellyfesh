// Servers a browser cannot use are explained, not reported as a generic failure.

import { expect, test } from '@playwright/test';
import { openApp, openSignIn, SUBSONIC_NO_CORS } from './helpers';

test('explains a server without CORS', async ({ page }) => {
  await openApp(page);
  const sheet = await openSignIn(page, 'Subsonic');
  await sheet.getByLabel('Server Address').fill(SUBSONIC_NO_CORS);
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await expect(sheet.getByRole('alert')).toContainText('does not allow requests from');
  await expect(sheet.getByRole('alert')).toContainText('CORS');
});

test('explains an address nothing answers at', async ({ page }) => {
  await openApp(page);
  const sheet = await openSignIn(page, 'Jellyfin');
  await sheet.getByLabel('Server Address').fill('http://127.0.0.1:4699');
  await sheet.getByRole('button', { name: 'Continue' }).click();
  await expect(sheet.getByRole('alert')).toContainText('Could not reach http://127.0.0.1:4699');
});
