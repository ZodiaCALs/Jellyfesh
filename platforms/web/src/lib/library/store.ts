// Where the music library lives in the browser: IndexedDB `jellyfesh-library`, apart from the
// plugins' database.
//
// * folders: the folders the person added, by name only. No folder handles are kept, in any
//   browser: reading a kept handle back has crashed Chromium-based browsers (the Chromium these
//   tests run in, and reports for Opera), and a crash there would come at every start. The
//   person picks a folder again on each visit before its songs play;
// * tracks: what the scan read from each song's tags (`LibTrack`), by key `<folder>/<path>`;
// * artwork: each embedded cover once, by the SHA-256 of its bytes.
//
// Without IndexedDB everything lives in memory for the visit.

import type { LibTrack } from '../core/types';

/** What a finished scan of a folder found. */
export interface ScanReport {
  at: number;
  added: number;
  updated: number;
  removed: number;
  /** Files that could not be read. */
  errors: number;
}

export interface Folder {
  id: number;
  name: string;
  addedAt: number;
  lastScan: ScanReport | null;
}

export interface Cover {
  bytes: Uint8Array;
  mime: string | null;
}

export interface LibraryStore {
  readonly persistent: boolean;
  folders(): Promise<Folder[]>;
  /** Adds a folder and returns it with its new id. */
  addFolder(name: string): Promise<Folder>;
  saveFolder(folder: Folder): Promise<void>;
  /** Removes a folder and every track of it. */
  removeFolder(id: number): Promise<void>;
  tracks(): Promise<LibTrack[]>;
  putTracks(tracks: LibTrack[]): Promise<void>;
  deleteTracks(keys: string[]): Promise<void>;
  cover(id: string): Promise<Cover | null>;
  hasCover(id: string): Promise<boolean>;
  putCover(id: string, cover: Cover): Promise<void>;
  /** Deletes the covers no track uses any more. */
  pruneCovers(used: Set<string>): Promise<void>;
}

const DB = 'jellyfesh-library';
const VERSION = 1;

function request<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function done(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

class IndexedLibrary implements LibraryStore {
  readonly persistent = true;

  constructor(private readonly db: IDBDatabase) {}

  async folders(): Promise<Folder[]> {
    const tx = this.db.transaction('folders', 'readonly');
    return (await request(tx.objectStore('folders').getAll())) as Folder[];
  }

  async addFolder(name: string): Promise<Folder> {
    const tx = this.db.transaction('folders', 'readwrite');
    const record: Omit<Folder, 'id'> = { name, addedAt: Date.now(), lastScan: null };
    const id = (await request(tx.objectStore('folders').add(record))) as number;
    await done(tx);
    return { id, ...record };
  }

  async saveFolder(folder: Folder): Promise<void> {
    const tx = this.db.transaction('folders', 'readwrite');
    tx.objectStore('folders').put(folder);
    await done(tx);
  }

  async removeFolder(id: number): Promise<void> {
    const tx = this.db.transaction(['folders', 'tracks'], 'readwrite');
    tx.objectStore('folders').delete(id);
    tx.objectStore('tracks').index('folder').openKeyCursor(IDBKeyRange.only(id)).onsuccess = (event) => {
      const cursor = (event.target as IDBRequest<IDBCursor | null>).result;
      if (!cursor) return;
      tx.objectStore('tracks').delete(cursor.primaryKey);
      cursor.continue();
    };
    await done(tx);
  }

  async tracks(): Promise<LibTrack[]> {
    const tx = this.db.transaction('tracks', 'readonly');
    return (await request(tx.objectStore('tracks').getAll())) as LibTrack[];
  }

  async putTracks(tracks: LibTrack[]): Promise<void> {
    if (tracks.length === 0) return;
    const tx = this.db.transaction('tracks', 'readwrite');
    const store = tx.objectStore('tracks');
    for (const track of tracks) store.put(track);
    await done(tx);
  }

  async deleteTracks(keys: string[]): Promise<void> {
    if (keys.length === 0) return;
    const tx = this.db.transaction('tracks', 'readwrite');
    const store = tx.objectStore('tracks');
    for (const key of keys) store.delete(key);
    await done(tx);
  }

  async cover(id: string): Promise<Cover | null> {
    const tx = this.db.transaction('artwork', 'readonly');
    return ((await request(tx.objectStore('artwork').get(id))) as Cover | undefined) ?? null;
  }

  async hasCover(id: string): Promise<boolean> {
    const tx = this.db.transaction('artwork', 'readonly');
    return (await request(tx.objectStore('artwork').count(id))) > 0;
  }

  async putCover(id: string, cover: Cover): Promise<void> {
    const tx = this.db.transaction('artwork', 'readwrite');
    tx.objectStore('artwork').put(cover, id);
    await done(tx);
  }

  async pruneCovers(used: Set<string>): Promise<void> {
    const tx = this.db.transaction('artwork', 'readwrite');
    const store = tx.objectStore('artwork');
    const keys = (await request(store.getAllKeys())) as string[];
    for (const key of keys) if (!used.has(key)) store.delete(key);
    await done(tx);
  }
}

class MemoryLibrary implements LibraryStore {
  readonly persistent = false;
  private folderList = new Map<number, Folder>();
  private trackList = new Map<string, LibTrack>();
  private covers = new Map<string, Cover>();
  private next = 1;

  async folders(): Promise<Folder[]> {
    return [...this.folderList.values()];
  }

  async addFolder(name: string): Promise<Folder> {
    const folder: Folder = { id: this.next++, name, addedAt: Date.now(), lastScan: null };
    this.folderList.set(folder.id, folder);
    return folder;
  }

  async saveFolder(folder: Folder): Promise<void> {
    this.folderList.set(folder.id, folder);
  }

  async removeFolder(id: number): Promise<void> {
    this.folderList.delete(id);
    for (const [key, track] of this.trackList) if (track.folder === id) this.trackList.delete(key);
  }

  async tracks(): Promise<LibTrack[]> {
    return [...this.trackList.values()];
  }

  async putTracks(tracks: LibTrack[]): Promise<void> {
    for (const track of tracks) this.trackList.set(track.key, track);
  }

  async deleteTracks(keys: string[]): Promise<void> {
    for (const key of keys) this.trackList.delete(key);
  }

  async cover(id: string): Promise<Cover | null> {
    return this.covers.get(id) ?? null;
  }

  async hasCover(id: string): Promise<boolean> {
    return this.covers.has(id);
  }

  async putCover(id: string, cover: Cover): Promise<void> {
    this.covers.set(id, cover);
  }

  async pruneCovers(used: Set<string>): Promise<void> {
    for (const key of this.covers.keys()) if (!used.has(key)) this.covers.delete(key);
  }
}

/** The library's store: IndexedDB, or memory when the browser offers no storage. */
export async function openLibraryStore(): Promise<LibraryStore> {
  try {
    const open = indexedDB.open(DB, VERSION);
    open.onupgradeneeded = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains('folders')) db.createObjectStore('folders', { keyPath: 'id', autoIncrement: true });
      if (!db.objectStoreNames.contains('tracks')) {
        db.createObjectStore('tracks', { keyPath: 'key' }).createIndex('folder', 'folder');
      }
      if (!db.objectStoreNames.contains('artwork')) db.createObjectStore('artwork');
    };
    return new IndexedLibrary(await request(open));
  } catch {
    return new MemoryLibrary();
  }
}
