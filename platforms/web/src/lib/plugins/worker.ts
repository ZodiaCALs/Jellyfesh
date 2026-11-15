// A plugin's Web Worker: one per plugin, so a plugin never shares memory or a thread with the
// page or another plugin. It reads and verifies the plugin's package (jf-web-sandbox: the same
// package and signature checks as the native host), checks that the transpiled component came
// from that package, instantiates it with the host imports (./imports.ts) and runs calls one at a
// time. The page enforces the time limits by terminating the worker (./host.ts).

import initSandbox, * as sandbox from '../../../generated/wasm/jf_web_sandbox.js';
import sandboxUrl from '../../../generated/wasm/jf_web_sandbox_bg.wasm?url';
import { type CallContext, createImports, type HttpTransport, TransportError, type TransportResponse } from './imports';
import type { FromWorker, ToWorker } from './protocol';

/* eslint-disable @typescript-eslint/no-explicit-any */
const scope = self as unknown as {
  postMessage(message: FromWorker, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null;
  location: Location;
};

let instance: any = null;
let rules: sandbox.Sandbox | null = null;
const memories: WebAssembly.Memory[] = [];
const call: CallContext = { deadline: Number.POSITIVE_INFINITY, protect: [], keystore: false, received: 0 };
const kv = new Map<string, Uint8Array>();
const secrets = new Map<string, string>();

function post(message: FromWorker) {
  scope.postMessage(message);
}

/** A synchronous request: allowed in dedicated workers, and what synchronous WIT imports need. */
const transport: HttpTransport = {
  send(request): TransportResponse {
    const xhr = new XMLHttpRequest();
    xhr.open(request.method, request.url, false);
    xhr.responseType = 'arraybuffer';
    xhr.timeout = request.timeoutMs;
    xhr.withCredentials = false;
    for (const [name, value] of request.headers) xhr.setRequestHeader(name, value);
    try {
      xhr.send(request.body as XMLHttpRequestBodyInit | null);
    } catch (error) {
      if ((error as DOMException).name === 'TimeoutError') throw new TransportError('timeout', 'timed out');
      throw new TransportError(
        'network',
        `cannot reach ${new URL(request.url).origin}: the server did not answer, or does not allow this page (CORS)`,
      );
    }
    if (xhr.status === 0) {
      throw new TransportError('network', `cannot reach ${new URL(request.url).origin} (no answer, or CORS)`);
    }
    const headers: [string, string][] = xhr
      .getAllResponseHeaders()
      .trim()
      .split(/[\r\n]+/)
      .filter(Boolean)
      .map((line) => {
        const at = line.indexOf(':');
        return [line.slice(0, at).trim(), line.slice(at + 1).trim()];
      });
    const body = xhr.response instanceof ArrayBuffer ? new Uint8Array(xhr.response) : new Uint8Array();
    return { status: xhr.status, headers, body, url: xhr.responseURL };
  },
};

function sleepSync(ms: number) {
  const until = performance.now() + ms;
  while (performance.now() < until) {
    // A plugin call is synchronous; nothing else runs in this worker meanwhile.
  }
}

function random(length: number): Uint8Array {
  const out = new Uint8Array(length);
  for (let i = 0; i < length; i += 65536) crypto.getRandomValues(out.subarray(i, Math.min(length, i + 65536)));
  return out;
}

async function init(message: Extract<ToWorker, { type: 'init' }>) {
  await initSandbox({ module_or_path: sandboxUrl });
  const response = await fetch(message.packageUrl);
  if (!response.ok) throw new Error(`cannot load ${message.packageUrl}: HTTP ${response.status}`);
  const pkg = new sandbox.Package(new Uint8Array(await response.arrayBuffer()));
  const info = JSON.parse(pkg.info());
  pkg.free();
  if (info.id !== message.plugin) throw new Error(`the package holds ${info.id}, not ${message.plugin}`);
  if (info.wasm_sha256 !== message.wasmSha256) {
    throw new Error('the transpiled plugin was not made from this package; rebuild the app');
  }
  if (!message.instantiate) {
    post({ type: 'ready', info, capabilities: [], memory: 0 });
    return;
  }
  for (const [key, value] of message.kv) kv.set(key, value);
  for (const [name, value] of message.secrets) secrets.set(name, value);
  rules = new sandbox.Sandbox(JSON.stringify(info.allow), message.settings, info.max_rps);
  const imports = createImports({
    rules: {
      check_request: (head) => rules!.check_request(head),
      access: (url) => rules!.access(url),
      take_token: (now) => rules!.take_token(now),
    },
    crypto: sandbox,
    http: transport,
    pageIsHttps: scope.location.protocol === 'https:',
    kv,
    secrets,
    onKv: (key, value) => post({ type: 'kv', key, value }),
    onSecret: (name, value) => post({ type: 'secret', name, value }),
    log: (level, text) => post({ type: 'log', level, message: text }),
    onNetworkError: (url, kind) => post({ type: 'network-error', url, kind }),
    now: () => performance.now(),
    wallNow: () => Date.now(),
    random,
    sleepSync,
    call,
  });
  const module = await import(/* @vite-ignore */ message.moduleUrl);
  const getCoreModule = async (path: string) => {
    const url = message.coreUrls?.[path] ?? new URL(path, message.moduleUrl);
    const bytes = await (await fetch(url)).arrayBuffer();
    return WebAssembly.compile(bytes);
  };
  const instantiateCore = async (core: WebAssembly.Module, coreImports: WebAssembly.Imports) => {
    const created = await WebAssembly.instantiate(core, coreImports);
    for (const value of Object.values(created.exports)) {
      if (value instanceof WebAssembly.Memory) memories.push(value);
    }
    return created;
  };
  call.deadline = performance.now() + message.startMs;
  instance = await module.instantiate(getCoreModule, imports, instantiateCore);
  try {
    instance.meta.settingsChanged(message.settings);
  } catch (error) {
    post({ type: 'log', level: 'warn', message: `the plugin did not accept its settings: ${describe(error)}` });
  }
  const capabilities: string[] = instance.meta.capabilities();
  call.deadline = Number.POSITIVE_INFINITY;
  post({ type: 'ready', info, capabilities, memory: memoryBytes() });
}

function memoryBytes(): number {
  return memories.reduce((sum, memory) => sum + memory.buffer.byteLength, 0);
}

function describe(error: unknown): string {
  if (error && typeof error === 'object' && 'payload' in error) return JSON.stringify((error as any).payload);
  return error instanceof Error ? error.message : String(error);
}

function run(message: Extract<ToWorker, { type: 'call' }>) {
  const [iface, name] = message.method.split('.');
  call.deadline = performance.now() + message.wallMs;
  call.protect = message.protect;
  call.keystore = message.keystore;
  call.received = 0;
  try {
    const value = instance[iface][name](...message.args);
    post({ type: 'result', seq: message.seq, ok: true, value, memory: memoryBytes() });
  } catch (error) {
    if (error && typeof error === 'object' && 'payload' in error) {
      post({ type: 'result', seq: message.seq, ok: false, error: { kind: 'plugin', payload: (error as any).payload }, memory: memoryBytes() });
    } else {
      // A trap (panic, unreachable, bad access): the instance cannot be used again.
      const text = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      post({ type: 'result', seq: message.seq, ok: false, error: { kind: 'trap', message: text }, memory: memoryBytes() });
    }
  } finally {
    call.protect = [];
    call.keystore = false;
    call.deadline = Number.POSITIVE_INFINITY;
  }
}

scope.onmessage = (event) => {
  const message = event.data;
  switch (message.type) {
    case 'init':
      init(message).catch((error) => post({ type: 'fatal', message: error instanceof Error ? error.message : String(error) }));
      break;
    case 'call':
      if (!instance) {
        post({ type: 'result', seq: message.seq, ok: false, error: { kind: 'trap', message: 'not started' }, memory: 0 });
      } else {
        run(message);
      }
      break;
    case 'settings':
      if (instance && rules) {
        const old = rules;
        rules = new sandbox.Sandbox(JSON.stringify(message.allow), message.settings, message.maxRps);
        old.free();
        try {
          instance.meta.settingsChanged(message.settings);
        } catch (error) {
          post({ type: 'log', level: 'warn', message: `the plugin did not accept its new settings: ${describe(error)}` });
        }
      }
      break;
  }
};
