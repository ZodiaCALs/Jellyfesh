// After `vite build`: writes the licences of everything the build put into the app, writes the
// service worker with the app shell's file list and a version that changes with every build's
// contents, and reports how much the first load downloads, compressed (what a server sending gzip
// or Brotli transfers for the first screen).
//
// Usage: node scripts/postbuild.mjs

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync, constants, gzipSync } from 'node:zlib';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(web, 'dist');

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

// THIRD_PARTY_NOTICES.txt (Settings › About links it): the Rust crates in the three WebAssembly
// modules and the plugins, and the npm packages whose code the bundle contains (Svelte's runtime,
// Vite's module loader, jco's generated glue, and jco's transpiler with its WASI shims, which
// install plugins from files), by tools/licenses/generate.py. Plain text, so every server shows it
// rather than offering a download. In CI a failure stops the build.
const repo = join(web, '..', '..');
const python = process.platform === 'win32' ? 'python' : 'python3';
const notices = spawnSync(
  python,
  [
    join(repo, 'tools/licenses/generate.py'),
    '--out',
    join(dist, 'THIRD_PARTY_NOTICES.txt'),
    '--npm',
    'platforms/web:svelte,vite,@bytecodealliance/jco,@bytecodealliance/jco-transpile,@bytecodealliance/preview2-shim',
    'jf-web',
    'jf-web-media',
    'jf-web-sandbox',
    'jf-subsonic',
    'jf-jellyfin',
  ],
  { cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] },
);
if (notices.status === 0) {
  console.log(`postbuild: THIRD_PARTY_NOTICES.txt, ${notices.stdout.trim()}`);
} else {
  console.warn(`postbuild: THIRD_PARTY_NOTICES.txt not written (${python} tools/licenses/generate.py failed)`);
  if (process.env.CI) process.exit(1);
}

const html = readFileSync(join(dist, 'index.html'), 'utf8');
const all = files(dist).map((p) => relative(dist, p).split('\\').join('/'));

// What the first screen loads: the page, what it links, the core's WebAssembly, the plugin list.
const linked = [...html.matchAll(/(?:src|href)="\.\/([^"]+)"/g)].map((m) => m[1]);
const entry = linked.filter((p) => p.startsWith('assets/'));
const coreWasm = all.filter((p) => /^assets\/jf_web_bg-[\w-]+\.wasm$/.test(p));
if (coreWasm.length !== 1) throw new Error(`postbuild: expected one core wasm, found ${coreWasm.join(', ') || 'none'}`);
const firstLoad = [
  'index.html',
  ...new Set([...entry, ...coreWasm, 'manifest.webmanifest', 'icons/icon.svg', 'plugins/index.json']),
].filter((p) => all.includes(p));

// The app shell kept for starting offline.
const shell = [
  './',
  ...new Set([...firstLoad.filter((p) => p !== 'index.html'), 'icons/apple-touch-icon.png']),
].map((p) => (p === './' ? p : `./${p}`));

const hash = createHash('sha256');
for (const path of all.sort()) {
  if (path === 'sw.js') continue;
  hash.update(path);
  hash.update(readFileSync(join(dist, path)));
}
const version = hash.digest('hex').slice(0, 16);
const template = readFileSync(join(web, 'scripts/sw.template.js'), 'utf8');
writeFileSync(
  join(dist, 'sw.js'),
  template.replace("'__VERSION__'", JSON.stringify(version)).replace('__SHELL__', JSON.stringify(shell, null, 2)),
);

// Glass blurs only where browsers read the standard backdrop-filter. Given it next to
// -webkit-backdrop-filter, the CSS minifier keeps just the prefixed one (which Chrome and Firefox
// ignore), so the sources write only the standard property and the minifier adds the prefix.
for (const path of all.filter((p) => p.endsWith('.css'))) {
  const css = readFileSync(join(dist, path), 'utf8');
  const prefixed = css.match(/-webkit-backdrop-filter:/g)?.length ?? 0;
  const standard = css.match(/(?<![\w-])backdrop-filter:/g)?.length ?? 0;
  if (prefixed > standard) {
    throw new Error(
      `postbuild: ${path} has ${prefixed} -webkit-backdrop-filter but only ${standard} backdrop-filter declarations; ` +
        'write only backdrop-filter in the sources',
    );
  }
}

if (!existsSync(join(dist, 'plugins/index.json'))) {
  console.warn('postbuild: dist/plugins/index.json is missing: this build has no source plugins (npm run plugins).');
}

let raw = 0;
let gzip = 0;
let brotli = 0;
const rows = [];
for (const path of firstLoad) {
  const bytes = readFileSync(join(dist, path));
  const g = gzipSync(bytes, { level: 9 }).length;
  const b = brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length;
  raw += bytes.length;
  gzip += g;
  brotli += b;
  rows.push(`  ${path.padEnd(44)} ${String(bytes.length).padStart(9)} ${String(g).padStart(9)} ${String(b).padStart(9)}`);
}
console.log(`postbuild: sw.js version ${version}, app shell of ${shell.length} entries`);
console.log(`postbuild: first load (bytes)                       raw      gzip    brotli`);
console.log(rows.join('\n'));
console.log(
  `  ${'total'.padEnd(44)} ${String(raw).padStart(9)} ${String(gzip).padStart(9)} ${String(brotli).padStart(9)}` +
    `  (${(gzip / 1024).toFixed(0)} KiB gzip, ${(brotli / 1024).toFixed(0)} KiB Brotli)`,
);
