// Test plugin packages for the end-to-end tests: the build's own Subsonic package with another
// manifest (an invented id and name, API 1.1), left unsigned, so the app treats it as a community
// plugin. A `.jfpkg` is a zip of a few files; this reads one (stored or deflated entries) and writes
// one with stored entries, using only Node's zlib.

import { crc32, inflateRawSync } from 'node:zlib';

const LOCAL = 0x04034b50;
const CENTRAL = 0x02014b50;
const END = 0x06054b50;

/** The files of a zip archive by name. */
export function unzip(bytes: Buffer): Map<string, Buffer> {
  let end = bytes.length - 22;
  while (end >= 0 && bytes.readUInt32LE(end) !== END) end -= 1;
  if (end < 0) throw new Error('not a zip archive');
  const count = bytes.readUInt16LE(end + 10);
  let at = bytes.readUInt32LE(end + 16);
  const files = new Map<string, Buffer>();
  for (let i = 0; i < count; i += 1) {
    if (bytes.readUInt32LE(at) !== CENTRAL) throw new Error('bad central directory');
    const method = bytes.readUInt16LE(at + 10);
    const size = bytes.readUInt32LE(at + 20);
    const nameLength = bytes.readUInt16LE(at + 28);
    const skip = nameLength + bytes.readUInt16LE(at + 30) + bytes.readUInt16LE(at + 32);
    const local = bytes.readUInt32LE(at + 42);
    const name = bytes.toString('utf8', at + 46, at + 46 + nameLength);
    if (bytes.readUInt32LE(local) !== LOCAL) throw new Error(`bad local header for ${name}`);
    const start = local + 30 + bytes.readUInt16LE(local + 26) + bytes.readUInt16LE(local + 28);
    const data = bytes.subarray(start, start + size);
    files.set(name, method === 8 ? inflateRawSync(data) : Buffer.from(data));
    at += 46 + skip;
  }
  return files;
}

/** A zip archive of `files`, stored without compression. */
export function zip(files: Map<string, Buffer>): Buffer {
  const parts: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of files) {
    const nameBytes = Buffer.from(name, 'utf8');
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(LOCAL, 0);
    local.writeUInt16LE(20, 4); // version needed: 2.0
    local.writeUInt16LE(0x21, 12); // 1980-01-01
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    parts.push(local, nameBytes, data);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(CENTRAL, 0);
    entry.writeUInt16LE(20, 4);
    entry.writeUInt16LE(20, 6);
    entry.writeUInt16LE(0x21, 14);
    entry.writeUInt32LE(crc, 16);
    entry.writeUInt32LE(data.length, 20);
    entry.writeUInt32LE(data.length, 24);
    entry.writeUInt16LE(nameBytes.length, 28);
    entry.writeUInt32LE(offset, 42);
    central.push(entry, nameBytes);
    offset += 30 + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(END, 0);
  end.writeUInt16LE(files.size, 8);
  end.writeUInt16LE(files.size, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...parts, directory, end]);
}

/**
 * `subsonic` (the bytes of a Subsonic package) with its manifest changed by `edit`, without a
 * signature.
 */
export function repackage(subsonic: Buffer, edit: (manifest: string) => string): Buffer {
  const files = unzip(subsonic);
  const manifest = files.get('manifest.toml');
  if (!manifest) throw new Error('the package has no manifest.toml');
  files.set('manifest.toml', Buffer.from(edit(manifest.toString('utf8')), 'utf8'));
  files.delete('SIGNATURE');
  return zip(files);
}

/** Replaces `from` in `text`, failing when it is not there (the manifest changed shape). */
export function swap(text: string, from: string | RegExp, to: string): string {
  const out = text.replace(from, to);
  if (out === text) throw new Error(`the manifest has no ${String(from)}`);
  return out;
}
