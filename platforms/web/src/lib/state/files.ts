// Files that play: audio files the person drags in or opens (they play from memory for this visit
// and are never stored), and the library's songs, opened through the browser when they play
// (./library.svelte.ts). The core decides what each file is and which lyrics file goes with which
// song (jf-web: classify_file, pair_sidecars); the media worker reads the tags.

import type { Jf } from '../core/wasm';
import type { FileMeta, FileTags, Lyrics, NewEntry } from '../core/types';
import { readTags } from '../media/client';
import { type Library, LIBRARY_ID } from './library.svelte';

/** Library songs kept open at once (each holds an object URL). */
const OPEN_LIBRARY_FILES = 6;

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
  private library: Library | null = null;
  /** Library songs opened, oldest first. */
  private opened: string[] = [];

  constructor(
    private readonly jf: Jf,
    private readonly core: InstanceType<Jf['Core']>,
  ) {}

  attach(library: Library) {
    this.library = library;
  }

  get(id: string): DroppedFile | undefined {
    return this.files.get(id);
  }

  /** A file to play: a dropped one, or a library song opened through the browser now. */
  async open(id: string): Promise<DroppedFile | undefined> {
    const known = this.files.get(id);
    if (known || !id.startsWith(LIBRARY_ID) || !this.library) return known;
    const opened = await this.library.open(id.slice(LIBRARY_ID.length));
    if (!opened) return undefined;
    const meta = this.library.metaOf(opened.track);
    const t = opened.track;
    const file: DroppedFile = {
      id,
      file: opened.file,
      url: URL.createObjectURL(opened.file),
      meta,
      tags: {
        title: t.title,
        artist: t.artist,
        album: t.album,
        album_artist: t.album_artist,
        artist_values: t.artist_values,
        explicit_artists: t.explicit_artists,
        year: t.year,
        track_no: t.track_no,
        disc_no: t.disc_no,
        duration_ms: t.duration_ms,
        codec: t.codec,
        sample_rate: t.sample_rate,
        channels: null,
        bitrate_kbps: t.bitrate_kbps,
        replaygain: t.replaygain,
        lyrics: null,
        cover_mime: opened.cover?.mime ?? null,
      },
      cover: opened.cover,
      sidecar: opened.sidecar,
    };
    this.files.set(id, file);
    this.opened.push(id);
    while (this.opened.length > OPEN_LIBRARY_FILES) {
      const old = this.opened.shift()!;
      const gone = this.files.get(old);
      if (gone) URL.revokeObjectURL(gone.url);
      this.files.delete(old);
    }
    return file;
  }

  /** Why a file cannot play, for people. */
  whyClosed(id: string): 'dropped' | 'ask' | 'pick' | 'gone' {
    if (!id.startsWith(LIBRARY_ID) || !this.library) return 'dropped';
    return this.library.unavailable(id.slice(LIBRARY_ID.length));
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

  /** The lyrics of a file: its lyrics file, else its tags. */
  async lyrics(id: string): Promise<Lyrics | null> {
    const dropped = await this.open(id);
    if (!dropped) return null;
    if (!dropped.tags?.lyrics && id.startsWith(LIBRARY_ID) && !dropped.sidecar) {
      // Library songs keep no lyrics in the catalogue: read the tags for them.
      try {
        const read = await readTags(dropped.file, dropped.meta.name);
        if (dropped.tags) dropped.tags.lyrics = read.tags.lyrics;
      } catch {
        // No readable tags.
      }
    }
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
