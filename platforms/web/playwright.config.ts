// End-to-end tests: the built app (vite preview of dist/) against the mock Subsonic and Jellyfin
// servers (tests/mock), in Chromium. Build first: `npm run build`. JF_APP_URL tests a copy served
// elsewhere instead, for example the website build with the app at /app/.

import { defineConfig } from '@playwright/test';

const APP = process.env.JF_APP_URL ?? 'http://localhost:4173/';
const preview = {
  command: 'npx vite preview --port 4173 --strictPort',
  url: APP,
  reuseExistingServer: !process.env.CI,
};

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 120_000,
  expect: { timeout: 15_000 },
  workers: 1,
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL: APP,
    viewport: { width: 1280, height: 800 },
    trace: 'retain-on-failure',
    launchOptions: {
      // Headless Chromium has no sound card; the tests read the output through Web Audio.
      args: ['--autoplay-policy=no-user-gesture-required'],
      executablePath: process.env.JF_CHROMIUM || undefined,
    },
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: [
    ...(process.env.JF_APP_URL ? [] : [preview]),
    {
      command: 'node tests/mock/serve.mjs',
      url: 'http://127.0.0.1:4620/System/Info/Public',
      reuseExistingServer: !process.env.CI,
    },
  ],
});
