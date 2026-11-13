// The drawing-time ports replay the contracts recorded from the Rust originals
// (tests/contract/*.json, docs/ffi.md "Drawing-time ports"), as the Swift, Kotlin and C# ports do.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { DisplayClock, nextSecond, nextStep, type PlaybackAnchor } from '../../src/lib/drawing/displayClock';
import { cursor, type LyricTimeline, nextChange } from '../../src/lib/drawing/lyricCursor';

const contracts = fileURLToPath(new URL('../../../../tests/contract/', import.meta.url));

/** JSON with every integer literal as a BigInt (i64 and u64 values do not fit a double). */
function readBig(name: string): any {
  const text = readFileSync(contracts + name, 'utf8');
  return JSON.parse(text, (_key, value, context?: { source?: string }) =>
    typeof value === 'number' && context?.source !== undefined && /^-?\d+$/.test(context.source)
      ? BigInt(context.source)
      : value,
  );
}

function read(name: string): any {
  return JSON.parse(readFileSync(contracts + name, 'utf8'));
}

function anchor(json: any): PlaybackAnchor {
  return {
    trackGeneration: json.track_generation,
    positionUs: json.position_us,
    anchorMonoNs: json.anchor_mono_ns,
    rate: typeof json.rate === 'bigint' ? Number(json.rate) : json.rate,
    durationUs: json.duration_us,
  };
}

describe('DisplayClock port (tests/contract/display-clock.json)', () => {
  const contract = readBig('display-clock.json');

  it('is version 1', () => {
    expect(contract.version).toBe(1n);
  });

  it('replays every case', () => {
    let checked = 0;
    for (const c of contract.cases) {
      const clock = new DisplayClock();
      c.steps.forEach((step: any, i: number) => {
        const at = `${c.name}, step ${i}`;
        switch (step.op) {
          case 'update':
            clock.update(anchor(step.anchor), step.now_ns);
            break;
          case 'position':
            expect(clock.positionUs(step.now_ns), at).toBe(step.expect);
            break;
          case 'wait_until':
            expect(clock.waitUntil(step.target_us, step.now_ns), at).toBe(step.expect_us);
            break;
          case 'running':
            expect(clock.isRunning, at).toBe(step.expect);
            break;
          default:
            throw new Error(`unknown op ${step.op}`);
        }
        checked++;
      });
    }
    expect(checked).toBeGreaterThan(20);
  });

  it('matches next_second', () => {
    for (const [position, expected] of contract.next_second) {
      expect(nextSecond(position), `next_second(${position})`).toBe(expected);
    }
  });

  it('matches next_step', () => {
    for (const [position, step, expected] of contract.next_step) {
      expect(nextStep(position, step), `next_step(${position}, ${step})`).toBe(expected);
    }
  });
});

describe('lyric cursor port (tests/contract/lyrics-cursor.json)', () => {
  const contract = read('lyrics-cursor.json');

  it('is version 1', () => {
    expect(contract.version).toBe(1);
  });

  it('replays every query', () => {
    let checked = 0;
    for (const c of contract.cases) {
      const timeline = c.timeline as LyricTimeline;
      for (const q of c.queries) {
        const at = `${c.name} at ${q.t}`;
        const got = cursor(timeline, q.t);
        expect(got.line, `${at}: line`).toBe(q.line);
        expect(got.word, `${at}: word`).toBe(q.word);
        expect(Math.abs(got.wordProgress - q.word_progress), `${at}: progress`).toBeLessThanOrEqual(1e-6);
        expect(nextChange(timeline, q.t), `${at}: next change`).toBe(q.next_change);
        checked++;
      }
    }
    expect(checked).toBeGreaterThan(100);
  });
});
