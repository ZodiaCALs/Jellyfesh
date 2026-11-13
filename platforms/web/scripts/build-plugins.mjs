// Prepares the official plugins for Jellyfesh Web:
//
//   dist/plugins/<id>.jfpkg (from plugins/release.sh; unsigned is fine in development)
//     -> public/plugins/<id>.jfpkg            the package itself; each plugin worker reads and
//                                             verifies it at run time (core/jf-web/sandbox)
//     -> public/plugins/<id>/plugin.js + *.core.wasm
//                                             its component, transpiled by jco for the browser's
//                                             own WebAssembly engine (instantiated by the worker)
//     -> public/plugins/index.json            what is bundled: id, name, version, files, and the
//                                             SHA-256 of the component the JavaScript came from
//
// The packages are read with the same Rust code as at run time (jf-web-sandbox, built by
// scripts/build-wasm.mjs), so a package that would not load fails the build. Set
// JF_PLUGIN_PACKAGES to read them from another folder.

import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const web = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(web, '../..');
const source = resolve(process.env.JF_PLUGIN_PACKAGES ?? join(root, 'dist/plugins'));
const out = join(web, 'public/plugins');
const official = ['org.jellyfesh.subsonic', 'org.jellyfesh.jellyfin'];

const sandboxJs = join(web, 'generated/wasm/jf_web_sandbox.js');
if (!existsSync(sandboxJs)) {
  console.error('build-plugins: run `npm run wasm` first (generated/wasm is missing)');
  process.exit(1);
}
const sandbox = await import(sandboxJs);
await sandbox.default({ module_or_path: readFileSync(join(web, 'generated/wasm/jf_web_sandbox_bg.wasm')) });

const packages = official.map((id) => join(source, `${id}.jfpkg`));
const missing = packages.filter((p) => !existsSync(p));
if (missing.length) {
  console.error(
    `build-plugins: missing ${missing.join(', ')}.\n  Build the packages first: plugins/release.sh (from the repository root)`,
  );
  process.exit(1);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const scratch = mkdtempSync(join(tmpdir(), 'jf-web-plugins-'));
const jco = join(web, 'node_modules/.bin/jco');
const index = [];
try {
  for (const path of packages) {
    const bytes = readFileSync(path);
    const pkg = new sandbox.Package(bytes);
    const info = JSON.parse(pkg.info());
    const component = join(scratch, `${info.id}.wasm`);
    writeFileSync(component, pkg.component());
    pkg.free();
    const dir = join(out, info.id);
    execFileSync(
      jco,
      [
        'transpile',
        component,
        '--out-dir',
        dir,
        '--name',
        'plugin',
        '--instantiation',
        'async',
        '--no-wasi-shim',
        '--no-typescript',
        '--minify',
        '--quiet',
      ],
      { stdio: 'inherit' },
    );
    // Type declarations are not served.
    rmSync(join(dir, 'interfaces'), { recursive: true, force: true });
    rmSync(join(dir, 'plugin.d.ts'), { force: true });
    copyFileSync(path, join(out, `${info.id}.jfpkg`));
    index.push({
      id: info.id,
      name: info.name,
      version: info.version,
      official: info.official,
      // For the views before a worker has verified the package; calls use the verified manifest.
      description: info.description,
      capabilities: info.capabilities,
      auth_kinds: info.auth_kinds,
      package: `${info.id}.jfpkg`,
      module: `${info.id}/plugin.js`,
      wasm_sha256: info.wasm_sha256,
      core_modules: readdirSync(dir).filter((f) => f.endsWith('.wasm')),
    });
    console.log(`build-plugins: ${info.id} ${info.version} (${info.official ? 'official' : 'unsigned or community'})`);
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
writeFileSync(join(out, 'index.json'), `${JSON.stringify({ plugins: index }, null, 2)}\n`);
console.log(`build-plugins: wrote ${out}`);
