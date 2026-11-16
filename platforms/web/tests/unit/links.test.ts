// Open in {Service} and remote sources (API 1.1) through the core's real exports (jf-web, built by
// `npm run wasm`), with the JSON the page hands them: a package's `links` as jf-web-sandbox reads
// them from the manifest, and the install check's package description (src/lib/plugins/host.ts).

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import * as jf from '../../generated/wasm/jf_web.js';
import type { ContentPolicy, Links } from '../../src/lib/core/types';

beforeAll(() => {
  const wasm = fileURLToPath(new URL('../../generated/wasm/jf_web_bg.wasm', import.meta.url));
  jf.initSync({ module: readFileSync(wasm) });
});

const POLICY: ContentPolicy = {
  unified_search: true,
  mixed_queue: true,
  library_storage: 'Full',
  metadata_ttl_secs: null,
  lyrics_sync: true,
  attribution: null,
};

function source(links: Links | null | undefined) {
  return new jf.Source(
    'org.example.linked',
    'Linked Sample',
    JSON.stringify([{ Setting: 'server_url' }]),
    JSON.stringify({ server_url: 'http://127.0.0.1:4610' }),
    JSON.stringify(POLICY),
    JSON.stringify(links ?? null),
  );
}

describe('item links', () => {
  it('fills in the templates with the id, percent-encoded', () => {
    const linked = source({
      track: 'https://music.example.com/song/{id}',
      album: 'https://music.example.com/album?id={id}&from=jf',
      artist: null,
      playlist: null,
    });
    expect(linked.link('Track', 'st-1 a/b')).toBe('https://music.example.com/song/st-1%20a%2Fb');
    expect(linked.link('Album', 'al&1')).toBe('https://music.example.com/album?id=al%261&from=jf');
    expect(linked.link('Artist', 'ar-1')).toBeUndefined();
    expect(linked.link('Folder', 'f-1')).toBeUndefined();
    expect(linked.link('Track', '')).toBeUndefined();
    linked.free();
  });

  it('gives none for plugins without links (API 1.0, or kept before 1.1)', () => {
    for (const links of [null, undefined]) {
      const plain = source(links);
      expect(plain.link('Track', '1')).toBeUndefined();
      plain.free();
    }
  });
});

describe('remote sources', () => {
  const describePackage = (playback_mode: string) =>
    JSON.stringify({
      id: 'org.example.remote',
      name: 'Remote Sample',
      signature: { Trusted: { key_id: 'release', label: 'Jellyfesh' } },
      playback_mode,
    });

  it('are refused at install with the words of docs/web.md', () => {
    expect(jf.install_refusal(describePackage('Remote'), undefined, true)).toBe(
      'Remote Sample plays on your own devices, which Jellyfesh Web cannot do yet. Use the Jellyfesh app on your computer or phone.',
    );
    expect(jf.install_refusal(describePackage('Stream'), undefined, true)).toBeUndefined();
  });
});
