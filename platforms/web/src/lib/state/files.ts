// Audio files the person drags in or opens: they play from memory for this visit and are never
// stored (no saved library on the web). The core decides what each file is and which lyrics file
// goes with which song (jf-web: classify_file, pair_sidecars); the media worker reads the tags.

import type { Jf } from '../core/wasm';
import type { FileMeta, FileTags, Lyrics, NewEntry } from '../core/types';
import { readTags } from '../media/client';

export interface DroppedFile {
  id: string;
  file: File;
  url: string;
  meta: FileMeta;
  tags: FileTags | null;
  cover: { bytes: Uint8Array; mime: string | null } | null;
  sidecar: File | null;
}

function stem(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(0, dot) : name;
}

export class Files {
  private files = new Map<string, DroppedFile>();
  private seq = 0;

  constructor(
    private readonly jf: Jf,
    private readonly core: InstanceType<Jf['Core']>,
  ) {}

  get(id: string): DroppedFile | undefined {
    return this.files.get(id);
  }

  /** Reads `list` and returns queue entries for its audio files, in order; lyrics files pair up. */
  async add(list: File[]): Promise<NewEntry[]> {
    const kinds = list.map((f) => this.jf.classify_file(f.name, f.type));
    const relevant = list.filter((_, i) => kinds[i] !== 'other');
    const pairs = JSON.parse(this.jf.pair_sidecars(JSON.stringify(relevant.map((f) => [f.name, f.type])))) as [
      number,
      number | null,
    ][];
    const entries: NewEntry[] = [];
    for (const [audio, lyrics] of pairs) {
      const file = relevant[audio];
      let tags: FileTags | null = null;
      let cover: DroppedFile['cover'] = null;
      try {
        const read = await readTags(file, file.name);
        tags = read.tags;
        if (read.cover) cover = { bytes: read.cover, mime: read.tags.cover_mime };
      } catch {
        // No readable tags: the name stands in.
      }
      const id = `f${++this.seq}-${Date.now().toString(36)}`;
      const meta: FileMeta = {
        id,
        name: file.name,
        title: tags?.title?.trim() || stem(file.name),
        artist: tags?.artist ?? tags?.album_artist ?? null,
        album: tags?.album ?? null,
        duration_ms: tags?.duration_ms ?? null,
        replaygain: tags?.replaygain ?? null,
      };
      this.core.add_file(JSON.stringify(meta));
      this.files.set(id, {
        id,
        file,
        url: URL.createObjectURL(file),
        meta,
        tags,
        cover,
        sidecar: lyrics === null ? null : relevant[lyrics],
      });
      entries.push({ kind: 'file', id });
    }
    return entries;
  }

  /** The lyrics of a dropped file: its lyrics file, else its tags. */
  async lyrics(id: string): Promise<Lyrics | null> {
    const dropped = this.files.get(id);
    if (!dropped) return null;
    if (dropped.sidecar) {
      try {
        const bytes = new Uint8Array(await dropped.sidecar.arrayBuffer());
        return JSON.parse(this.jf.lyrics_from_file(bytes, dropped.sidecar.name)) as Lyrics;
      } catch {
        // Unreadable lyrics file: try the tags.
      }
    }
    if (dropped.tags?.lyrics) {
      try {
        return JSON.parse(this.jf.lyrics_from_tags(dropped.tags.lyrics)) as Lyrics;
      } catch {
        return null;
      }
    }
    return null;
  }
}
