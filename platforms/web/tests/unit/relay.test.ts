// Requests through a relay the person runs (tools/jf-relay, docs/web-relay.md), with the real Rust
// rules (jf-web-sandbox and jf-web, built by `npm run wasm`) and a fake relay: what the plugin's
// requests carry, how answers, cookies and redirects come back, and which sources need a relay.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import * as jf from '../../generated/wasm/jf_web.js';
import * as sandbox from '../../generated/wasm/jf_web_sandbox.js';
import { type CallContext, createImports, type HostDeps, type TransportRequest, type TransportResponse } from '../../src/lib/plugins/imports';
import { asciiJson, type RelayConfig, RelayError, unwrap, wrap } from '../../src/lib/plugins/relay';

const RELAY: RelayConfig = { address: 'http://127.0.0.1:8790', token: 'a-token-of-sixteen-chars' };
const API = 'https://u.y.qq.com/cgi-bin/musics.fcg';
const NOW = 1_790_000_000_000;

beforeAll(() => {
  const dir = (name: string) => fileURLToPath(new URL(`../../generated/wasm/${name}`, import.meta.url));
  sandbox.initSync({ module: readFileSync(dir('jf_web_sandbox_bg.wasm')) });
  jf.initSync({ module: readFileSync(dir('jf_web_bg.wasm')) });
});

/** What the fake relay answers: the service's status, headers and body. */
type Service = (url: string, method: string, headers: [string, string][]) => { status: number; headers: [string, string][]; body?: string };

interface Fake {
  deps: HostDeps;
  sent: TransportRequest[];
  network: unknown[][];
  jarChanges: number;
  service: Service;
  relayAnswer?: (request: TransportRequest) => TransportResponse;
}

function fake(relay: RelayConfig | null = RELAY): Fake {
  const allow = [
    { Host: { host: 'u.y.qq.com', wildcard: false, port: 443 } },
    { Host: { host: 'open.weixin.qq.com', wildcard: false, port: 443 } },
  ];
  const rules = new sandbox.Sandbox(JSON.stringify(allow), '{}', 5);
  const jar = new sandbox.CookieJar(undefined);
  const clock = { now: 1000 };
  const call: CallContext = { deadline: clock.now + 10_000, protect: [], keystore: false, received: 0 };
  const f: Fake = {
    sent: [],
    network: [],
    jarChanges: 0,
    service: () => ({ status: 200, headers: [['content-type', 'application/json']], body: '{"code":0}' }),
    deps: undefined as unknown as HostDeps,
  };
  f.deps = {
    rules: {
      check_request: (head) => rules.check_request(head),
      access: (url) => rules.access(url),
      take_token: (now) => rules.take_token(now),
      redirect: (method, url, status, location) => sandbox.redirect(method, url, status, location),
    },
    crypto: sandbox,
    http: {
      send(request) {
        f.sent.push(request);
        if (f.relayAnswer) return f.relayAnswer(request);
        const header = (name: string) => request.headers.find(([n]) => n.toLowerCase() === name)?.[1] ?? '';
        const inner = JSON.parse(header('jf-relay-headers')) as [string, string][];
        const answer = f.service(header('jf-relay-url'), header('jf-relay-method'), inner);
        return {
          status: 200,
          headers: [
            ['jf-relay-status', String(answer.status)],
            ['jf-relay-headers', asciiJson(answer.headers)],
          ],
          body: new TextEncoder().encode(answer.body ?? ''),
          url: request.url,
        };
      },
    },
    pageIsHttps: true,
    kv: new Map(),
    secrets: new Map(),
    onKv: () => undefined,
    onSecret: () => undefined,
    log: () => undefined,
    onNetworkError: (...args) => f.network.push(args),
    relay: () => relay,
    cookies: {
      header: (url, now) => jar.header(url, now),
      store: (url, setCookies, now) => jar.store(url, setCookies, now),
    },
    onCookies: () => (f.jarChanges += 1),
    now: () => clock.now,
    wallNow: () => NOW,
    random: (n) => new Uint8Array(n),
    sleepSync: (ms) => (clock.now += ms),
    call,
  };
  return f;
}

function attempt<T>(run: () => T): { ok: true; value: T } | { ok: false; error: { tag: string; val?: string } } {
  try {
    return { ok: true, value: run() };
  } catch (thrown) {
    return { ok: false, error: (thrown as { payload: { tag: string; val?: string } }).payload };
  }
}

type Sent = { status: number; headers: { name: string; value: string }[]; body: Uint8Array; url: string };

function send(f: Fake, url: string, extra: Partial<{ method: string; headers: { name: string; value: string }[]; body: Uint8Array; followRedirects: boolean }> = {}) {
  const http = createImports(f.deps)['jellyfesh:plugin/http'] as { send(r: unknown): Sent };
  return http.send({ method: 'get', url, headers: [], body: new Uint8Array(), followRedirects: true, ...extra });
}

function header(request: TransportRequest, name: string): string | undefined {
  return request.headers.find(([n]) => n.toLowerCase() === name)?.[1];
}

describe('the relay protocol', () => {
  it('wraps a request in headers, as ASCII', () => {
    const wrapped = wrap(RELAY, 'post', API, [['Referer', 'https://y.qq.com/'], ['X-Name', 'Zoë 🎵']]);
    expect(wrapped.url).toBe('http://127.0.0.1:8790/v1/forward');
    expect(wrapped.method).toBe('POST');
    const headers = new Map(wrapped.headers);
    expect(headers.get('Authorization')).toBe(`Bearer ${RELAY.token}`);
    expect(headers.get('Jf-Relay-Url')).toBe(API);
    expect(headers.get('Jf-Relay-Method')).toBe('POST');
    expect(headers.get('Jf-Relay-Headers')).toMatch(/^[\x20-\x7e]+$/);
    expect(JSON.parse(headers.get('Jf-Relay-Headers')!)).toEqual([['Referer', 'https://y.qq.com/'], ['X-Name', 'Zoë 🎵']]);
  });

  it('unwraps answers and refusals', () => {
    const answer = unwrap(200, (n) => ({ 'jf-relay-status': '302', 'jf-relay-headers': '[["Location","/next"]]' })[n] ?? null);
    expect(answer).toEqual({ status: 302, headers: [['location', '/next']] });
    const refused = unwrap(401, (n) => (n === 'jf-relay-error' ? 'token' : null), new TextEncoder().encode('{"error":"token","message":"The relay refused the token."}'));
    expect(refused).toBeInstanceOf(RelayError);
    expect((refused as RelayError).code).toBe('token');
    expect((unwrap(0, () => null) as RelayError).code).toBe('unreachable');
  });
});

describe('http through the relay', () => {
  it('sends every header the plugin set, the forbidden ones too, with the token', () => {
    const f = fake();
    const answer = send(f, `${API}?sign=zzb1`, {
      method: 'post',
      body: new TextEncoder().encode('{"comm":{}}'),
      headers: [
        { name: 'referer', value: 'https://y.qq.com/' },
        { name: 'origin', value: 'https://y.qq.com' },
        { name: 'user-agent', value: 'Mozilla/5.0' },
        { name: 'cookie', value: 'uin=7000001; qm_keyst=W_X_key' },
      ],
    });
    expect(answer.status).toBe(200);
    expect(new TextDecoder().decode(answer.body)).toBe('{"code":0}');
    expect(answer.url).toBe(`${API}?sign=zzb1`);
    const sent = f.sent[0];
    expect(sent.url).toBe('http://127.0.0.1:8790/v1/forward');
    expect(sent.method).toBe('POST');
    expect(header(sent, 'authorization')).toBe(`Bearer ${RELAY.token}`);
    expect(header(sent, 'jf-relay-method')).toBe('POST');
    expect(JSON.parse(header(sent, 'jf-relay-headers')!)).toEqual([
      ['referer', 'https://y.qq.com/'],
      ['origin', 'https://y.qq.com'],
      ['user-agent', 'Mozilla/5.0'],
      ['cookie', 'uin=7000001; qm_keyst=W_X_key'],
    ]);
    expect(new TextDecoder().decode(sent.body!)).toBe('{"comm":{}}');
  });

  it('still checks the allowlist first', () => {
    const f = fake();
    expect(attempt(() => send(f, 'https://tracker.example/collect'))).toMatchObject({ ok: false, error: { tag: 'not-allowed' } });
    expect(f.sent).toHaveLength(0);
  });

  it('keeps the cookies services set, and sends them before the plugin’s own', () => {
    const f = fake();
    f.service = () => ({ status: 200, headers: [['set-cookie', 'MUSIC_U=session; Max-Age=600; Path=/'], ['set-cookie', '__csrf=ab12; Path=/']] });
    const first = send(f, API);
    // The plugin reads Set-Cookie, which a page never sees without the relay.
    expect(first.headers.filter((h) => h.name === 'set-cookie')).toHaveLength(2);
    expect(f.jarChanges).toBe(1);
    send(f, API, { headers: [{ name: 'cookie', value: 'own=1' }] });
    const inner = JSON.parse(header(f.sent[1], 'jf-relay-headers')!) as [string, string][];
    expect(inner).toContainEqual(['cookie', 'MUSIC_U=session; __csrf=ab12; own=1']);
    // Not to another host.
    send(f, 'https://open.weixin.qq.com/connect/qrconnect');
    const other = JSON.parse(header(f.sent[2], 'jf-relay-headers')!) as [string, string][];
    expect(other.some(([name]) => name === 'cookie')).toBe(false);
  });

  it('follows redirects itself, each hop checked, credentials kept to their origin', () => {
    const f = fake();
    f.service = (url) =>
      url === API ? { status: 302, headers: [['location', 'https://open.weixin.qq.com/next']] } : { status: 200, headers: [], body: 'there' };
    const answer = send(f, API, { method: 'post', body: new Uint8Array([1]), headers: [{ name: 'Cookie', value: 'a=b' }, { name: 'X-Keep', value: '1' }] });
    expect(new TextDecoder().decode(answer.body)).toBe('there');
    expect(answer.url).toBe('https://open.weixin.qq.com/next');
    expect(header(f.sent[1], 'jf-relay-method')).toBe('GET');
    expect(f.sent[1].body).toBeNull();
    expect(JSON.parse(header(f.sent[1], 'jf-relay-headers')!)).toEqual([['X-Keep', '1']]);
    // A redirect off the allowlist is refused; without followRedirects it comes back as it is.
    f.service = () => ({ status: 301, headers: [['location', 'https://evil.example/']] });
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'not-allowed' } });
    expect(send(f, API, { followRedirects: false }).status).toBe(301);
    f.service = () => ({ status: 302, headers: [['location', API]] });
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'too-many-redirects' } });
  });

  it('turns the relay’s refusals into the import’s errors and tells the page', () => {
    const f = fake();
    const refuse = (status: number, code: string) => {
      f.relayAnswer = () => ({ status, headers: [['jf-relay-error', code]], body: new TextEncoder().encode(`{"error":"${code}","message":"no"}`), url: '' });
    };
    refuse(401, 'token');
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'connection' } });
    expect(f.network.at(-1)).toEqual([RELAY.address, 'relay', 'token']);
    refuse(403, 'host');
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'not-allowed' } });
    refuse(403, 'address');
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'blocked-address' } });
    refuse(504, 'timeout');
    expect(attempt(() => send(f, API))).toMatchObject({ ok: false, error: { tag: 'timeout' } });
  });

  it('sends directly when no relay is set up', () => {
    const f = fake(null);
    f.relayAnswer = (request) => ({ status: 200, headers: [], body: new Uint8Array(), url: request.url });
    send(f, API, { headers: [{ name: 'referer', value: 'https://y.qq.com/' }] });
    expect(f.sent[0].url).toBe(API);
    expect(f.sent[0].headers).toEqual([]);
  });
});

describe('which sources need a relay (the core)', () => {
  it('knows the services that refuse browsers', () => {
    expect(jf.refusing_service('community.unofficial.qqmusic', '[]')).toBe('QQ Music');
    expect(jf.refusing_service('community.unofficial.netease', '[]')).toBe('NetEase Cloud Music');
    expect(jf.refusing_service('org.jellyfesh.subsonic', JSON.stringify([{ Setting: 'server_url' }]))).toBeUndefined();
    expect(jf.refusing_service('org.example.qq', JSON.stringify([{ Host: { host: 'u.y.qq.com', wildcard: false, port: 443 } }]))).toBe('QQ Music');
  });

  it('checks the relay address against the page', () => {
    expect(jf.relay_address('http://127.0.0.1:8790/', true)).toBe('http://127.0.0.1:8790');
    expect(() => jf.relay_address('http://192.168.1.20:8790', true)).toThrow('mixed-content');
    expect(jf.relay_address('http://192.168.1.20:8790', false)).toBe('http://192.168.1.20:8790');
    expect(() => jf.relay_address('relay.example.com', false)).toThrow('invalid');
  });
});
