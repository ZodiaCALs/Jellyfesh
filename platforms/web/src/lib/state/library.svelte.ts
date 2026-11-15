// The music library: the folders the person added, what a scan read from them, and the lists the
// views show. The catalogue's rules (grouping, sorting, search) are the core's
// (`jf_web::library`); this keeps the tracks in IndexedDB, reads the folders through the browser
// (./library/folders.ts) and hands songs to the player as files.
//
// In the item views the library is one more source, `LIBRARY`: its albums, artists and songs are
// `Item`s, so the pages, rows, cards and menus are the same as for a server.

import type { Jf } from '../core/wasm';
import type {
  AlbumSort,
  FileMeta,
  Item,
  ItemPage,
  LibAlbumView,
  LibArtistView,
  LibStats,
  LibTrack,
  LibTrackView,
  NewEntry,
  TrackSort,
} from '../core/types';
import { readTags } from '../media/client';
import { fileAt, type FolderFile, keepsFolders, mayRead, type PickedFolder, pickFolder, walk } from '../library/folders';
import type { Cover, Folder, LibraryStore, ScanReport } from '../library/store';

/** The library's id where views take a source's id. */
export const LIBRARY = 'library';
/** The core's ids for library files: `lib:<key>`. */
export const LIBRARY_ID = 'lib:';
/** Artwork URLs of library items: `jf-library:<cover id>`. */
export const COVER_SCHEME = 'jf-library:';

/** A folder as Settings › Library lists it. */
export interface FolderView {
  id: number;
  name: string;
  /** `ready`: readable now; `ask`: the browser wants a click to allow reading it again;
   *  `pick`: this browser keeps no access, so the folder must be picked again this visit. */
  access: 'ready' | 'ask' | 'pick';
  lastScan: ScanReport | null;
}

export interface ScanProgress {
  folder: string;
  seen: number;
  changed: number;
}

/** A library song opened for playing. */
export interface OpenedTrack {
  track: LibTrack;
  file: File;
  sidecar: File | null;
  cover: Cover | null;
}

const BATCH = 50;
/** During a scan, the lists refresh at most this often (each refresh rebuilds the catalogue's index). */
const PUBLISH_MS = 1500;
/** Cover images kept as object URLs at once; older ones are released (images on screen stay drawn). */
const COVER_CACHE = 300;

function stem(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(0, dot) : name;
}

function nameOf(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

function dirOf(path: string): string {
  const slash = path.lastIndexOf('/');
  return slash >= 0 ? path.slice(0, slash) : '';
}

/** A short id for a cover: SHA-256 where the page may use WebCrypto, else a 53-bit hash. */
async function coverId(bytes: Uint8Array): Promise<string> {
  if (globalThis.crypto?.subtle) {
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes as BufferSource));
    return Array.from(digest.subarray(0, 16), (b) => b.toString(16).padStart(2, '0')).join('');
  }
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (const b of bytes) {
    h1 = Math.imul(h1 ^ b, 2654435761);
    h2 = Math.imul(h2 ^ b, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return `h${(4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(16)}-${bytes.length.toString(16)}`;
}

function artworkOf(cover: string | null): Item['artwork'] {
  return cover ? { url: `${COVER_SCHEME}${cover}`, headers: [], width: null, height: null } : null;
}

function base(id: string, kind: Item['kind'], title: string): Item {
  return {
    id,
    kind,
    title,
    artist: null,
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
  };
}

export function trackItem(t: LibTrackView): Item {
  return {
    ...base(t.key, 'Track', t.title),
    artist: t.artist,
    artist_id: t.artist_id,
    album: t.album,
    album_id: t.album_id,
    duration_ms: t.duration_ms,
    track_number: t.track_no,
    disc_number: t.disc_no,
    year: t.year,
    artwork: artworkOf(t.artwork),
  };
}

export function albumItem(a: LibAlbumView): Item {
  return {
    ...base(a.id, 'Album', a.title),
    artist: a.artist,
    artist_id: a.artist_id,
    year: a.year,
    child_count: a.track_count,
    duration_ms: a.duration_ms,
    artwork: artworkOf(a.artwork),
  };
}

export function artistItem(a: LibArtistView): Item {
  return { ...base(a.id, 'Artist', a.name), child_count: a.album_count, artwork: artworkOf(a.artwork) };
}

export class Library {
  folders = $state.raw<FolderView[]>([]);
  progress = $state<ScanProgress | null>(null);
  /** Bumps whenever the catalogue changes, so views reload what they show. */
  revision = $state(0);
  stats = $state.raw<LibStats>({ tracks: 0, albums: 0, artists: 0, duration_ms: 0 });
  /** The last scan's problem, if it could not run. */
  problem = $state<string | null>(null);
  readonly keepsFolders = keepsFolders();
  private catalogue: InstanceType<Jf['Library']>;
  private tracks = new Map<string, LibTrack>();
  private records = new Map<number, Folder>();
  /** Folders whose handle the page may read now. */
  private readable = new Set<number>();
  /** Files of folders picked this visit, where the browser keeps no access: path → file. */
  private visit = new Map<number, Map<string, File>>();
  private covers = new Map<string, Promise<string | null>>();
  private scanning: Promise<void> | null = null;

  constructor(
    private readonly jf: Jf,
    private readonly core: InstanceType<Jf['Core']>,
    readonly store: LibraryStore,
    /** Opens an `<input webkitdirectory>` where the browser keeps no folders. */
    private readonly pickFallback: () => Promise<PickedFolder | null>,
  ) {
    this.catalogue = new jf.Library();
  }

  /** Loads the folders and the catalogue kept from earlier visits. */
  async init() {
    const [folders, tracks] = await Promise.all([this.store.folders(), this.store.tracks()]);
    for (const folder of folders) this.records.set(folder.id, folder);
    for (const track of tracks) this.tracks.set(track.key, track);
    this.catalogue.load(JSON.stringify(tracks));
    this.core.add_files(JSON.stringify(tracks.map((t) => this.metaOf(t))));
    await Promise.all(
      folders.map(async (folder) => {
        if (folder.handle && (await mayRead(folder.handle, false).catch(() => false))) this.readable.add(folder.id);
      }),
    );
    this.publish();
  }

  get isEmpty(): boolean {
    return this.tracks.size === 0;
  }

  private publish() {
    this.folders = [...this.records.values()]
      .sort((a, b) => a.name.localeCompare(b.name) || a.id - b.id)
      .map((f) => ({
        id: f.id,
        name: f.name,
        access: this.readable.has(f.id) || this.visit.has(f.id) ? 'ready' : f.handle ? 'ask' : 'pick',
        lastScan: f.lastScan,
      }));
    this.stats = JSON.parse(this.catalogue.stats()) as LibStats;
    this.revision += 1;
  }

  // ---- folders ------------------------------------------------------------------------------

  /** Asks for a folder, adds it and reads it. False when the person cancelled. */
  async addFolder(): Promise<boolean> {
    const picked = await pickFolder(this.pickFallback);
    if (!picked) return false;
    const folder = await this.store.addFolder(picked.name, picked.handle);
    this.records.set(folder.id, folder);
    if (picked.handle) this.readable.add(folder.id);
    if (picked.files) this.visit.set(folder.id, new Map(picked.files.map((f) => [f.path, f.file])));
    this.publish();
    await this.scan([folder.id]);
    return true;
  }

  /** Lets the page read a kept folder again (Chrome and Edge ask once per visit). */
  async allowAccess(id: number): Promise<boolean> {
    const folder = this.records.get(id);
    if (!folder?.handle) return false;
    const allowed = await mayRead(folder.handle, true).catch(() => false);
    if (allowed) this.readable.add(id);
    this.publish();
    return allowed;
  }

  /** Picks a folder again where the browser keeps no access, and reads what changed. */
  async pickAgain(id: number): Promise<boolean> {
    const folder = this.records.get(id);
    if (!folder) return false;
    const picked = await this.pickFallback();
    if (!picked?.files) return false;
    this.visit.set(id, new Map(picked.files.map((f) => [f.path, f.file])));
    if (picked.name !== folder.name) {
      folder.name = picked.name;
      await this.store.saveFolder(folder);
    }
    this.publish();
    await this.scan([id]);
    return true;
  }

  async removeFolder(id: number) {
    await this.store.removeFolder(id);
    this.records.delete(id);
    this.readable.delete(id);
    this.visit.delete(id);
    for (const [key, track] of this.tracks) if (track.folder === id) this.tracks.delete(key);
    this.catalogue.remove_folder(id);
    await this.store.pruneCovers(this.usedCovers());
    this.publish();
  }

  private usedCovers(): Set<string> {
    const used = new Set<string>();
    for (const track of this.tracks.values()) if (track.artwork) used.add(track.artwork);
    return used;
  }

  /** Reads every folder the page can read now again: new, changed and removed files. */
  async rescan() {
    await this.scan([...this.records.keys()].filter((id) => this.readable.has(id) || this.visit.has(id)));
  }

  private async scan(ids: number[]) {
    while (this.scanning) await this.scanning;
    this.scanning = (async () => {
      this.problem = null;
      try {
        for (const id of ids) {
          const folder = this.records.get(id);
          if (folder) await this.scanFolder(folder);
        }
      } catch (error) {
        this.problem = error instanceof Error ? error.message : String(error);
      } finally {
        this.progress = null;
        await this.store.pruneCovers(this.usedCovers()).catch(() => undefined);
        this.publish();
      }
    })();
    try {
      await this.scanning;
    } finally {
      this.scanning = null;
    }
  }

  private async *filesOf(folder: Folder): AsyncGenerator<FolderFile> {
    const visit = this.visit.get(folder.id);
    if (visit) {
      for (const [path, file] of visit) yield { path, file };
    } else if (folder.handle && this.readable.has(folder.id)) {
      yield* walk(folder.handle);
    }
  }

  private async scanFolder(folder: Folder) {
    const progress: ScanProgress = { folder: folder.name, seen: 0, changed: 0 };
    this.progress = { ...progress };
    // List first: lyrics files pair with the songs of their own folder.
    const audio: FolderFile[] = [];
    const lyricsByDir = new Map<string, FolderFile[]>();
    for await (const entry of this.filesOf(folder)) {
      const kind = this.jf.classify_file(nameOf(entry.path), entry.file.type);
      if (kind === 'audio') audio.push(entry);
      else if (kind === 'lyrics') {
        const dir = dirOf(entry.path);
        lyricsByDir.set(dir, [...(lyricsByDir.get(dir) ?? []), entry]);
      }
    }
    const sidecars = new Map<string, string>();
    const byDir = new Map<string, FolderFile[]>();
    for (const entry of audio) byDir.set(dirOf(entry.path), [...(byDir.get(dirOf(entry.path)) ?? []), entry]);
    for (const [dir, songs] of byDir) {
      const lyrics = lyricsByDir.get(dir) ?? [];
      if (lyrics.length === 0) continue;
      const list = [...songs, ...lyrics];
      const pairs = JSON.parse(
        this.jf.pair_sidecars(JSON.stringify(list.map((f) => [nameOf(f.path), f.file.type]))),
      ) as [number, number | null][];
      for (const [song, lyric] of pairs) if (lyric !== null) sidecars.set(list[song].path, list[lyric].path);
    }

    const report: ScanReport = { at: Date.now(), added: 0, updated: 0, removed: 0, errors: 0 };
    const seen = new Set<string>();
    let batch: LibTrack[] = [];
    let published = performance.now();
    const flush = async () => {
      if (batch.length === 0) return;
      await this.store.putTracks(batch);
      this.catalogue.upsert(JSON.stringify(batch));
      this.core.add_files(JSON.stringify(batch.map((t) => this.metaOf(t))));
      batch = [];
      if (performance.now() - published >= PUBLISH_MS) {
        published = performance.now();
        this.publish();
      }
    };
    for (const entry of audio) {
      const key = `${folder.id}/${entry.path}`;
      seen.add(key);
      progress.seen += 1;
      const known = this.tracks.get(key);
      const lyricsPath = sidecars.get(entry.path) ?? null;
      if (known && known.size === entry.file.size && known.modified_ms === entry.file.lastModified) {
        if (known.lyrics_path !== lyricsPath) {
          const changed = { ...known, lyrics_path: lyricsPath };
          this.tracks.set(key, changed);
          batch.push(changed);
        }
      } else {
        const track = await this.read(folder.id, entry, key, lyricsPath, known?.added_ms ?? Date.now());
        if (!track) {
          report.errors += 1;
          continue;
        }
        this.tracks.set(key, track);
        batch.push(track);
        if (known) report.updated += 1;
        else report.added += 1;
        progress.changed += 1;
      }
      if (progress.seen % 10 === 0) this.progress = { ...progress };
      if (batch.length >= BATCH) await flush();
    }
    await flush();
    const gone = [...this.tracks.values()].filter((t) => t.folder === folder.id && !seen.has(t.key)).map((t) => t.key);
    if (gone.length > 0) {
      for (const key of gone) this.tracks.delete(key);
      await this.store.deleteTracks(gone);
      this.catalogue.remove(JSON.stringify(gone));
      report.removed = gone.length;
    }
    folder.lastScan = report;
    await this.store.saveFolder(folder);
  }

  /** Reads a song's tags; null when the file cannot be read. */
  private async read(folder: number, entry: FolderFile, key: string, lyricsPath: string | null, addedMs: number): Promise<LibTrack | null> {
    try {
      const { tags, cover } = await readTags(entry.file, nameOf(entry.path));
      let artwork: string | null = null;
      if (cover && cover.length > 0) {
        artwork = await coverId(cover);
        if (!(await this.store.hasCover(artwork))) await this.store.putCover(artwork, { bytes: cover, mime: tags.cover_mime });
      }
      return {
        key,
        folder,
        path: entry.path,
        title: tags.title?.trim() || null,
        artist: tags.artist ?? null,
        artist_values: tags.artist_values ?? [],
        explicit_artists: tags.explicit_artists ?? [],
        album: tags.album ?? null,
        album_artist: tags.album_artist ?? null,
        year: tags.year ?? null,
        track_no: tags.track_no ?? null,
        disc_no: tags.disc_no ?? null,
        duration_ms: tags.duration_ms ?? null,
        replaygain: tags.replaygain ?? null,
        artwork,
        lyrics_path: lyricsPath,
        size: entry.file.size,
        modified_ms: entry.file.lastModified,
        added_ms: addedMs,
        codec: tags.codec ?? null,
        sample_rate: tags.sample_rate ?? null,
        bitrate_kbps: tags.bitrate_kbps ?? null,
      };
    } catch {
      return null;
    }
  }

  // ---- lists ----------------------------------------------------------------------------------

  albums(sort: AlbumSort, descending: boolean, offset = 0, limit = 10_000): Item[] {
    return (JSON.parse(this.catalogue.albums(sort, descending, offset, limit)) as LibAlbumView[]).map(albumItem);
  }

  artists(offset = 0, limit = 10_000): Item[] {
    return (JSON.parse(this.catalogue.artists(offset, limit)) as LibArtistView[]).map(artistItem);
  }

  songs(sort: TrackSort, descending: boolean, offset = 0, limit = 100_000): Item[] {
    return (JSON.parse(this.catalogue.tracks(sort, descending, offset, limit)) as LibTrackView[]).map(trackItem);
  }

  /** Albums, newest first. */
  recentlyAdded(limit: number): Item[] {
    return this.albums('added', true, 0, limit);
  }

  /** What an album or artist page lists: an album's songs; an artist's albums, then songs. */
  browse(item: Item): ItemPage {
    if (item.kind === 'Album') {
      const found = JSON.parse(this.catalogue.album(item.id)) as { album: LibAlbumView; tracks: LibTrackView[] } | null;
      return { items: found ? found.tracks.map(trackItem) : [], next: null, total: found?.tracks.length ?? 0 };
    }
    if (item.kind === 'Artist') {
      const found = JSON.parse(this.catalogue.artist(item.id)) as {
        artist: LibArtistView;
        albums: LibAlbumView[];
        tracks: LibTrackView[];
      } | null;
      const items = found ? [...found.albums.map(albumItem), ...found.tracks.map(trackItem)] : [];
      return { items, next: null, total: items.length };
    }
    return { items: [], next: null, total: 0 };
  }

  /** The library's album or artist record for a page opened from elsewhere (its id). */
  item(kind: 'Album' | 'Artist', id: string): Item | null {
    if (kind === 'Album') {
      const found = JSON.parse(this.catalogue.album(id)) as { album: LibAlbumView } | null;
      return found ? albumItem(found.album) : null;
    }
    const found = JSON.parse(this.catalogue.artist(id)) as { artist: LibArtistView } | null;
    return found ? artistItem(found.artist) : null;
  }

  search(text: string, limit: number): { tracks: Item[]; albums: Item[]; artists: Item[] } {
    const found = JSON.parse(this.catalogue.search(text, limit)) as {
      tracks: LibTrackView[];
      albums: LibAlbumView[];
      artists: LibArtistView[];
    };
    return { tracks: found.tracks.map(trackItem), albums: found.albums.map(albumItem), artists: found.artists.map(artistItem) };
  }

  // ---- playing ----------------------------------------------------------------------------------

  entry(key: string): NewEntry {
    return { kind: 'file', id: `${LIBRARY_ID}${key}` };
  }

  /** What the core keeps of a library song (its tags), so the queue can hold it. */
  metaOf(track: LibTrack): FileMeta {
    const name = nameOf(track.path);
    return {
      id: `${LIBRARY_ID}${track.key}`,
      name,
      title: track.title?.trim() || stem(name),
      artist: track.artist ?? track.album_artist,
      album: track.album,
      duration_ms: track.duration_ms,
      replaygain: track.replaygain,
    };
  }

  track(key: string): LibTrack | undefined {
    return this.tracks.get(key);
  }

  /**
   * Right after the core restored a saved queue: restoring starts a fresh session, so the
   * library's songs are handed to the core again (those in the queue get their tags back).
   */
  restoreQueue() {
    this.core.add_files(JSON.stringify([...this.tracks.values()].map((t) => this.metaOf(t))));
  }

  /** Opens a song's file (and its lyrics file and cover) through the browser; null when the
   *  folder cannot be read now. */
  async open(key: string): Promise<OpenedTrack | null> {
    const track = this.tracks.get(key);
    if (!track) return null;
    const folder = this.records.get(track.folder);
    if (!folder) return null;
    const visit = this.visit.get(folder.id);
    let file: File | null = null;
    let sidecar: File | null = null;
    if (visit) {
      file = visit.get(track.path) ?? null;
      sidecar = track.lyrics_path ? (visit.get(track.lyrics_path) ?? null) : null;
    } else if (folder.handle && (this.readable.has(folder.id) || (await mayRead(folder.handle, false).catch(() => false)))) {
      this.readable.add(folder.id);
      file = await fileAt(folder.handle, track.path);
      sidecar = track.lyrics_path ? await fileAt(folder.handle, track.lyrics_path) : null;
    }
    if (!file) return null;
    const cover = track.artwork ? await this.store.cover(track.artwork) : null;
    return { track, file, sidecar, cover };
  }

  /** Why a song cannot be opened, for people. */
  unavailable(key: string): 'ask' | 'pick' | 'gone' {
    const track = this.tracks.get(key);
    const folder = track ? this.records.get(track.folder) : undefined;
    if (!folder) return 'gone';
    if (folder.handle) return this.readable.has(folder.id) ? 'gone' : 'ask';
    return this.visit.has(folder.id) ? 'gone' : 'pick';
  }

  /** An object URL for a cover (`jf-library:<id>`); the most recent ones are kept. */
  coverUrl(url: string): Promise<string | null> {
    const id = url.startsWith(COVER_SCHEME) ? url.slice(COVER_SCHEME.length) : url;
    let found = this.covers.get(id);
    if (found) {
      // Most recently used last.
      this.covers.delete(id);
      this.covers.set(id, found);
      return found;
    }
    found = this.store.cover(id).then((cover) =>
      cover ? URL.createObjectURL(new Blob([cover.bytes as BlobPart], { type: cover.mime ?? 'image/jpeg' })) : null,
    );
    this.covers.set(id, found);
    while (this.covers.size > COVER_CACHE) {
      const [oldest, made] = this.covers.entries().next().value as [string, Promise<string | null>];
      this.covers.delete(oldest);
      void made.then((made) => made && URL.revokeObjectURL(made));
    }
    return found;
  }
}
