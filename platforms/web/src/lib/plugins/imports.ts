// The WIT host imports (plugins/wit/imports.wit) for a plugin running in a Web Worker, as jco's
// generated code calls them: synchronously. Everything the browser provides is injected (the
// network request, the clock, storage hand-off, the Rust sandbox rules), so the rules here are
// unit-tested without a browser (tests/unit/imports.test.ts).
//
// What matches the native host (core/jf-plugin-host), and where a browser differs, is in
// docs/web.md ("How plugins run"):
//
// * http: the request rules and the allowlist are jf_plugin_model's (through jf-web-sandbox), the
//   URL a redirect ended at is checked again, max_rps is the same token bucket, responses are
//   limited to 16 MiB and 64 MiB per call. The browser sends the request itself (a synchronous
//   XMLHttpRequest in the worker): it follows redirects itself, keeps no per-plugin cookie jar,
//   refuses headers it forbids to pages, resolves names itself, and needs CORS from the server.
// * kv: plain values (keys of 1-128 printable ASCII bytes, 1 MiB per plugin) and secrets (names of
//   a-z 0-9 . _ -), handed to the page to persist; a value containing a password typed for the
//   login in progress is refused, as natively.
// * crypto: jf_plugin_model::crypto in Rust (jf-web-sandbox); random bytes from getRandomValues.
// * clock, log: Date.now and performance.now; log lines rate-limited (bursts of 100, then 20 a
//   second), cut at 2 KiB, control characters escaped, typed passwords masked.

export const KV_QUOTA = 1024 * 1024;
export const KV_MAX_KEY = 128;
export const MAX_SECRET = 64 * 1024;
export const MAX_RANDOM = 64 * 1024;
export const LOG_BURST = 100;
export const LOG_RATE = 20;
export const LOG_MAX = 2048;
/** Protected values shorter than this are not looked for inside stored values. */
const MIN_PROTECTED_MATCH = 4;

/** The Rust rules for one plugin (jf_web_sandbox.Sandbox). */
export interface SandboxRules {
  /** Returns the URL to send, or throws an Error whose message is `{"NotAllowed":…}` or `{"InvalidRequest":…}`. */
  check_request(head: string): string;
  /** `user`, `fixed` or undefined: whether the allowlist covers a URL. */
  access(url: string): string | undefined;
  /** Undefined when a request token is free at `nowMs`, else the wait in ms. */
  take_token(nowMs: number): number | undefined;
}

/** The crypto import's primitives (jf_web_sandbox's functions). */
export interface CryptoBackend {
  digest(algorithm: string, data: Uint8Array): Uint8Array;
  hmac(algorithm: string, key: Uint8Array, data: Uint8Array): Uint8Array;
  aes_encrypt(mode: string, key: Uint8Array, data: Uint8Array): Uint8Array;
  aes_decrypt(mode: string, key: Uint8Array, data: Uint8Array): Uint8Array;
  rsa_encrypt(key: string, padding: string, data: Uint8Array): Uint8Array;
}

export interface TransportRequest {
  method: string;
  url: string;
  headers: [string, string][];
  body: Uint8Array | null;
  timeoutMs: number;
}

export interface TransportResponse {
  status: number;
  headers: [string, string][];
  body: Uint8Array;
  /** The URL that answered, after redirects. */
  url: string;
}

/** Why a request got no response, as the transport saw it. */
export class TransportError extends Error {
  constructor(
    readonly kind: 'timeout' | 'network',
    message: string,
  ) {
    super(message);
  }
}

/** Sends a request synchronously (an XMLHttpRequest in the worker). */
export interface HttpTransport {
  send(request: TransportRequest): TransportResponse;
}

/** The call in progress. */
export interface CallContext {
  /** When the call must end, on the `now()` clock (ms). */
  deadline: number;
  /** Values typed into the login form being submitted: refused in storage, masked in logs. */
  protect: string[];
  /** Also refuse them as secrets (password-exchange). */
  keystore: boolean;
  /** Response bytes received in this call. */
  received: number;
}

export interface HostDeps {
  rules: SandboxRules;
  crypto: CryptoBackend;
  http: HttpTransport;
  /** Whether the page is https (so plain http is mixed content). */
  pageIsHttps: boolean;
  kv: Map<string, Uint8Array>;
  secrets: Map<string, string>;
  /** A plain value changed (null: deleted); the page persists it. */
  onKv(key: string, value: Uint8Array | null): void;
  /** A secret changed (null: deleted); the page encrypts and persists it. */
  onSecret(name: string, value: string | null): void;
  log(level: string, message: string): void;
  /** A request found no server (origin and path only: queries can hold credentials). */
  onNetworkError(url: string, kind: 'mixed-content' | 'network'): void;
  /** Monotonic ms (performance.now()). */
  now(): number;
  /** Wall-clock ms since the epoch. */
  wallNow(): number;
  random(length: number): Uint8Array;
  /** Waits synchronously (the worker has nothing else to do during a call). */
  sleepSync(ms: number): void;
  call: CallContext;
}

/** jco turns a thrown `{ payload }` into the import's error result. */
function fail(payload: unknown): never {
  throw { payload };
}

/** `{"NotAllowed":"…"}` → `{tag: 'not-allowed', val: '…'}`. */
function variant(json: string): { tag: string; val?: string } {
  try {
    const value = JSON.parse(json);
    if (typeof value === 'string') return { tag: kebab(value) };
    const [name, val] = Object.entries(value)[0] as [string, unknown];
    return { tag: kebab(name), val: String(val) };
  } catch {
    return { tag: 'invalid-input', val: json };
  }
}

function kebab(name: string): string {
  return name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
}

function contains(haystack: Uint8Array, needle: Uint8Array): boolean {
  if (needle.length === 0 || needle.length > haystack.length) return false;
  outer: for (let i = 0; i + needle.length <= haystack.length; i++) {
    for (let j = 0; j < needle.length; j++) {
      if (haystack[i + j] !== needle[j]) continue outer;
    }
    return true;
  }
  return false;
}

const encoder = new TextEncoder();

/** Whether `value` holds a password typed for the login in progress. */
export function containsProtected(call: CallContext, value: Uint8Array): boolean {
  return call.protect.some((secret) => {
    const needle = encoder.encode(secret);
    return needle.length >= MIN_PROTECTED_MATCH && contains(value, needle);
  });
}

export function validKvKey(key: string): boolean {
  const bytes = encoder.encode(key);
  return bytes.length > 0 && bytes.length <= KV_MAX_KEY && bytes.every((b) => b >= 0x20 && b < 0x7f);
}

export function validSecretName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 64 &&
    /^[a-z0-9]/.test(name) &&
    !name.includes('..') &&
    /^[a-z0-9._-]+$/.test(name)
  );
}

function kvUsed(kv: Map<string, Uint8Array>): number {
  let used = 0;
  for (const [key, value] of kv) used += encoder.encode(key).length + value.length;
  return used;
}

/** Masks protected values, escapes control characters and cuts at 2 KiB, as the native log. */
export function sanitizeLog(message: string, protect: string[]): string {
  let text = message.length > LOG_MAX * 4 ? message.slice(0, LOG_MAX * 4) : message;
  for (const secret of protect) {
    if (secret.length > 0) text = text.split(secret).join('***');
  }
  text = text.replace(/[\u0000-\u001f\u007f]/g, (c) => {
    const code = c.charCodeAt(0);
    return code === 0x0a ? '\\n' : code === 0x0d ? '\\r' : code === 0x09 ? '\\t' : `\\u{${code.toString(16)}}`;
  });
  if (encoder.encode(text).length > LOG_MAX) {
    while (encoder.encode(text).length > LOG_MAX - 3) text = text.slice(0, -1);
    text += '…';
  }
  return text;
}

/** Bursts of 100 lines, then 20 a second; reports how many were dropped. */
export class LogLimiter {
  private tokens = LOG_BURST;
  private last: number | null = null;
  private dropped = 0;

  admit(nowMs: number): number | null {
    if (this.last !== null) {
      const elapsed = Math.max(0, nowMs - this.last) / 1000;
      this.tokens = Math.min(LOG_BURST, this.tokens + elapsed * LOG_RATE);
    }
    this.last = nowMs;
    if (this.tokens >= 1) {
      this.tokens -= 1;
      const dropped = this.dropped;
      this.dropped = 0;
      return dropped;
    }
    this.dropped += 1;
    return null;
  }
}

/** Headers browsers do not let a page set (Fetch's forbidden request-headers): skipped. */
const BROWSER_SETS = new Set([
  'accept-charset',
  'accept-encoding',
  'access-control-request-headers',
  'access-control-request-method',
  'cookie',
  'cookie2',
  'date',
  'dnt',
  'origin',
  'referer',
  'user-agent',
  'via',
]);

/** Whether the browser refuses to let a page set this (lowercase) header. */
export function browserSetsHeader(lower: string): boolean {
  return BROWSER_SETS.has(lower) || lower.startsWith('sec-');
}

function isLoopback(url: URL): boolean {
  const host = url.hostname;
  return host === 'localhost' || host.endsWith('.localhost') || host === '[::1]' || /^127\./.test(host);
}

/** The import object for jco's `instantiate`. */
export function createImports(deps: HostDeps): Record<string, Record<string, unknown>> {
  const limiter = new LogLimiter();
  const limits = { maxResponse: 16 * 1024 * 1024, maxPerCall: 64 * 1024 * 1024, maxTimeout: 30_000 };

  const http = {
    send(req: {
      method: string;
      url: string;
      headers: { name: string; value: string }[];
      body: Uint8Array;
      timeoutMs?: number;
      followRedirects: boolean;
    }) {
      const call = deps.call;
      const head = {
        method: req.method,
        url: req.url,
        headers: req.headers,
        body_len: req.body.length,
      };
      let url: string;
      try {
        url = deps.rules.check_request(JSON.stringify(head));
      } catch (error) {
        fail(variant((error as Error).message));
      }
      const parsed = new URL(url);
      if (deps.pageIsHttps && parsed.protocol === 'http:' && !isLoopback(parsed)) {
        deps.onNetworkError(`${parsed.origin}${parsed.pathname}`, 'mixed-content');
        fail({
          tag: 'connection',
          val: `mixed content: this page is https and cannot reach ${parsed.origin} over plain http`,
        });
      }
      // max_rps: wait for a token while the call has time.
      for (;;) {
        const wait = deps.rules.take_token(deps.now());
        if (wait === undefined) break;
        if (deps.now() + wait > call.deadline) fail({ tag: 'rate-limited' });
        deps.sleepSync(wait);
      }
      const left = call.deadline - deps.now();
      if (left <= 0) fail({ tag: 'timeout' });
      const timeoutMs = Math.max(1, Math.min(req.timeoutMs ?? limits.maxTimeout, limits.maxTimeout, left));
      const headers: [string, string][] = req.headers
        .filter((h) => !browserSetsHeader(h.name.toLowerCase()))
        .map((h) => [h.name, h.value]);
      let response: TransportResponse;
      try {
        response = deps.http.send({
          method: req.method.toUpperCase(),
          url,
          headers,
          body: req.body.length > 0 ? req.body : null,
          timeoutMs,
        });
      } catch (error) {
        if (error instanceof TransportError && error.kind === 'timeout') fail({ tag: 'timeout' });
        deps.onNetworkError(`${parsed.origin}${parsed.pathname}`, 'network');
        fail({ tag: 'connection', val: (error as Error).message || 'the request failed' });
      }
      if (response.url && response.url !== url && deps.rules.access(response.url) === undefined) {
        fail({ tag: 'not-allowed', val: `${new URL(response.url).origin} is not on the allowlist (after a redirect)` });
      }
      const room = Math.min(limits.maxResponse, limits.maxPerCall - call.received);
      if (response.body.length > room) fail({ tag: 'too-large' });
      call.received += response.body.length;
      return {
        status: response.status,
        headers: response.headers.slice(0, 256).map(([name, value]) => ({ name: name.toLowerCase(), value })),
        body: response.body,
        url: response.url || url,
      };
    },
  };

  const kv = {
    get(key: string) {
      if (!validKvKey(key)) fail({ tag: 'invalid-key', val: 'keys are 1–128 bytes of printable ASCII' });
      return deps.kv.get(key);
    },
    set(key: string, value: Uint8Array) {
      if (!validKvKey(key)) fail({ tag: 'invalid-key', val: 'keys are 1–128 bytes of printable ASCII' });
      if (containsProtected(deps.call, value) || containsProtected(deps.call, encoder.encode(key))) {
        fail({ tag: 'forbidden', val: 'a password typed for this login cannot be stored; exchange it for a token' });
      }
      const current = deps.kv.get(key);
      const used = kvUsed(deps.kv) - (current ? encoder.encode(key).length + current.length : 0);
      if (used + encoder.encode(key).length + value.length > KV_QUOTA) fail({ tag: 'quota-exceeded' });
      const copy = new Uint8Array(value);
      deps.kv.set(key, copy);
      deps.onKv(key, copy);
    },
    delete(key: string) {
      if (!validKvKey(key)) fail({ tag: 'invalid-key', val: 'keys are 1–128 bytes of printable ASCII' });
      if (deps.kv.delete(key)) deps.onKv(key, null);
    },
    setSecret(name: string, value: string) {
      if (!validSecretName(name)) fail({ tag: 'invalid-key', val: 'secret names are 1–64 characters of a-z 0-9 . _ -' });
      if (value.length > MAX_SECRET) fail({ tag: 'invalid-key', val: 'a secret is at most 64 KiB' });
      if (deps.call.keystore && containsProtected(deps.call, encoder.encode(value))) {
        fail({ tag: 'forbidden', val: 'a password typed for this login cannot be stored; exchange it for a token' });
      }
      deps.secrets.set(name, value);
      deps.onSecret(name, value);
    },
    getSecret(name: string) {
      if (!validSecretName(name)) fail({ tag: 'invalid-key', val: 'secret names are 1–64 characters of a-z 0-9 . _ -' });
      return deps.secrets.get(name);
    },
    deleteSecret(name: string) {
      if (!validSecretName(name)) fail({ tag: 'invalid-key', val: 'secret names are 1–64 characters of a-z 0-9 . _ -' });
      if (deps.secrets.delete(name)) deps.onSecret(name, null);
    },
  };

  const algorithm = (tag: string) => tag.charAt(0).toUpperCase() + tag.slice(1);
  const cryptoCall = (f: () => Uint8Array) => {
    try {
      return f();
    } catch (error) {
      fail(variant((error as Error).message));
    }
  };
  /** An `aes-mode` variant as jf-plugin-model's JSON. */
  const aesMode = (mode: { tag: string; val?: unknown }) => {
    switch (mode.tag) {
      case 'cbc':
        return JSON.stringify({ Cbc: Array.from(mode.val as Uint8Array) });
      case 'gcm': {
        const p = mode.val as { nonce: Uint8Array; aad: Uint8Array };
        return JSON.stringify({ Gcm: { nonce: Array.from(p.nonce), aad: Array.from(p.aad) } });
      }
      default:
        return JSON.stringify('Ecb');
    }
  };
  const rsaKey = (key: { tag: string; val: unknown }) => {
    switch (key.tag) {
      case 'pem':
        return JSON.stringify({ Pem: key.val });
      case 'der':
        return JSON.stringify({ Der: Array.from(key.val as Uint8Array) });
      default: {
        const c = key.val as { modulus: Uint8Array; exponent: Uint8Array };
        return JSON.stringify({ Components: { modulus: Array.from(c.modulus), exponent: Array.from(c.exponent) } });
      }
    }
  };
  const crypto = {
    digest: (alg: string, data: Uint8Array) => cryptoCall(() => deps.crypto.digest(algorithm(alg), data)),
    hmac: (alg: string, key: Uint8Array, data: Uint8Array) =>
      cryptoCall(() => deps.crypto.hmac(algorithm(alg), key, data)),
    aesEncrypt: (mode: { tag: string; val?: unknown }, key: Uint8Array, data: Uint8Array) =>
      cryptoCall(() => deps.crypto.aes_encrypt(aesMode(mode), key, data)),
    aesDecrypt: (mode: { tag: string; val?: unknown }, key: Uint8Array, data: Uint8Array) =>
      cryptoCall(() => deps.crypto.aes_decrypt(aesMode(mode), key, data)),
    rsaEncrypt: (key: { tag: string; val: unknown }, padding: string, data: Uint8Array) =>
      cryptoCall(() =>
        deps.crypto.rsa_encrypt(
          rsaKey(key),
          padding === 'pkcs1v15' ? 'Pkcs1v15' : padding === 'oaep-sha1' ? 'OaepSha1' : 'OaepSha256',
          data,
        ),
      ),
    randomBytes(length: number) {
      if (length > MAX_RANDOM) fail({ tag: 'too-large' });
      return deps.random(length);
    },
  };

  const clock = {
    nowUtcMs: () => BigInt(Math.floor(deps.wallNow())),
    monotonicMs: () => BigInt(Math.floor(deps.now())),
  };

  const log = {
    log(level: string, message: string) {
      const text = sanitizeLog(message, deps.call.protect);
      const admitted = limiter.admit(deps.now());
      if (admitted === null) return;
      if (admitted > 0) deps.log('warn', `${admitted} log lines dropped (rate limit)`);
      deps.log(level, text);
    },
  };

  // jco's instantiation mode looks interfaces up without their version (`jellyfesh:plugin/http`);
  // the versioned names of API 1.0 and 1.1 are there too, for tools that use them.
  const interfaces: Record<string, Record<string, unknown>> = { http, kv, crypto, clock, log, types: {} };
  const imports: Record<string, Record<string, unknown>> = {};
  for (const [name, value] of Object.entries(interfaces)) {
    imports[`jellyfesh:plugin/${name}`] = value;
    imports[`jellyfesh:plugin/${name}@1.0.0`] = value;
    imports[`jellyfesh:plugin/${name}@1.1.0`] = value;
  }
  return imports;
}
