#!/usr/bin/env node
// Renders the promo: every frame of scene/index.html in Chromium (Playwright), encoded by ffmpeg.
//
//   node marketing/promo/render.mjs                          the whole video, with the music
//   node marketing/promo/render.mjs --fps 30                 another frame rate (default 60)
//   node marketing/promo/render.mjs --from 7 --to 20         a part, no music (build/part.mp4)
//   node marketing/promo/render.mjs --stills 1,12.4,33       PNG stills (build/stills/)
//   --build DIR   where frames and intermediate files go (default marketing/promo/build)
//   --workers N   pages rendering at once (default 3)
//
// The page draws the frame for a time t that this script sets (window.renderAt), so the result
// is the same however long a frame takes. The whole video is rendered once to JPEG frames
// (quality 98) and encoded in two passes to a fixed bitrate, so it stays under 25 MB; the frames
// are deleted afterwards. The music comes from music/make_music.py. Output:
// website/assets/video/jellyfesh-v0.0.7-public-beta.mp4 and poster.jpg (H.264 High, yuv420p,
// BT.709, +faststart, AAC 192 kb/s).
import { spawn, spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, promises as fsp, rmSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');
const require = createRequire(import.meta.url);

/** Playwright from this folder's node_modules, PLAYWRIGHT_MODULE, or the global npm modules. */
function playwright() {
  const candidates = ['playwright'];
  if (process.env.PLAYWRIGHT_MODULE) candidates.unshift(process.env.PLAYWRIGHT_MODULE);
  try {
    const globalRoot = spawnSync('npm', ['root', '-g'], { encoding: 'utf8' }).stdout.trim();
    if (globalRoot) candidates.push(path.join(globalRoot, 'playwright'));
  } catch {
    // No npm: only the candidates above.
  }
  for (const candidate of candidates) {
    try {
      return require(candidate);
    } catch {
      // Try the next.
    }
  }
  throw new Error('Playwright is not installed: npm i playwright (and npx playwright install chromium), or set PLAYWRIGHT_MODULE to its folder.');
}

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const DURATION = 70;
const FPS = Number(opt('fps', '60'));
const FROM = Number(opt('from', '0'));
const TO = Number(opt('to', String(DURATION)));
const STILLS = opt('stills', null);
const WORKERS = Number(opt('workers', '3'));
// Headless Chromium keeps memory from every frame whose transforms change scale (the camera), so
// each page is replaced after this many frames; a page then stays under about 2 GB.
const RECYCLE = Number(opt('recycle', '120'));
const WHOLE = FROM === 0 && TO === DURATION && !STILLS;
const BUILD = path.resolve(opt('build', path.join(HERE, 'build')));
const VIDEO_DIR = path.join(ROOT, 'website', 'assets', 'video');
const OUT = path.resolve(opt('out', WHOLE ? path.join(VIDEO_DIR, 'jellyfesh-v0.0.7-public-beta.mp4') : path.join(BUILD, 'part.mp4')));
// 70 s at about 2.55 Mb/s of video and 192 kb/s of music stays under 25 MB.
const VIDEO_KBPS = Number(opt('kbps', '2550'));

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2' };

/** A small static server for the repository (the page also uses website/assets for the brand). */
function serve() {
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      const url = decodeURIComponent(new URL(req.url, 'http://x').pathname);
      const file = path.join(ROOT, path.normalize(url));
      if (!file.startsWith(ROOT + path.sep) || !existsSync(file) || statSync(file).isDirectory()) {
        res.writeHead(404);
        res.end();
        return;
      }
      res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
      createReadStream(file).pipe(res);
    });
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function run(cmd, argv) {
  const r = spawnSync(cmd, argv, { stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} ${argv.join(' ')} failed (${r.status})`);
}

async function openPage(browser, url) {
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  // Closing a page's context ends its renderer and gives back what it held.
  context.setDefaultTimeout(120000);
  const page = await context.newPage();
  page.on('pageerror', (e) => console.error('page error:', e.message));
  page.on('console', (m) => m.type() === 'error' && console.error('console:', m.text()));
  await page.goto(url);
  await page.evaluate(() => window.buildAll());
  return page;
}

// x264: High profile, a keyframe every two seconds, BT.709 tagged; film tuning and a slow preset
// keep gradients and small type clean at this bitrate.
const X264 = (fps) => ['-c:v', 'libx264', '-preset', 'slow', '-tune', 'film', '-pix_fmt', 'yuv420p', '-profile:v', 'high', '-level', '4.2', '-g', String(fps * 2), '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-color_range', 'tv'];
// JPEG frames are full-range BT.601 YCbCr; PNG frames are RGB.
const TO_709_FROM_JPEG = 'scale=in_color_matrix=bt601:in_range=full:out_color_matrix=bt709:out_range=tv';
const TO_709_FROM_RGB = 'scale=out_color_matrix=bt709:out_range=tv';

async function main() {
  mkdirSync(BUILD, { recursive: true });
  const { chromium } = playwright();
  const server = await serve();
  const url = `http://127.0.0.1:${server.address().port}/marketing/promo/scene/index.html`;
  const browser = await chromium.launch();
  const started = Date.now();

  if (STILLS) {
    const page = await openPage(browser, url);
    const dir = path.resolve(opt('dir', path.join(BUILD, 'stills')));
    mkdirSync(dir, { recursive: true });
    for (const t of STILLS.split(',').map(Number)) {
      await page.evaluate((x) => window.renderAt(x), t);
      const file = path.join(dir, `still-${t.toFixed(2).padStart(6, '0')}.png`);
      await page.screenshot({ path: file, type: 'png' });
      console.log(file);
    }
    await browser.close();
    server.close();
    return;
  }

  const frames = Math.round((TO - FROM) * FPS);
  if (!WHOLE) {
    // A part: PNG frames straight into one constant-quality pass.
    let page = await openPage(browser, url);
    const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'png', '-i', '-', ...X264(FPS), '-crf', opt('crf', '18'), '-vf', TO_709_FROM_RGB, '-movflags', '+faststart', OUT], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let i = 0; i < frames; i++) {
      if (i > 0 && i % RECYCLE === 0) {
        await page.context().close();
        page = await openPage(browser, url);
      }
      await page.evaluate((x) => window.renderAt(x), FROM + i / FPS);
      const shot = await page.screenshot({ type: 'png' });
      if (!ff.stdin.write(shot)) await new Promise((r) => ff.stdin.once('drain', r));
      if (i % FPS === 0) process.stdout.write(`\rframe ${i}/${frames}   `);
    }
    ff.stdin.end();
    await new Promise((r) => ff.on('close', r));
    await browser.close();
    server.close();
    console.log(`\n${OUT} in ${((Date.now() - started) / 1000).toFixed(0)} s`);
    return;
  }

  // The whole video: frames from several pages at once, kept as JPEG for the two passes.
  const frameDir = path.join(BUILD, 'frames');
  rmSync(frameDir, { recursive: true, force: true });
  mkdirSync(frameDir, { recursive: true });
  let done = 0;
  await Promise.all(
    Array.from({ length: WORKERS }, async (_, w) => {
      let page = await openPage(browser, url);
      let count = 0;
      for (let i = w; i < frames; i += WORKERS) {
        if (count > 0 && count % RECYCLE === 0) {
          await page.context().close();
          page = await openPage(browser, url);
        }
        count += 1;
        await page.evaluate((x) => window.renderAt(x), i / FPS);
        const shot = await page.screenshot({ type: 'jpeg', quality: 98 });
        await fsp.writeFile(path.join(frameDir, `f${String(i).padStart(5, '0')}.jpg`), shot);
        done += 1;
        if (done % FPS === 0) {
          const eta = ((Date.now() - started) / done) * (frames - done) / 1000;
          process.stdout.write(`\rframes ${done}/${frames}  eta ${eta.toFixed(0)} s   `);
        }
      }
      await page.context().close();
    }),
  );
  await browser.close();
  server.close();
  process.stdout.write(`\nrendered in ${((Date.now() - started) / 1000).toFixed(0)} s\n`);

  const silent = path.join(BUILD, 'video.mp4');
  const log = path.join(BUILD, 'x264-2pass');
  const input = ['-framerate', String(FPS), '-i', path.join(frameDir, 'f%05d.jpg')];
  const rate = ['-b:v', `${VIDEO_KBPS}k`, '-maxrate', `${Math.round(VIDEO_KBPS * 2.4)}k`, '-bufsize', `${VIDEO_KBPS * 4}k`, '-passlogfile', log];
  run('ffmpeg', ['-y', '-loglevel', 'error', ...input, ...X264(FPS), ...rate, '-pass', '1', '-vf', TO_709_FROM_JPEG, '-an', '-f', 'mp4', '/dev/null']);
  run('ffmpeg', ['-y', '-loglevel', 'error', ...input, ...X264(FPS), ...rate, '-pass', '2', '-vf', TO_709_FROM_JPEG, '-movflags', '+faststart', silent]);
  if (!args.includes('--keep-frames')) rmSync(frameDir, { recursive: true, force: true });

  // The music, then the two together.
  const wav = path.join(BUILD, 'music.wav');
  if (!existsSync(wav)) run('python3', [path.join(HERE, 'music', 'make_music.py'), wav]);
  mkdirSync(path.dirname(OUT), { recursive: true });
  run('ffmpeg', ['-y', '-loglevel', 'error', '-i', silent, '-i', wav, '-map', '0:v:0', '-map', '1:a:0', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '192k', '-ar', '48000', '-ac', '2', '-movflags', '+faststart', OUT]);
  // The poster: the end card, with the name and the address.
  run('ffmpeg', ['-y', '-loglevel', 'error', '-ss', opt('poster-at', '6.3'), '-i', OUT, '-frames:v', '1', '-q:v', '2', path.join(path.dirname(OUT), 'poster.jpg')]);
  console.log(`${OUT}: ${(statSync(OUT).size / 1e6).toFixed(2)} MB, ${((Date.now() - started) / 1000).toFixed(0)} s in all`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
