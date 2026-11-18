// Artwork for rows and Now Playing. Some sources put no image in their items (a Subsonic cover URL
// carries the sign-in), so the page asks the plugin's `artwork` for the rows on screen, once each,
// as the apps' thumbnail loader does. How a picture reaches an <img>:
//
// * a plain URL loads straight into <img> (no CORS needed to show it), without a Referer: the
//   native apps send none unless the plugin asks, and image hosts that refuse other sites'
//   pages accept requests without one;
// * a picture that needs request headers is fetched (CORS) and shown from memory;
// * a picture of a source that goes through the relay (its service refuses browsers) is fetched
//   through the relay with the plugin's headers, and shown from memory.
//
// An answer is kept per source, account, item and size; a failed one is not (the plugin was
// restarting, the sign-in was not finished, the network dropped): it is asked again after a pause.

import type { ImageRef, Item } from '../core/types';
import type { Plugin } from '../plugins/host';
import { headerOf, type RelayConfig, relayFetch } from '../plugins/relay';

const MAX_REFS = 4000;
const MAX_BLOBS = 300;
/** How long a failed answer is not asked again. */
const RETRY_MS = 30_000;

function trim<K, V>(map: Map<K, V>, max: number, drop?: (value: V) => void) {
  while (map.size > max) {
    const [key, value] = map.entries().next().value as [K, V];
    map.delete(key);
    drop?.(value);
  }
}

export class Artworks {
  private refs = new Map<string, Promise<ImageRef | null>>();
  private failed = new Map<string, number>();
  private blobs = new Map<string, Promise<string | null>>();

  constructor(private readonly now: () => number = () => Date.now()) {}

  /** The image for `item` at about `size` px. */
  ref(plugin: Plugin | undefined, item: Item, size: number): Promise<ImageRef | null> {
    const own = item.artwork;
    if (own && (own.width === null || own.width >= size * 0.75)) return Promise.resolve(own);
    if (!plugin || !plugin.declares('Artwork') || plugin.blocked) return Promise.resolve(own);
    // A new sign-in asks again: what the plugin answered before it may have been nothing.
    const key = `${plugin.id}\u0000${plugin.state.account ?? ''}\u0000${item.kind}\u0000${item.id}\u0000${size}`;
    let found = this.refs.get(key);
    if (!found) {
      const failedAt = this.failed.get(key);
      if (failedAt !== undefined && this.now() - failedAt < RETRY_MS) return Promise.resolve(own);
      found = plugin.artwork(item, size).then(
        (image) => image ?? own,
        () => {
          this.refs.delete(key);
          this.failed.set(key, this.now());
          trim(this.failed, MAX_REFS);
          return own;
        },
      );
      this.refs.set(key, found);
      trim(this.refs, MAX_REFS);
    }
    return found;
  }

  /** A URL an <img> can show for `image`; `relay` when the picture's source goes through one. */
  url(image: ImageRef, relay: RelayConfig | null = null): Promise<string | null> {
    if (image.headers.length === 0 && !relay) return Promise.resolve(image.url);
    const headers = image.headers.map((h) => [h.name, h.value] as [string, string]);
    const key = `${relay ? 'relay' : 'cors'}\u0000${image.url}\u0000${headers.map(([n, v]) => `${n}:${v}`).join('\u0000')}`;
    let found = this.blobs.get(key);
    if (!found) {
      found = (relay ? fetchRelayed(relay, image.url, headers) : fetchDirect(image.url, headers)).then((blob) => {
        if (!blob) this.blobs.delete(key);
        return blob ? URL.createObjectURL(blob) : null;
      });
      this.blobs.set(key, found);
      trim(this.blobs, MAX_BLOBS, (url) => void url.then((u) => u && URL.revokeObjectURL(u)));
    }
    return found;
  }
}

/** A picture fetched with its headers (the server must allow this page: CORS). */
export async function fetchDirect(url: string, headers: [string, string][]): Promise<Blob | null> {
  try {
    const response = await fetch(url, { headers, mode: 'cors', credentials: 'omit', referrerPolicy: 'no-referrer' });
    return response.ok ? await response.blob() : null;
  } catch {
    return null;
  }
}

/** A picture fetched through the relay, typed as the service said. */
export async function fetchRelayed(relay: RelayConfig, url: string, headers: [string, string][]): Promise<Blob | null> {
  try {
    const { answer, response } = await relayFetch(relay, url, headers);
    if (answer.status !== 200) return null;
    return new Blob([await response.arrayBuffer()], { type: headerOf(answer, 'content-type') ?? '' });
  } catch {
    return null;
  }
}
