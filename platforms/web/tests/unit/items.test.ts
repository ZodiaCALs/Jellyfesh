// The songs and minutes footer of an album or playlist page.

import { describe, expect, it } from 'vitest';
import type { Item } from '../../src/lib/core/types';
import { minutesText, summary } from '../../src/lib/items';

describe('the songs and minutes footer', () => {
  function track(seconds: number | null): Item {
    return {
      id: String(Math.random()),
      kind: 'Track',
      title: 'Invented',
      artist: null,
      album: null,
      album_id: null,
      artist_id: null,
      duration_ms: seconds === null ? null : seconds * 1000,
      track_number: null,
      disc_number: null,
      year: null,
      child_count: null,
      isrc: null,
      artwork: null,
      explicit: false,
      playable: true,
      unplayable: null,
    };
  }

  it('counts songs and rounds to minutes, at least one', () => {
    expect(summary([track(8), track(8), track(8), track(8)])).toBe('4 songs, 1 minute');
    expect(summary(Array.from({ length: 12 }, () => track(240)))).toBe('12 songs, 48 minutes');
    expect(summary([track(null)])).toBe('1 song');
  });

  it('gives hours from an hour on', () => {
    expect(minutesText(65 * 60_000)).toBe('1 hour 5 minutes');
    expect(minutesText(120 * 60_000)).toBe('2 hours');
    expect(minutesText(10_000)).toBe('1 minute');
  });
});
