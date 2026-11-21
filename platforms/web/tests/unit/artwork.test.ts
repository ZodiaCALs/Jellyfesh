// The cover loader (src/lib/state/artwork.ts): which picture a row shows and how it reaches the
// <img>, with a fake plugin and a fake network.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ImageRef, Item } from '../../src/lib/core/types';
import type { Plugin } from '../../src/lib/plugins/host';
import { Artworks } from '../../src/lib/state/artwork';

function image(url: string, headers: ImageRef['headers'] = [], width: number | null = 300): ImageRef {
  return { url, headers, width, height: width };
}

const ITEM: Item = {
  id: 'al1',
  kind: 'Album',
  title: 'Slow Tide',
  artist: 'Harbour Lights',
  album: null,
  album_id: null,
  artist_id: null,
  duration_ms: null,
  track_number: null,
  disc_number: null,
  year: null,
  child_count: null,
  isrc: null,
  artwork: null,
  explicit: false,
  playable: true,
  unplayable: null,
} as unknown as Item;

/** A plugin whose `artwork` answers with `answer` (a function, so it can fail or change). */
function plugin(answer: () => Promise<ImageRef | null>, account: string | null = 'demo') {
  const calls = { count: 0 };
  const fake = {
    id: 'org.example.music',
    state: { account },
    blocked: null,
    declares: (capability: string) => capability === 'Artwork',
    artwork: () => {
      calls.count += 1;
      return answer();
    },
  };
  return { plugin: fake as unknown as Plugin, calls };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('covers', () => {
  it('asks a source a few at a time, the newest requests first', async () => {
    const started: string[] = [];
    const finish = new Map<string, (image: ImageRef | null) => void>();
    const fake = {
      id: 'org.example.music',
      state: { account: 'demo' },
      blocked: null,
      declares: (capability: string) => capability === 'Artwork',
      artwork: (item: Item) =>
        new Promise<ImageRef | null>((resolve) => {
          started.push(item.id);
          finish.set(item.id, resolve);
        }),
    } as unknown as Plugin;
    const artworks = new Artworks();
    const answers = ['a', 'b', 'c', 'd', 'e'].map((id) => artworks.ref(fake, { ...ITEM, id }, 300));
    expect(started).toEqual(['a', 'b']);
    finish.get('a')?.(image('https://cdn.example/a.jpg'));
    await answers[0];
    await Promise.resolve();
    expect(started).toEqual(['a', 'b', 'e']);
    finish.get('b')?.(null);
    await answers[1];
    await Promise.resolve();
    expect(started).toEqual(['a', 'b', 'e', 'd']);
    finish.get('e')?.(null);
    await answers[4];
    await Promise.resolve();
    expect(started).toEqual(['a', 'b', 'e', 'd', 'c']);
    finish.get('d')?.(null);
    finish.get('c')?.(null);
    await Promise.all(answers);
    expect((await answers[0])?.url).toBe('https://cdn.example/a.jpg');
  });

  it('uses the item’s own picture when it is large enough', async () => {
    const { plugin: p, calls } = plugin(async () => image('https://cdn.example/big.jpg'));
    const own = { ...ITEM, artwork: image('https://cdn.example/own.jpg', [], 600) };
    expect((await new Artworks().ref(p, own, 300))?.url).toBe('https://cdn.example/own.jpg');
    expect(calls.count).toBe(0);
  });

  it('asks the plugin once per account, item and size', async () => {
    const { plugin: p, calls } = plugin(async () => image('https://cdn.example/a.jpg'));
    const artworks = new Artworks();
    await artworks.ref(p, ITEM, 96);
    await artworks.ref(p, ITEM, 96);
    expect(calls.count).toBe(1);
    // After signing in as someone else the plugin is asked again.
    (p as unknown as { state: { account: string } }).state.account = 'other';
    await artworks.ref(p, ITEM, 96);
    expect(calls.count).toBe(2);
  });

  it('does not keep a failed answer: asks again after a pause', async () => {
    let now = 1000;
    let fail = true;
    const { plugin: p, calls } = plugin(async () => {
      if (fail) throw new Error('The plugin was restarted.');
      return image('https://cdn.example/a.jpg');
    });
    const artworks = new Artworks(() => now);
    expect(await artworks.ref(p, ITEM, 96)).toBeNull();
    expect(await artworks.ref(p, ITEM, 96)).toBeNull();
    expect(calls.count).toBe(1);
    now += 31_000;
    fail = false;
    expect((await artworks.ref(p, ITEM, 96))?.url).toBe('https://cdn.example/a.jpg');
    expect(calls.count).toBe(2);
  });

  it('shows plain pictures straight, and fetches those with headers or behind a relay', async () => {
    const fetched: { url: string; init: RequestInit }[] = [];
    vi.stubGlobal('fetch', async (url: string, init: RequestInit) => {
      fetched.push({ url, init });
      const relayed = url.endsWith('/v1/forward');
      return new Response(new Uint8Array([0x89, 0x50]), {
        status: 200,
        headers: relayed ? { 'jf-relay-status': '200', 'jf-relay-headers': '[["content-type","image/png"]]' } : { 'content-type': 'image/jpeg' },
      });
    });
    vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: (blob: Blob) => `blob:${blob.type}` }));
    const artworks = new Artworks();
    expect(await artworks.url(image('https://cdn.example/a.jpg'))).toBe('https://cdn.example/a.jpg');
    expect(fetched).toHaveLength(0);
    expect(await artworks.url(image('https://cdn.example/b.jpg', [{ name: 'Authorization', value: 'x' }]))).toBe('blob:image/jpeg');
    expect(fetched[0].init).toMatchObject({ mode: 'cors', credentials: 'omit', referrerPolicy: 'no-referrer' });
    const relay = { address: 'http://127.0.0.1:8790', token: 'a-token-of-sixteen-chars' };
    expect(await artworks.url(image('https://y.gtimg.cn/c.jpg'), relay)).toBe('blob:image/png');
    expect(fetched[1].url).toBe('http://127.0.0.1:8790/v1/forward');
    expect(new Headers(fetched[1].init.headers).get('jf-relay-url')).toBe('https://y.gtimg.cn/c.jpg');
  });
});
