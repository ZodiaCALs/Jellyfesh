// Turning a plugin's StreamInfo into something an <audio> element can load.
//
// An element cannot send request headers, and some servers need them (Jellyfin's Authorization).
// With the service worker in control, the element loads a same-origin `__stream/<key>` URL and the
// worker fetches the real one with the headers, passing Range requests through; it asks this page
// for the URL and headers each time, so they live only in the page's memory and never in a URL or
// a cache. Without the service worker (the first visit, before it takes over, or a browser that
// blocks it) the page fetches the whole file first. Streams without headers load directly, with
// CORS, since Web Audio needs the samples.

import type { StreamInfo } from '../core/types';
import type { MediaSource } from './engine';

interface Target {
  url: string;
  headers: [string, string][];
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

/** The media for `info`. */
export async function prepareStream(info: StreamInfo): Promise<Prepared> {
  if (info.headers.length === 0) {
    return { media: { url: info.url, crossOrigin: true }, release() {} };
  }
  const headers = info.headers.map((h) => [h.name, h.value] as [string, string]);
  if (typeof navigator !== 'undefined' && navigator.serviceWorker?.controller) {
    listen();
    const key = randomKey();
    targets.set(key, { url: info.url, headers });
    return {
      media: { url: new URL(`__stream/${key}`, document.baseURI).href, crossOrigin: false },
      release: () => targets.delete(key),
    };
  }
  const response = await fetch(info.url, { headers, mode: 'cors', credentials: 'omit', cache: 'no-store' });
  if (!response.ok) throw new Error(`the server answered ${response.status}`);
  const url = URL.createObjectURL(await response.blob());
  return { media: { url, crossOrigin: false }, release: () => URL.revokeObjectURL(url) };
}
