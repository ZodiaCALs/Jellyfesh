// The plugin host imports with the real Rust rules (jf-web-sandbox, built by `npm run wasm`) and
// a fake network: what a plugin may reach, store and log in the browser.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import * as sandbox from '../../generated/wasm/jf_web_sandbox.js';
import {
  type CallContext,
  createImports,
  type HostDeps,
  KV_QUOTA,
  LOG_BURST,
  LogLimiter,
  sanitizeLog,
  TransportError,
  type TransportRequest,
  type TransportResponse,
} from '../../src/lib/plugins/imports';

const SERVER = 'http://192.168.1.20:4533';

beforeAll(() => {
  const wasm = fileURLToPath(new URL('../../generated/wasm/jf_web_sandbox_bg.wasm', import.meta.url));
  sandbox.initSync({ module: readFileSync(wasm) });
});

interface Fake {
  deps: HostDeps;
  sent: TransportRequest[];
  stored: [string, Uint8Array | null][];
  secrets: [string, string | null][];
  logs: [string, string][];
  network: [string, string][];
  clock: { now: number };
  answer: (request: TransportRequest) => TransportResponse;
}

function fake(options: { https?: boolean; maxRps?: number; protect?: string[]; keystore?: boolean } = {}): Fake {
  const rules = new sandbox.Sandbox(JSON.stringify([{ Setting: 'server_url' }]), JSON.stringify({ server_url: SERVER }), options.maxRps ?? 20);
  const clock = { now: 1000 };
  const call: CallContext = { deadline: clock.now + 10_000, protect: options.protect ?? [], keystore: options.keystore ?? false, received: 0 };
  const f: Fake = {
    sent: [],
    stored: [],
    secrets: [],
    logs: [],
    network: [],
    clock,
    answer: (request) => ({ status: 200, headers: [['Content-Type', 'application/json']], body: new TextEncoder().encode('{}'), url: request.url }),
    deps: undefined as unknown as HostDeps,
  };
  f.deps = {
    rules: {
      check_request: (head) => rules.check_request(head),
      access: (url) => rules.access(url),
      take_token: (now) => rules.take_token(now),
    },
    crypto: sandbox,
    http: {
      send(request) {
        f.sent.push(request);
        return f.answer(request);
      },
    },
    pageIsHttps: options.https ?? false,
    kv: new Map(),
    secrets: new Map(),
    onKv: (key, value) => f.stored.push([key, value]),
    onSecret: (name, value) => f.secrets.push([name, value]),
    log: (level, message) => f.logs.push([level, message]),
    onNetworkError: (url, kind) => f.network.push([url, kind]),
    now: () => clock.now,
    wallNow: () => 1_790_000_000_000,
    random: (n) => new Uint8Array(n).fill(7),
    sleepSync: (ms) => (clock.now += ms),
    call,
  };
  return f;
}

/** Runs an import and returns its jco error payload, or the value. */
function attempt<T>(run: () => T): { ok: true; value: T } | { ok: false; error: { tag: string; val?: string } } {
  try {
    return { ok: true, value: run() };
  } catch (thrown) {
    return { ok: false, error: (thrown as { payload: { tag: string; val?: string } }).payload };
  }
}

function request(url: string, extra: Partial<{ method: string; headers: { name: string; value: string }[]; body: Uint8Array }> = {}) {
  return { method: 'get', url, headers: [], body: new Uint8Array(), followRedirects: true, ...extra };
}

function http(f: Fake) {
  return createImports(f.deps)['jellyfesh:plugin/http'] as { send(r: ReturnType<typeof request>): { status: number; url: string } };
}

function kv(f: Fake) {
  return createImports(f.deps)['jellyfesh:plugin/kv'] as Record<string, (...args: any[]) => any>;
}

describe('http', () => {
  it('reaches the configured server', () => {
    const f = fake();
    const result = attempt(() => http(f).send(request(`${SERVER}/rest/ping.view?f=json`)));
    expect(result.ok).toBe(true);
    expect(f.sent).toHaveLength(1);
    expect(f.sent[0].url).toBe(`${SERVER}/rest/ping.view?f=json`);
  });

  it('refuses every other host', () => {
    const f = fake();
    const result = attempt(() => http(f).send(request('https://tracker.example/collect')));
    expect(result).toMatchObject({ ok: false, error: { tag: 'not-allowed' } });
    expect(f.sent).toHaveLength(0);
  });

  it('refuses a redirect that ends somewhere else', () => {
    const f = fake();
    f.answer = (r) => ({ status: 200, headers: [], body: new Uint8Array(), url: 'https://elsewhere.example/x' });
    expect(attempt(() => http(f).send(request(`${SERVER}/a`)))).toMatchObject({ ok: false, error: { tag: 'not-allowed' } });
  });

  it('reports mixed content from an https page, but allows loopback', () => {
    const f = fake({ https: true });
    expect(attempt(() => http(f).send(request(`${SERVER}/rest/ping.view`)))).toMatchObject({ ok: false, error: { tag: 'connection' } });
    expect(f.network).toEqual([[`${SERVER}/rest/ping.view`, 'mixed-content']]);
    expect(f.sent).toHaveLength(0);
  });

  it('drops headers the browser sets itself', () => {
    const f = fake();
    http(f).send(
      request(`${SERVER}/x`, {
        headers: [
          { name: 'Authorization', value: 'MediaBrowser Token="t"' },
          { name: 'User-Agent', value: 'plugin' },
          { name: 'Sec-Fetch-Mode', value: 'cors' },
        ],
      }),
    );
    expect(f.sent[0].headers).toEqual([['Authorization', 'MediaBrowser Token="t"']]);
  });

  it('turns transport failures into WIT errors and reports unreachable servers', () => {
    const f = fake();
    f.answer = () => {
      throw new TransportError('timeout', 'timed out');
    };
    expect(attempt(() => http(f).send(request(`${SERVER}/slow`)))).toMatchObject({ ok: false, error: { tag: 'timeout' } });
    f.answer = () => {
      throw new TransportError('network', 'no answer, or CORS');
    };
    expect(attempt(() => http(f).send(request(`${SERVER}/gone?t=secret`)))).toMatchObject({ ok: false, error: { tag: 'connection' } });
    expect(f.network.at(-1)).toEqual([`${SERVER}/gone`, 'network']);
  });

  it('limits a response to 16 MiB', () => {
    const f = fake();
    f.answer = (r) => ({ status: 200, headers: [], body: new Uint8Array(16 * 1024 * 1024 + 1), url: r.url });
    expect(attempt(() => http(f).send(request(`${SERVER}/big`)))).toMatchObject({ ok: false, error: { tag: 'too-large' } });
  });

  it('waits for the rate limit within the call, then gives up', () => {
    const f = fake({ maxRps: 1 });
    const imports = http(f);
    imports.send(request(`${SERVER}/1`));
    const before = f.clock.now;
    imports.send(request(`${SERVER}/2`));
    expect(f.clock.now).toBeGreaterThan(before);
    f.deps.call.deadline = f.clock.now + 10;
    expect(attempt(() => imports.send(request(`${SERVER}/3`)))).toMatchObject({ ok: false, error: { tag: 'rate-limited' } });
  });
});

describe('kv', () => {
  it('stores plain values and hands them to the page', () => {
    const f = fake();
    const store = kv(f);
    store.set('session', new TextEncoder().encode('abc'));
    expect(new TextDecoder().decode(store.get('session'))).toBe('abc');
    expect(f.stored.map(([k]) => k)).toEqual(['session']);
    store.delete('session');
    expect(store.get('session')).toBeUndefined();
    expect(f.stored.at(-1)).toEqual(['session', null]);
  });

  it('checks keys and the 1 MiB quota', () => {
    const f = fake();
    const store = kv(f);
    expect(attempt(() => store.set('', new Uint8Array(1)))).toMatchObject({ ok: false, error: { tag: 'invalid-key' } });
    expect(attempt(() => store.set('tab\tkey', new Uint8Array(1)))).toMatchObject({ ok: false, error: { tag: 'invalid-key' } });
    store.set('big', new Uint8Array(KV_QUOTA - 10));
    expect(attempt(() => store.set('more', new Uint8Array(100)))).toMatchObject({ ok: false, error: { tag: 'quota-exceeded' } });
  });

  it('refuses to store a password typed for the login in progress', () => {
    const f = fake({ protect: ['hunter2-password'], keystore: true });
    const store = kv(f);
    expect(attempt(() => store.set('creds', new TextEncoder().encode('user:hunter2-password')))).toMatchObject({
      ok: false,
      error: { tag: 'forbidden' },
    });
    expect(attempt(() => store.setSecret('token', 'hunter2-password'))).toMatchObject({ ok: false, error: { tag: 'forbidden' } });
    store.setSecret('token', 'a-derived-token');
    expect(f.secrets).toEqual([['token', 'a-derived-token']]);
  });

  it('checks secret names', () => {
    const f = fake();
    expect(attempt(() => kv(f).setSecret('../escape', 'x'))).toMatchObject({ ok: false, error: { tag: 'invalid-key' } });
    expect(attempt(() => kv(f).setSecret('Upper', 'x'))).toMatchObject({ ok: false, error: { tag: 'invalid-key' } });
  });
});

describe('crypto and clock', () => {
  it('computes digests, MACs and AES in Rust', () => {
    const imports = createImports(fake().deps)['jellyfesh:plugin/crypto'] as Record<string, (...args: any[]) => any>;
    const hex = (bytes: Uint8Array) => Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    const abc = new TextEncoder().encode('abc');
    expect(hex(imports.digest('md5', abc))).toBe('900150983cd24fb0d6963f7d28e17f72');
    expect(hex(imports.digest('sha256', abc))).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    const key = new Uint8Array(16).fill(1);
    const mode = { tag: 'gcm', val: { nonce: new Uint8Array(12).fill(2), aad: new Uint8Array() } };
    const sealed = imports.aesEncrypt(mode, key, abc);
    expect(new TextDecoder().decode(imports.aesDecrypt(mode, key, sealed))).toBe('abc');
    expect(attempt(() => imports.randomBytes(1024 * 1024))).toMatchObject({ ok: false, error: { tag: 'too-large' } });
    expect(imports.randomBytes(4)).toHaveLength(4);
  });

  it('gives times as 64-bit integers', () => {
    const clock = createImports(fake().deps)['jellyfesh:plugin/clock'] as Record<string, () => bigint>;
    expect(clock.monotonicMs()).toBe(1000n);
    expect(clock.nowUtcMs()).toBe(1_790_000_000_000n);
  });
});

describe('log', () => {
  it('masks typed passwords and escapes control characters', () => {
    expect(sanitizeLog('login demo/hunter2-password\nok', ['hunter2-password'])).toBe('login demo/***\\nok');
  });

  it('allows bursts of 100 lines, then 20 a second, and counts what it dropped', () => {
    const limiter = new LogLimiter();
    for (let i = 0; i < LOG_BURST; i++) expect(limiter.admit(0)).toBe(0);
    expect(limiter.admit(0)).toBeNull();
    expect(limiter.admit(0)).toBeNull();
    expect(limiter.admit(100)).toBe(2);
  });

  it('goes through the import with the protected values masked', () => {
    const f = fake({ protect: ['hunter2-password'] });
    (createImports(f.deps)['jellyfesh:plugin/log'] as { log(level: string, m: string): void }).log('info', 'pw=hunter2-password');
    expect(f.logs).toEqual([['info', 'pw=***']]);
  });
});
