// Every string the views translate has a Simplified Chinese, Traditional Chinese and Japanese
// entry, with the same placeholders.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { pickLanguage } from '../../src/lib/i18n/index.svelte';

const src = fileURLToPath(new URL('../../src/', import.meta.url));
const catalogues = Object.fromEntries(
  ['zh-Hans', 'zh-Hant', 'ja'].map((lang) => [
    lang,
    JSON.parse(readFileSync(join(src, 'lib/i18n', `${lang}.json`), 'utf8')) as Record<string, string>,
  ]),
);

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : /\.(ts|svelte)$/.test(name) ? [path] : [];
  });
}

const STRING = "'((?:[^'\\\\]|\\\\.)*)'";
const ids = new Set<string>();
for (const file of files(src)) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(new RegExp(`\\bt\\(\\s*${STRING}`, 'g'))) ids.add(m[1].replace(/\\'/g, "'"));
  for (const m of text.matchAll(new RegExp(`\\btn\\(\\s*${STRING}`, 'g'))) ids.add(m[1].replace(/\\'/g, "'"));
  // pt('context', 'msg'): keyed `context|msg`.
  for (const m of text.matchAll(new RegExp(`\\bpt\\(\\s*${STRING}\\s*,\\s*${STRING}`, 'g'))) {
    ids.add(`${m[1]}|${m[2]}`.replace(/\\'/g, "'"));
  }
}

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('translations', () => {
  it('finds the strings', () => {
    expect(ids.size).toBeGreaterThan(150);
  });

  for (const [lang, catalogue] of Object.entries(catalogues)) {
    it(`has every string in ${lang}, with its placeholders`, () => {
      const missing = [...ids].filter((id) => !(id in catalogue));
      expect(missing).toEqual([]);
      for (const id of ids) expect(placeholders(catalogue[id]), `${lang}: ${id}`).toEqual(placeholders(id));
    });
  }

  it('picks a language from the browser list', () => {
    expect(pickLanguage(['zh-TW', 'en'])).toBe('zh-Hant');
    expect(pickLanguage(['zh-HK'])).toBe('zh-Hant');
    expect(pickLanguage(['zh-CN'])).toBe('zh-Hans');
    expect(pickLanguage(['zh'])).toBe('zh-Hans');
    expect(pickLanguage(['ja-JP'])).toBe('ja');
    expect(pickLanguage(['de-DE', 'ja'])).toBe('ja');
    expect(pickLanguage(['de-DE'])).toBe('en');
  });
});
