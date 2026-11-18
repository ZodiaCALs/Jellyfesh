// Messages between the page (./host.ts) and a plugin's worker (./worker.ts). Values are
// structured-cloned: jco's shapes (including bigint and Uint8Array) pass through unchanged.

import type { AllowRule, PackageInfo } from '../core/types';
import type { RelayConfig, RelayErrorCode } from './relay';

export type ToWorker =
  | {
      type: 'init';
      plugin: string;
      /** Absolute URLs of the package and of the transpiled component's JavaScript. */
      packageUrl: string;
      moduleUrl: string;
      /** The component's core WebAssembly files by name, when they are not beside the module. */
      coreUrls: Record<string, string> | null;
      /** The SHA-256 of the component the JavaScript was transpiled from (plugins/index.json). */
      wasmSha256: string;
      /** The settings as JSON. */
      settings: string;
      kv: [string, Uint8Array][];
      secrets: [string, string][];
      /** Time allowed for instantiation and the first calls, ms. */
      startMs: number;
      /** Run the plugin; false only reads and verifies its package. */
      instantiate: boolean;
      /** The relay its requests go through (a source that refuses browsers), or null. */
      relay: RelayConfig | null;
      /** Its cookie jar (jf_web_sandbox.CookieJar JSON), for requests through the relay. */
      cookies: string | null;
    }
  | {
      type: 'call';
      seq: number;
      /** `interface.function` in jco's names, e.g. `catalog.search`. */
      method: string;
      args: unknown[];
      /** The call's wall-clock budget, ms. */
      wallMs: number;
      /** Values typed into a login form being submitted. */
      protect: string[];
      keystore: boolean;
    }
  | { type: 'settings'; settings: string; allow: AllowRule[]; maxRps: number }
  | { type: 'relay'; relay: RelayConfig | null };

export type CallFailure = { kind: 'plugin'; payload: { tag: string; val?: unknown } } | { kind: 'trap'; message: string };

export type FromWorker =
  | { type: 'ready'; info: PackageInfo; capabilities: string[]; memory: number }
  | { type: 'fatal'; message: string }
  | { type: 'result'; seq: number; ok: true; value: unknown; memory: number }
  | { type: 'result'; seq: number; ok: false; error: CallFailure; memory: number }
  | { type: 'kv'; key: string; value: Uint8Array | null }
  | { type: 'secret'; name: string; value: string | null }
  | { type: 'log'; level: string; message: string }
  | { type: 'network-error'; url: string; kind: 'mixed-content' | 'network' | 'relay'; reason?: RelayErrorCode }
  /** The cookie jar changed: all of it (for the next worker of this visit), and what outlives the visit. */
  | { type: 'cookies'; all: string; persistent: string };
