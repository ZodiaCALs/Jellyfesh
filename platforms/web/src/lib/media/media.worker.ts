// The media worker (the apps' media thread, docs/architecture.md §4): decodes artwork with the
// browser's own decoders (off the main thread), hands the pixels to jf-web-media for the theme
// colours and baked backdrops (jf-palette), and reads the tags of dropped files (lofty). Loaded on
// demand, so neither its code nor its work touches the first screen.

import init, { Artwork, Tags } from '../../../generated/wasm/jf_web_media.js';
import mediaUrl from '../../../generated/wasm/jf_web_media_bg.wasm?url';

export type MediaRequest =
  | { type: 'artwork'; id: number; image: Blob; appearances: ('light' | 'dark')[] }
  | { type: 'tags'; id: number; file: Blob; name: string };

export type MediaReply =
  | { type: 'artwork'; id: number; themes: string; backdrops: { appearance: string; png: Blob }[] }
  | { type: 'tags'; id: number; tags: string; cover: Uint8Array | null }
  | { type: 'error'; id: number; message: string };

/** The artwork's long edge for analysis and baking: the bake keeps 1/4 of it, at most 256 px. */
const MAX_EDGE = 1024;

const scope = self as unknown as {
  postMessage(message: MediaReply, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<MediaRequest>) => void) | null;
};

const ready = init({ module_or_path: mediaUrl });

async function artwork(request: Extract<MediaRequest, { type: 'artwork' }>) {
  await ready;
  const probe = await createImageBitmap(request.image);
  const scale = Math.min(1, MAX_EDGE / Math.max(probe.width, probe.height));
  const width = Math.max(1, Math.round(probe.width * scale));
  const height = Math.max(1, Math.round(probe.height * scale));
  probe.close();
  const bitmap = await createImageBitmap(request.image, {
    resizeWidth: width,
    resizeHeight: height,
    resizeQuality: 'medium',
    colorSpaceConversion: 'none',
    premultiplyAlpha: 'none',
  });
  const canvas = new OffscreenCanvas(width, height);
  const context = canvas.getContext('2d', { willReadFrequently: true })!;
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  const pixels = context.getImageData(0, 0, width, height).data;
  const art = new Artwork(width, height, new Uint8Array(pixels.buffer, pixels.byteOffset, pixels.byteLength));
  const themes = art.themes();
  const backdrops: { appearance: string; png: Blob }[] = [];
  for (const appearance of request.appearances) {
    const baked = art.backdrop(appearance);
    const out = new OffscreenCanvas(baked.width, baked.height);
    const image = new ImageData(new Uint8ClampedArray(baked.pixels()), baked.width, baked.height);
    out.getContext('2d')!.putImageData(image, 0, 0);
    backdrops.push({ appearance, png: await out.convertToBlob({ type: 'image/png' }) });
    baked.free();
  }
  art.free();
  scope.postMessage({ type: 'artwork', id: request.id, themes, backdrops });
}

async function tags(request: Extract<MediaRequest, { type: 'tags' }>) {
  await ready;
  const bytes = new Uint8Array(await request.file.arrayBuffer());
  const read = new Tags(bytes, request.name);
  const cover = read.cover() ?? null;
  scope.postMessage({ type: 'tags', id: request.id, tags: read.json(), cover }, cover ? [cover.buffer] : []);
  read.free();
}

scope.onmessage = (event) => {
  const request = event.data;
  const run = request.type === 'artwork' ? artwork(request) : tags(request);
  run.catch((error) =>
    scope.postMessage({ type: 'error', id: request.id, message: error instanceof Error ? error.message : String(error) }),
  );
};
