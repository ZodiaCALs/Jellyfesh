// The page's side of the media worker: starts it on first use and matches replies to requests.

import type { FileTags, Themes } from '../core/types';
import type { MediaReply, MediaRequest } from './media.worker';

let worker: Worker | null = null;
let seq = 0;
const waiting = new Map<number, { resolve(reply: MediaReply): void; reject(error: Error): void }>();

function get(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL('./media.worker.ts', import.meta.url), { type: 'module', name: 'media' });
  worker.onmessage = (event: MessageEvent<MediaReply>) => {
    const reply = event.data;
    const pending = waiting.get(reply.id);
    if (!pending) return;
    waiting.delete(reply.id);
    if (reply.type === 'error') pending.reject(new Error(reply.message));
    else pending.resolve(reply);
  };
  worker.onerror = (event) => {
    for (const [, pending] of waiting) pending.reject(new Error(event.message || 'the media worker failed'));
    waiting.clear();
    worker?.terminate();
    worker = null;
  };
  return worker;
}

type WithoutId<T> = T extends unknown ? Omit<T, 'id'> : never;

function ask(request: WithoutId<MediaRequest>): Promise<MediaReply> {
  const id = ++seq;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    get().postMessage({ ...request, id } as MediaRequest);
  });
}

export interface ArtworkResult {
  themes: Themes;
  /** Baked backdrops by appearance, as PNG blobs. */
  backdrops: Map<string, Blob>;
}

/** The theme of an image and its baked backdrops. */
export async function analyseArtwork(image: Blob, appearances: ('light' | 'dark')[]): Promise<ArtworkResult> {
  const reply = await ask({ type: 'artwork', image, appearances });
  if (reply.type !== 'artwork') throw new Error('unexpected reply');
  return {
    themes: JSON.parse(reply.themes) as Themes,
    backdrops: new Map(reply.backdrops.map((b) => [b.appearance, b.png])),
  };
}

/** The tags of a dropped audio file, and its embedded cover. */
export async function readTags(file: Blob, name: string): Promise<{ tags: FileTags; cover: Uint8Array | null }> {
  const reply = await ask({ type: 'tags', file, name });
  if (reply.type !== 'tags') throw new Error('unexpected reply');
  return { tags: JSON.parse(reply.tags) as FileTags, cover: reply.cover };
}
