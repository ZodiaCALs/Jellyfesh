// The player bar keeps every part in its own column at any width (docs/ui.md, "The player bar and
// the mini player", v0.0.9): a dropped file with a very long title, artist and album, and lyrics
// whose long lines are timed word by word with long translations, at the full bar's widths. The
// title, the artist and the translation end with "…", the sung line fades out at its column's
// edge, the times stay beside the scrubber, and the controls never reach into the scrubber's
// column. Everything here is invented: a generated tone and made-up words. With JF_SHOTS set to a
// folder, the bar is saved there at each width.

import { expect, test } from '@playwright/test';
import { taggedWav } from '../mock/media.mjs';
import { openApp, waitPlaying } from './helpers';

const TITLE = 'A Very Long Song Title That Keeps Going Well Past the Edge of Its Column (Extended Harbour Mix)';
const ARTIST = 'The Lantern Keepers and the Tidal Choir of the Northern Harbour';
const LINES: [number, number, string, string][] = [
  [
    0.2,
    14.8,
    'Every lantern on the harbour wall remembers how the evening tide came softly over the stones and stayed until the morning',
    '港口墙上的每一盏灯笼都记得傍晚的潮水如何轻轻漫过石头，一直停留到清晨，再悄悄退回海里',
  ],
  [
    15,
    29.8,
    'And every rope and every mast is singing of the long night when the ferries waited for the light to find them',
    '每一根绳索和每一根桅杆都在歌唱那个漫长的夜晚，渡船一直等待着光把它们找到',
  ],
];

/** TTML with each word timed and a translation per line, as files.spec.ts drops. */
function ttml(): string {
  const words = (text: string, from: number, to: number) => {
    const list = text.split(' ');
    const step = (to - from) / list.length;
    return list
      .map((word, i) => `<span begin="${(from + i * step).toFixed(2)}s" end="${(from + (i + 1) * step).toFixed(2)}s">${word}${i < list.length - 1 ? ' ' : ''}</span>`)
      .join('');
  };
  const lines = LINES.map(
    ([begin, end, text, translation]) =>
      `<p begin="${begin}s" end="${end}s">${words(text, begin, end)}<span ttm:role="x-translation" xml:lang="zh-Hans">${translation}</span></p>`,
  );
  return `<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xml:lang="en"><body><div>
${lines.join('\n')}
</div></body></tt>`;
}

interface Box {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

test('keeps every part of the player bar in its own column, at every width', async ({ page }) => {
  await openApp(page);
  const audio = taggedWav(30, 440, { title: TITLE, artist: ARTIST, album: 'Harbour Lights: The Complete Recordings, Volume Two' }).toString(
    'base64',
  );
  const dataTransfer = await page.evaluateHandle(
    ({ audio, lyrics }) => {
      const bytes = Uint8Array.from(atob(audio), (c) => c.charCodeAt(0));
      const transfer = new DataTransfer();
      transfer.items.add(new File([bytes], 'Long Song.wav', { type: 'audio/wav' }));
      transfer.items.add(new File([lyrics], 'Long Song.ttml', { type: 'application/ttml+xml' }));
      return transfer;
    },
    { audio, lyrics: ttml() },
  );
  await page.dispatchEvent('body', 'dragenter', { dataTransfer });
  await page.dispatchEvent('body', 'drop', { dataTransfer });
  await waitPlaying(page, TITLE);
  const bar = page.getByRole('region', { name: 'Now Playing' });
  await expect(bar.locator('.lyric-line')).toContainText('Every lantern');
  await expect(bar.locator('.translation')).toContainText('港口墙上');

  for (const width of [900, 960, 1024, 1100, 1280, 1440]) {
    await page.setViewportSize({ width, height: 800 });
    await expect(bar.locator('.inner')).toBeVisible();
    // The bar's container queries settle with the layout.
    await page.waitForTimeout(150);
    const layout = await bar.evaluate((root) => {
      const box = (element: Element | null): Box | null => {
        if (!element) return null;
        const { left, right, top, bottom } = element.getBoundingClientRect();
        return { left, right, top, bottom };
      };
      const one = (selector: string) => box(root.querySelector(selector));
      const shown = (selector: string) =>
        [...root.querySelectorAll(selector)].map(box).filter((b): b is Box => b !== null && b.right - b.left > 0);
      const ends = (selector: string) => {
        const element = root.querySelector(selector) as HTMLElement | null;
        return element ? { overflowing: element.scrollWidth > element.clientWidth, ellipsis: getComputedStyle(element).textOverflow } : null;
      };
      return {
        inner: one('.inner'),
        open: one('.inner > .open'),
        middle: one('.inner > .middle'),
        end: one('.inner > .end'),
        texts: shown('.inner > .open .title, .inner > .open .lyric-line, .inner > .open .translation'),
        middleParts: shown('.inner > .middle button, .inner > .middle .time, .inner > .middle .track'),
        endParts: shown('.inner > .end > *'),
        start: one('.middle .time.start'),
        track: one('.middle .track'),
        stop: one('.middle .time.end'),
        title: ends('.inner > .open .title'),
        translation: ends('.inner > .open .translation'),
        sung: getComputedStyle(root.querySelector('.lyric-line .bar-line')!).maskImage,
      };
    });
    const where = `at ${width} px`;
    const { inner, open, middle, end } = layout;
    expect(inner && open && middle && end, where).toBeTruthy();
    // The three columns follow one another inside the bar.
    expect(open!.right, where).toBeLessThanOrEqual(middle!.left);
    expect(middle!.right, where).toBeLessThanOrEqual(end!.left);
    expect(end!.right, where).toBeLessThanOrEqual(inner!.right);
    // Each part stays in its column.
    for (const text of layout.texts) expect(text.right, `${where}: a line of the title column`).toBeLessThanOrEqual(open!.right + 0.5);
    for (const part of layout.middleParts) {
      expect(part.left, `${where}: a control of the middle column`).toBeGreaterThanOrEqual(middle!.left - 0.5);
      expect(part.right, `${where}: a control of the middle column`).toBeLessThanOrEqual(middle!.right + 0.5);
    }
    for (const part of layout.endParts) expect(part.left, `${where}: a control of the end column`).toBeGreaterThanOrEqual(end!.left - 0.5);
    // The times stay beside the scrubber, on its line.
    expect(layout.start!.right, where).toBeLessThanOrEqual(layout.track!.left);
    expect(layout.track!.right, where).toBeLessThanOrEqual(layout.stop!.left);
    const centre = (b: Box) => (b.top + b.bottom) / 2;
    expect(Math.abs(centre(layout.start!) - centre(layout.track!)), where).toBeLessThan(4);
    // The title and the translation end with "…"; the sung line fades out.
    expect(layout.title, where).toEqual({ overflowing: true, ellipsis: 'ellipsis' });
    expect(layout.translation, where).toEqual({ overflowing: true, ellipsis: 'ellipsis' });
    expect(layout.sung, where).toContain('linear-gradient');
    if (process.env.JF_SHOTS) await bar.screenshot({ path: `${process.env.JF_SHOTS}/player-bar-${width}.png` });
  }
});
