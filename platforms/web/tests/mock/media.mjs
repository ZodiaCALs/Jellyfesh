// Generated test media for the mock servers: sine tones as WAV and gradients as PNG. Nothing here
// is a recording or a real cover.

import { deflateSync } from 'node:zlib';

/**
 * A 16-bit mono PCM WAV of a sine tone at `frequency` Hz lasting `seconds`, at full `amplitude`
 * from the first sample to the last (no fades, so a gap between two tracks shows as silence).
 */
export function wav(seconds, frequency, { sampleRate = 44100, amplitude = 0.5 } = {}) {
  const frames = Math.round(seconds * sampleRate);
  const data = Buffer.alloc(frames * 2);
  for (let i = 0; i < frames; i++) {
    const sample = Math.round(Math.sin((2 * Math.PI * frequency * i) / sampleRate) * amplitude * 32767);
    data.writeInt16LE(sample, i * 2);
  }
  const header = Buffer.alloc(44);
  header.write('RIFF', 0, 'ascii');
  header.writeUInt32LE(36 + data.length, 4);
  header.write('WAVE', 8, 'ascii');
  header.write('fmt ', 12, 'ascii');
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(sampleRate * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write('data', 36, 'ascii');
  header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

/**
 * `wav` with RIFF INFO tags (title INAM, artist IART, album IPRD, track number IPRT, date ICRD),
 * as tagging software writes them into WAV files.
 */
export function taggedWav(seconds, frequency, { title, artist, album, track, year } = {}) {
  const plain = wav(seconds, frequency);
  const fields = [
    ['INAM', title],
    ['IART', artist],
    ['IPRD', album],
    ['IPRT', track === undefined ? undefined : String(track)],
    ['ICRD', year === undefined ? undefined : String(year)],
  ].filter(([, value]) => value !== undefined);
  const parts = fields.map(([id, value]) => {
    const text = Buffer.from(`${value}\0`, 'utf8');
    const padded = Buffer.alloc(text.length + (text.length % 2));
    text.copy(padded);
    const head = Buffer.alloc(8);
    head.write(id, 0, 'ascii');
    head.writeUInt32LE(text.length, 4);
    return Buffer.concat([head, padded]);
  });
  const info = Buffer.concat([Buffer.from('INFO', 'ascii'), ...parts]);
  const list = Buffer.alloc(8);
  list.write('LIST', 0, 'ascii');
  list.writeUInt32LE(info.length, 4);
  const out = Buffer.concat([plain, list, info]);
  out.writeUInt32LE(out.length - 8, 4);
  return out;
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

/** A `size`×`size` PNG: a diagonal gradient from `from` to `to` ([r, g, b]) with a soft disc. */
export function png(size, from, to) {
  const rows = [];
  for (let y = 0; y < size; y++) {
    const row = Buffer.alloc(1 + size * 3);
    for (let x = 0; x < size; x++) {
      const t = (x + y) / (2 * (size - 1));
      const dx = x - size * 0.62;
      const dy = y - size * 0.38;
      const disc = Math.max(0, 1 - Math.sqrt(dx * dx + dy * dy) / (size * 0.3)) * 0.35;
      for (let c = 0; c < 3; c++) {
        const base = from[c] + (to[c] - from[c]) * t;
        row[1 + x * 3 + c] = Math.round(base + (255 - base) * disc);
      }
    }
    rows.push(row);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // RGB
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}
