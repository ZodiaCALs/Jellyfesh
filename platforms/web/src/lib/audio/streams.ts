// Turning a plugin's StreamInfo into something an <audio> element can load.
//
// An element cannot send request headers, and some servers need them (Jellyfin's Authorization).
// With the service worker in control, the element loads a same-origin `__stream/<key>` URL and the
// worker fetches the real one with the headers, passing Range requests through; it asks this page
// for the URL and headers each time, so they live only in the page's memory and never in a URL or
// a cache. Without the service worker (the first visit, before it takes over, or a browser that
// blocks it) the page fetches the whole file first. Streams without headers load directly, with
// CORS, since Web Audio needs the samples.
//
// A stream of a source that goes through the relay (its service refuses browsers; ../plugins/
// relay.ts) is fetched through the relay the same two ways: the service worker unwraps the
// relay's answer (the service's status and headers, Content-Range included), and without it the
// page downloads the song through the relay first.

import type { StreamInfo } from '../core/types';
import { headerOf, type RelayConfig, relayFetch, wrap } from '../plugins/relay';
import type { MediaSource } from './engine';

interface Target {
  url: string;
  method?: 'POST';
  headers: [string, string][];
  /** The answer is the relay's: the service worker unwraps it. */
  relayed?: boolean;
}

const targets = new Map<string, Target>();
let listening = false;

function listen() {
  if (listening || typeof navigator === 'undefined' || !navigator.serviceWorker) return;
  listening = true;
  navigator.serviceWorker.addEventListener('message', (event: MessageEvent) => {
    const data = event.data as { type?: string; key?: string } | null;
    if (data?.type !== 'stream-request' || typeof data.key !== 'string') return;
    event.ports[0]?.postMessage(targets.get(data.key) ?? null);
  });
}

function randomKey(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

export interface Prepared {
  media: MediaSource;
  /** Frees what the stream holds (a registration or a downloaded file). */
  release(): void;
}

/** Registers `target` with the service worker: the element loads `__stream/<key>`. */
function throughWorker(target: Target): Prepared {
  listen();
  const key = randomKey();
  targets.set(key, target);
  return {
    media: { url: new URL(`__stream/${key}`, document.baseURI).href, crossOrigin: false },
    release: () => targets.delete(key),
  };
}

function fromMemory(blob: Blob): Prepared {
  const url = URL.createObjectURL(blob);
  return { media: { url, crossOrigin: false }, release: () => URL.revokeObjectURL(url) };
}

/** The media for `info`; `relay` when its source goes through one. */
export async function prepareStream(info: StreamInfo, relay: RelayConfig | null = null): Promise<Prepared> {
  const headers = info.headers.map((h) => [h.name, h.value] as [string, string]);
  const worker = typeof navigator !== 'undefined' && navigator.serviceWorker?.controller;
  if (relay) {
    if (worker) {
      const wrapped = wrap(relay, 'GET', info.url, headers);
      return throughWorker({ url: wrapped.url, method: 'POST', headers: wrapped.headers, relayed: true });
    }
    const { answer, response } = await relayFetch(relay, info.url, headers);
    if (answer.status !== 200) throw new Error(`the server answered ${answer.status}`);
    return fromMemory(new Blob([await response.arrayBuffer()], { type: headerOf(answer, 'content-type') ?? '' }));
  }
  if (headers.length === 0) {
    return { media: { url: info.url, crossOrigin: true }, release() {} };
  }
  if (worker) return throughWorker({ url: info.url, headers });
  const response = await fetch(info.url, { headers, mode: 'cors', credentials: 'omit', cache: 'no-store' });
  if (!response.ok) throw new Error(`the server answered ${response.status}`);
  return fromMemory(await response.blob());
}
