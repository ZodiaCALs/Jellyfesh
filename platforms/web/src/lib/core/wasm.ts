// Loads jf-web, the page's core (core/jf-web, built by scripts/build-wasm.mjs), once.

import init, * as jf from '../../../generated/wasm/jf_web.js';
import coreUrl from '../../../generated/wasm/jf_web_bg.wasm?url';

export type Jf = typeof jf;

let loading: Promise<Jf> | undefined;

/** The core module, initialised. */
export function loadCore(): Promise<Jf> {
  loading ??= init({ module_or_path: coreUrl }).then(() => jf);
  return loading;
}

/** Parses JSON a core function returned. */
export function parse<T>(json: string): T {
  return JSON.parse(json) as T;
}
