// Jellyfesh Web's service worker (written to dist/sw.js by scripts/postbuild.mjs, which fills in
// the version and the app shell). It:
//
// * keeps the app shell (the page, its script and styles, the core's WebAssembly, the icons) so the
//   app starts without a connection, for example to reach a server on the local network;
// * keeps hashed assets the first time they load (workers, the other WebAssembly modules), and the
//   bundled plugins, refreshed from the network whenever it answers;
// * carries `__stream/<key>` requests for streams that need request headers (an <audio> element
//   cannot send them): it asks the page for the stream's URL and headers each time and fetches it
//   with them, passing Range requests through. Nothing of a server's is ever cached. A stream of a
//   source that goes through the person's relay (src/lib/plugins/relay.ts) is a POST to the relay,
//   whose answer carries the service's status and headers in Jf-Relay-Status and Jf-Relay-Headers:
//   the worker answers the element with those.

const VERSION = '__VERSION__';
const SHELL = __SHELL__;
const CACHE = `jellyfesh-shell-${VERSION}`;
const RUNTIME = `jellyfesh-runtime-${VERSION}`;

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(SHELL))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) {
        if (key.startsWith('jellyfesh-') && key !== CACHE && key !== RUNTIME) await caches.delete(key);
      }
      await self.clients.claim();
    })(),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  // Servers' requests (streams, artwork, API calls) are never touched.
  if (url.origin !== self.location.origin) return;
  const scope = new URL(self.registration.scope).pathname;
  if (!url.pathname.startsWith(scope)) return;
  const path = url.pathname.slice(scope.length);
  if (path.startsWith('__stream/')) {
    event.respondWith(stream(event, decodeURIComponent(path.slice('__stream/'.length))));
  } else if (request.mode === 'navigate') {
    event.respondWith(networkFirst(request, CACHE, new URL('index.html', self.registration.scope).href));
  } else if (path.startsWith('assets/')) {
    event.respondWith(cacheFirst(request));
  } else {
    event.respondWith(networkFirst(request, RUNTIME));
  }
});

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(RUNTIME);
    await cache.put(request, response.clone());
  }
  return response;
}

async function networkFirst(request, cacheName, key = request) {
  try {
    const response = await fetch(request);
    if (response.ok && response.type === 'basic') {
      const cache = await caches.open(cacheName);
      await cache.put(key, response.clone());
    }
    return response;
  } catch (error) {
    const cached = await caches.match(key);
    if (cached) return cached;
    throw error;
  }
}

/** Asks the page that owns `clientId` (or any open page) for the stream behind `key`. */
async function target(clientId, key) {
  const candidates = [];
  if (clientId) {
    const client = await self.clients.get(clientId);
    if (client) candidates.push(client);
  }
  if (candidates.length === 0) candidates.push(...(await self.clients.matchAll({ type: 'window' })));
  for (const client of candidates) {
    const answer = await new Promise((resolve) => {
      const channel = new MessageChannel();
      const timer = setTimeout(() => resolve(null), 3000);
      channel.port1.onmessage = (event) => {
        clearTimeout(timer);
        resolve(event.data);
      };
      client.postMessage({ type: 'stream-request', key }, [channel.port2]);
    });
    if (answer) return answer;
  }
  return null;
}

async function stream(event, key) {
  const found = await target(event.clientId || event.resultingClientId, key);
  if (!found) return new Response('', { status: 404, statusText: 'Unknown stream' });
  const headers = new Headers(found.headers);
  const range = event.request.headers.get('range');
  if (range) headers.set('range', range);
  const upstream = await fetch(found.url, {
    method: found.method || 'GET',
    headers,
    mode: 'cors',
    credentials: 'omit',
    cache: 'no-store',
    referrerPolicy: 'no-referrer',
  });
  // Through the relay: the service's status and headers are inside its answer.
  let status = upstream.status;
  let statusText = upstream.statusText;
  let served = upstream.headers;
  if (found.relayed) {
    const inner = upstream.headers.get('jf-relay-status');
    if (upstream.status !== 200 || !inner) {
      return new Response('', { status: 502, statusText: upstream.headers.get('jf-relay-error') || 'Relay' });
    }
    status = Number(inner);
    statusText = '';
    served = new Headers();
    try {
      for (const [name, value] of JSON.parse(upstream.headers.get('jf-relay-headers') || '[]')) served.append(name, value);
    } catch {
      // No readable headers: the body still plays.
    }
  }
  const out = new Headers();
  for (const name of ['content-type', 'content-length', 'content-range', 'accept-ranges', 'last-modified', 'etag']) {
    const value = served.get(name);
    if (value) out.set(name, value);
  }
  // A server that does not expose Content-Range to scripts: rebuild it for open-ended ranges.
  if (status === 206 && !out.has('content-range') && range) {
    const match = /^bytes=(\d+)-(\d*)$/.exec(range.trim());
    const length = Number(served.get('content-length'));
    if (match && Number.isFinite(length) && length > 0) {
      const start = Number(match[1]);
      const total = match[2] === '' ? String(start + length) : '*';
      out.set('content-range', `bytes ${start}-${start + length - 1}/${total}`);
    }
  }
  if (status === 206) out.set('accept-ranges', 'bytes');
  return new Response(status === 204 || status === 304 ? null : upstream.body, { status, statusText, headers: out });
}
