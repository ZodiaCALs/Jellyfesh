// Artwork for rows and Now Playing. Some sources put no image in their items (a Subsonic cover URL
// carries the sign-in), so the page asks the plugin's `artwork` for the rows on screen, once each,
// as the apps' thumbnail loader does. Images that need request headers are fetched and shown from
// memory; the rest load straight into <img>.

import type { ImageRef, Item } from '../core/types';
import type { Plugin } from '../plugins/host';

const MAX_REFS = 4000;
const MAX_BLOBS = 300;

function trim<K, V>(map: Map<K, V>, max: number, drop?: (value: V) => void) {
  while (map.size > max) {
    const [key, value] = map.entries().next().value as [K, V];
    map.delete(key);
    drop?.(value);
  }
}

export class Artworks {
  private refs = new Map<string, Promise<ImageRef | null>>();
  private blobs = new Map<string, Promise<string | null>>();

  /** The image for `item` at about `size` px. */
  ref(plugin: Plugin | undefined, item: Item, size: number): Promise<ImageRef | null> {
    const own = item.artwork;
    if (own && (own.width === null || own.width >= size * 0.75)) return Promise.resolve(own);
    if (!plugin || !plugin.declares('Artwork') || plugin.blocked) return Promise.resolve(own);
    const key = `${plugin.id}\u0000${item.kind}\u0000${item.id}\u0000${size}`;
    let found = this.refs.get(key);
    if (!found) {
      found = plugin.artwork(item, size).then(
        (image) => image ?? own,
        () => own,
      );
      this.refs.set(key, found);
      trim(this.refs, MAX_REFS);
    }
    return found;
  }

  /** A URL an <img> can show for `image`. */
  url(image: ImageRef): Promise<string | null> {
    if (image.headers.length === 0) return Promise.resolve(image.url);
    const key = `${image.url}\u0000${image.headers.map((h) => `${h.name}:${h.value}`).join('\u0000')}`;
    let found = this.blobs.get(key);
    if (!found) {
      found = fetch(image.url, {
        headers: image.headers.map((h) => [h.name, h.value] as [string, string]),
        mode: 'cors',
        credentials: 'omit',
      })
        .then(async (response) => (response.ok ? URL.createObjectURL(await response.blob()) : null))
        .catch(() => null);
      this.blobs.set(key, found);
      trim(this.blobs, MAX_BLOBS, (url) => void url.then((u) => u && URL.revokeObjectURL(u)));
    }
    return found;
  }
}
