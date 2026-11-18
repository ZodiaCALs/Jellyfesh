// Jellyfesh Web's side of a relay the person runs themselves (tools/jf-relay, docs/web-relay.md),
// for sources whose services refuse browsers: no CORS headers, and headers a page may not set
// (Cookie, Referer, User-Agent, Origin). Which sources need it is decided by the core
// (jf_web::refusing_service) or learned from a failed request (./diagnose.ts).
//
// A request to the service is wrapped in a POST to `<relay>/v1/forward`: its URL, method and
// headers travel in `Jf-Relay-*` headers (the headers as ASCII JSON), its body as the body, the
// token as `Authorization: Bearer`. The relay answers 200 with the service's status and headers in
// `Jf-Relay-Status` and `Jf-Relay-Headers` (Set-Cookie and Location included: it never follows a
// redirect, the worker checks each hop) and the service's body; or a 4xx/5xx with
// `Jf-Relay-Error` when it refuses. These helpers are pure, shared by the plugin workers (a
// synchronous request), the page (covers, downloads) and, in plain JavaScript, the service worker
// (streams; scripts/sw.template.js).

export interface RelayConfig {
  /** The relay's address, normalised by the core (`jf_web::relay_address`). */
  address: string;
  token: string;
}

export const FORWARD = '/v1/forward';
export const PING = '/v1/ping';

/** Why the relay did not pass a request on (`Jf-Relay-Error`), or `unreachable` when nothing answered. */
export type RelayErrorCode =
  | 'origin'
  | 'token'
  | 'host'
  | 'address'
  | 'request'
  | 'dns'
  | 'upstream'
  | 'timeout'
  | 'busy'
  | 'unreachable';

export class RelayError extends Error {
  constructor(
    readonly code: RelayErrorCode,
    message: string,
  ) {
    super(message);
  }
}

/** JSON with every non-ASCII character escaped: header values are bytes (Latin-1 to browsers). */
export function asciiJson(value: unknown): string {
  return JSON.stringify(value).replace(/[\u007f-￿]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
}

export interface Wrapped {
  url: string;
  method: 'POST';
  headers: [string, string][];
}

/** The POST to the relay that makes `method url` with `headers` (a body goes as its body). */
export function wrap(relay: RelayConfig, method: string, url: string, headers: [string, string][]): Wrapped {
  return {
    url: `${relay.address}${FORWARD}`,
    method: 'POST',
    headers: [
      ['Authorization', `Bearer ${relay.token}`],
      ['Jf-Relay-Url', url],
      ['Jf-Relay-Method', method.toUpperCase()],
      ['Jf-Relay-Headers', asciiJson(headers)],
    ],
  };
}

export interface Unwrapped {
  /** The service's status. */
  status: number;
  /** The service's headers, names in lowercase. */
  headers: [string, string][];
}

const CODES: RelayErrorCode[] = ['origin', 'token', 'host', 'address', 'request', 'dns', 'upstream', 'timeout', 'busy'];

/**
 * The service's answer inside a relay's response (`status` and `header` as the response gives
 * them), or the RelayError the relay answered with.
 */
export function unwrap(status: number, header: (name: string) => string | null, body?: Uint8Array): Unwrapped | RelayError {
  const inner = header('jf-relay-status');
  if (status === 200 && inner !== null && /^\d{3}$/.test(inner)) {
    let headers: [string, string][] = [];
    try {
      const parsed = JSON.parse(header('jf-relay-headers') ?? '[]') as unknown;
      if (Array.isArray(parsed)) {
        headers = parsed
          .filter((pair): pair is [string, string] => Array.isArray(pair) && typeof pair[0] === 'string' && typeof pair[1] === 'string')
          .map(([name, value]) => [name.toLowerCase(), value]);
      }
    } catch {
      // A relay that sent no readable headers: the status still stands.
    }
    return { status: Number(inner), headers };
  }
  const code = header('jf-relay-error');
  let message = '';
  try {
    message = body && body.length > 0 ? String((JSON.parse(new TextDecoder().decode(body)) as { message?: unknown }).message ?? '') : '';
  } catch {
    // Not the relay's JSON.
  }
  if (code && (CODES as string[]).includes(code)) return new RelayError(code as RelayErrorCode, message || code);
  return new RelayError('unreachable', message || `the relay answered ${status}`);
}

/** A header of an unwrapped answer. */
export function headerOf(answer: Unwrapped, name: string): string | null {
  return answer.headers.find(([n]) => n === name)?.[1] ?? null;
}

/** Makes a request through the relay from the page (covers, downloads): the service's answer and its body. */
export async function relayFetch(
  relay: RelayConfig,
  url: string,
  headers: [string, string][] = [],
  init: { method?: string; signal?: AbortSignal } = {},
): Promise<{ answer: Unwrapped; response: Response }> {
  const wrapped = wrap(relay, init.method ?? 'GET', url, headers);
  let response: Response;
  try {
    response = await fetch(wrapped.url, {
      method: 'POST',
      headers: wrapped.headers,
      mode: 'cors',
      credentials: 'omit',
      cache: 'no-store',
      referrerPolicy: 'no-referrer',
      signal: init.signal,
    });
  } catch {
    throw new RelayError('unreachable', `cannot reach the relay at ${relay.address}`);
  }
  const answer = unwrap(response.status, (name) => response.headers.get(name), response.ok ? undefined : new Uint8Array(await response.arrayBuffer()));
  if (answer instanceof RelayError) throw answer;
  return { answer, response };
}

/** Checks the relay: null when it answers this page with this token, else why not. */
export async function pingRelay(relay: RelayConfig, timeoutMs = 8000): Promise<RelayError | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(`${relay.address}${PING}`, {
      headers: { Authorization: `Bearer ${relay.token}` },
      mode: 'cors',
      credentials: 'omit',
      cache: 'no-store',
      referrerPolicy: 'no-referrer',
      signal: controller.signal,
    });
    if (response.ok) {
      const body = (await response.json().catch(() => null)) as { relay?: unknown } | null;
      return body?.relay === 'jf-relay' ? null : new RelayError('unreachable', 'not a Jellyfesh relay');
    }
    const error = unwrap(response.status, (name) => response.headers.get(name), new Uint8Array(await response.arrayBuffer()));
    return error instanceof RelayError ? error : new RelayError('unreachable', `the relay answered ${response.status}`);
  } catch {
    // Nothing answered, or the relay does not allow this page (CORS hides which): ask without
    // CORS, which needs no permission. An answer then means the relay is there but refused this
    // page's origin.
    try {
      await fetch(`${relay.address}${PING}`, { mode: 'no-cors', credentials: 'omit', cache: 'no-store', signal: controller.signal });
      return new RelayError('origin', 'the relay does not accept this page');
    } catch {
      return new RelayError('unreachable', `cannot reach the relay at ${relay.address}`);
    }
  } finally {
    clearTimeout(timer);
  }
}
