// A dragged-in audio file with a lyrics file next to it plays without any server, and is not
// kept: a generated sine tone and invented lyrics with a translation and a romanization.

import { expect, test } from '@playwright/test';
import { wav } from '../mock/media.mjs';
import { openApp, snapshot, waitPlaying } from './helpers';

const TTML = `<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xml:lang="ja">
  <body>
    <div>
      <p begin="0.3s" end="2.5s"><span begin="0.3s" end="1.2s">海の</span><span begin="1.2s" end="2.4s">灯</span><span ttm:role="x-translation" xml:lang="en">Lights of the sea</span><span ttm:role="x-roman">umi no akari</span></p>
      <p begin="2.6s" end="5s"><span begin="2.6s" end="3.6s">波に</span><span begin="3.6s" end="4.8s">ゆれて</span><span ttm:role="x-translation" xml:lang="en">Swaying on the waves</span><span ttm:role="x-roman">nami ni yurete</span></p>
    </div>
  </body>
</tt>`;

test('plays a dropped file with its lyrics file', async ({ page }) => {
  await openApp(page);
  const audio = wav(6, 523.25).toString('base64');
  const dataTransfer = await page.evaluateHandle(
    ({ audio, ttml }) => {
      const bytes = Uint8Array.from(atob(audio), (c) => c.charCodeAt(0));
      const transfer = new DataTransfer();
      transfer.items.add(new File([bytes], 'Harbour Tone.wav', { type: 'audio/wav' }));
      transfer.items.add(new File([ttml], 'Harbour Tone.ttml', { type: 'application/ttml+xml' }));
      return transfer;
    },
    { audio, ttml: TTML },
  );
  await page.dispatchEvent('body', 'dragenter', { dataTransfer });
  await expect(page.getByText('Drop audio files to play them. They are not saved.')).toBeVisible();
  await page.dispatchEvent('body', 'drop', { dataTransfer });
  await waitPlaying(page, 'Harbour Tone');

  await page.getByRole('button', { name: 'Open Now Playing' }).click();
  const lyrics = page.getByRole('region', { name: 'Lyrics' });
  await expect(lyrics.getByText('umi no akari')).toBeVisible();
  await expect(lyrics.getByText('Lights of the sea')).toBeVisible();
  await expect(lyrics.locator('button[aria-current="true"]')).toHaveText(/波にゆれて/, { timeout: 10_000 });

  await page.getByRole('button', { name: 'Info', exact: true }).click();
  await expect(page.getByText('Dropped on this page; not saved')).toBeVisible();

  // Nothing of the file survives a reload.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Home', level: 1 })).toBeVisible();
  expect((await snapshot(page)).queueLength).toBe(0);
});
