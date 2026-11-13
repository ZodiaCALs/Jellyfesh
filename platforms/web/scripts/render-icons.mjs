// Renders the web app's icons from the Ice app icon (website/assets/icon-ice.svg, a full 1024 px
// tile): public/icons/icon.svg and the PNG sizes browsers and home screens ask for. The PNGs are
// committed; run this again when the icon changes (it needs Playwright's Chromium).
//
// Usage: node scripts/render-icons.mjs

import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const source = join(web, '../../website/assets/icon-ice.svg');
const out = join(web, 'public/icons');
mkdirSync(out, { recursive: true });
copyFileSync(source, join(out, 'icon.svg'));
const svg = readFileSync(source, 'utf8');

// [file, size]. The tile is full bleed with the glyph well inside the central 80 %, so the 512 px
// image also serves as the maskable icon (manifest: "any maskable").
const targets = [
  ['icon-192.png', 192],
  ['icon-512.png', 512],
  ['apple-touch-icon.png', 180],
];

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  for (const [file, size] of targets) {
    await page.setViewportSize({ width: size, height: size });
    await page.setContent(
      `<!doctype html><html><body style="margin:0">` +
        svg.replace('<svg ', '<svg style="width:100%;height:100%;display:block;position:absolute;inset:0" ') +
        `</body></html>`,
    );
    const png = await page.screenshot({ type: 'png' });
    writeFileSync(join(out, file), png);
    console.log(`render-icons: ${file} (${size} px)`);
  }
} finally {
  await browser.close();
}
