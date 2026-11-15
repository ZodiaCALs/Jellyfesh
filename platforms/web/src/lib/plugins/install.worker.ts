// Prepares a plugin package someone installs from a file (Sources → Install from File…): reads and
// checks it with jf-web-sandbox (the native host's package and signature checks) and transpiles its
// component with jco for the browser's own WebAssembly engine, with the options
// scripts/build-plugins.mjs gives the bundled plugins at build time (not minified: the minifier
// is a native module). A worker of its own, loaded only for an install: the transpiler is 4 MB of
// WebAssembly and takes a moment.

import { generate } from '@bytecodealliance/jco/component';
import initSandbox, * as sandbox from '../../../generated/wasm/jf_web_sandbox.js';
import sandboxUrl from '../../../generated/wasm/jf_web_sandbox_bg.wasm?url';
import type { PackageInfo } from '../core/types';

export interface InstallRequest {
  id: number;
  bytes: Uint8Array;
}

export type InstallReply =
  | { id: number; ok: true; info: PackageInfo; module: Uint8Array; cores: [string, Uint8Array][] }
  | { id: number; ok: false; message: string };

const scope = self as unknown as {
  postMessage(message: InstallReply, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<InstallRequest>) => void) | null;
};

const ready = initSandbox({ module_or_path: sandboxUrl });

async function prepare(request: InstallRequest) {
  await ready;
  let pkg: sandbox.Package;
  try {
    pkg = new sandbox.Package(request.bytes);
  } catch (error) {
    throw new Error(`This is not a Jellyfesh plugin package: ${error instanceof Error ? error.message : String(error)}`);
  }
  const info = JSON.parse(pkg.info()) as PackageInfo;
  const component = pkg.component();
  pkg.free();
  const transpiled = await generate(component, {
    name: 'plugin',
    map: [],
    instantiation: { tag: 'async' },
    noTypescript: true,
    noComponentErrorWrapping: true,
    base64Cutoff: 5000,
    emitConditionalSuspendingWasm: true,
  });
  let module: Uint8Array | null = null;
  const cores: [string, Uint8Array][] = [];
  for (const [name, bytes] of transpiled.files) {
    if (name === 'plugin.js') module = bytes;
    else if (name.endsWith('.wasm')) cores.push([name, bytes]);
  }
  if (!module) throw new Error('The plugin could not be prepared for this browser (no module came out).');
  scope.postMessage({ id: request.id, ok: true, info, module, cores }, [
    module.buffer as ArrayBuffer,
    ...cores.map(([, bytes]) => bytes.buffer as ArrayBuffer),
  ]);
}

scope.onmessage = (event) => {
  const request = event.data;
  prepare(request).catch((error) =>
    scope.postMessage({ id: request.id, ok: false, message: error instanceof Error ? error.message : String(error) }),
  );
};
