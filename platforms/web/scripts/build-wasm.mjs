// Builds Jellyfesh Web's Rust side for the browser:
//
//   core/jf-web          -> generated/wasm/jf_web.js + jf_web_bg.wasm           (the page)
//   core/jf-web/media    -> generated/wasm/jf_web_media.js + _bg.wasm            (media worker)
//   core/jf-web/sandbox  -> generated/wasm/jf_web_sandbox.js + _bg.wasm          (plugin workers)
//
// cargo builds them for wasm32-unknown-unknown with the `web` profile, wasm-bindgen (the CLI must
// be the exact version of the wasm-bindgen crate in Cargo.lock) writes the JavaScript bindings, and
// binaryen's wasm-opt (from jco's dependencies) strips names and debug sections. Its optimisation
// passes are left out on purpose: LLVM already optimised for size, and they make the compressed
// download larger.
//
// Needs Rust with the wasm32-unknown-unknown target and `cargo install wasm-bindgen-cli --version
// <the Cargo.lock version> --locked` (`node scripts/build-wasm.mjs --bindgen-version` prints it).
// Honours CARGO_TARGET_DIR.

import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const web = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(web, '../..');
const out = join(web, 'generated/wasm');
const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(root, 'target');
const crates = [
  ['jf-web', 'jf_web'],
  ['jf-web-media', 'jf_web_media'],
  ['jf-web-sandbox', 'jf_web_sandbox'],
];

function run(command, args, options = {}) {
  execFileSync(command, args, { stdio: 'inherit', cwd: root, ...options });
}

function lockedVersion(name) {
  const lock = readFileSync(join(root, 'Cargo.lock'), 'utf8');
  const match = lock.match(new RegExp(`name = "${name}"\\nversion = "([^"]+)"`));
  if (!match) throw new Error(`${name} is not in Cargo.lock`);
  return match[1];
}

const wanted = lockedVersion('wasm-bindgen');
if (process.argv.includes('--bindgen-version')) {
  // For CI: cargo install wasm-bindgen-cli --locked --version "$(node scripts/build-wasm.mjs --bindgen-version)"
  console.log(wanted);
  process.exit(0);
}
let installed = '';
try {
  installed = execFileSync('wasm-bindgen', ['--version'], { encoding: 'utf8' }).trim().split(' ').pop();
} catch {
  // reported below
}
if (installed !== wanted) {
  console.error(
    `build-wasm: wasm-bindgen ${wanted} is needed (found ${installed || 'none'}).\n` +
      `  cargo install wasm-bindgen-cli --version ${wanted} --locked`,
  );
  process.exit(1);
}

const packages = crates.flatMap(([crate]) => ['-p', crate]);
run('cargo', ['build', '--locked', '--target', 'wasm32-unknown-unknown', '--profile', 'web', ...packages], {
  env: { ...process.env, CARGO_INCREMENTAL: '0' },
});

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const wasmOpt = join(web, 'node_modules/binaryen/bin/wasm-opt');
for (const [, name] of crates) {
  const input = join(target, 'wasm32-unknown-unknown/web', `${name}.wasm`);
  run('wasm-bindgen', ['--target', 'web', '--out-dir', out, '--out-name', name, input]);
  const bound = join(out, `${name}_bg.wasm`);
  run('node', [wasmOpt, '--strip-debug', '--strip-producers', '--strip-target-features', bound, '-o', bound]);
}
console.log(`build-wasm: wrote ${out}`);
