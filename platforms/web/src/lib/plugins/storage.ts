// Where plugins keep their state in the browser (the native host's `<dir>/data/<id>/` and the
// OS keystore, ADR-0015):
//
// * IndexedDB `jellyfesh`: per plugin its state (on or off, strikes, account, settings, consent),
//   its plain kv values, and its secrets;
// * secrets are encrypted with AES-GCM under a key WebCrypto generated as non-extractable: the key
//   is stored as a CryptoKey object, so no script (not even this one) can read its bytes, and a
//   copy of the database without the browser profile does not reveal the secrets. Each value is
//   bound to its plugin and name as additional data.
//
// Without IndexedDB (some private windows), everything lives in memory until the page closes,
// like the native "session only" keystore, and the Sources page says so. Without WebCrypto (a
// page served over plain http from anywhere but localhost) the rest is kept but secrets are not:
// there is no key to protect them with.

export interface PluginState {
  enabled: boolean;
  strikes: number;
  disabledReason: string | null;
  account: string | null;
  /** The settings as the person set them (JSON object). */
  settings: Record<string, unknown> | null;
  /** The community disclosure was accepted for this plugin. */
  acknowledged: boolean;
}

export interface Loaded {
  state: PluginState | null;
  kv: [string, Uint8Array][];
  secrets: [string, string][];
}

export interface Store {
  /** False when nothing outlives the page. */
  readonly persistent: boolean;
  /** False when secrets (sign-ins) do not outlive the page. */
  readonly keepsSecrets: boolean;
  load(plugin: string): Promise<Loaded>;
  saveState(plugin: string, state: PluginState): Promise<void>;
  setKv(plugin: string, key: string, value: Uint8Array | null): Promise<void>;
  setSecret(plugin: string, name: string, value: string | null): Promise<void>;
  /** Deletes the plugin's secrets (sign-out). */
  forgetSecrets(plugin: string): Promise<void>;
  getGlobal<T>(name: string): Promise<T | null>;
  setGlobal<T>(name: string, value: T): Promise<void>;
}

const DB = 'jellyfesh';
const VERSION = 1;
const STORES = ['state', 'kv', 'secrets', 'keys', 'global'] as const;

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

const encoder = new TextEncoder();
const decoder = new TextDecoder();

class IndexedStore implements Store {
  readonly persistent = true;
  private key: CryptoKey | null = null;
  /** Secrets of this visit when they cannot be encrypted. */
  private memory = new Map<string, Map<string, string>>();

  constructor(
    private readonly db: IDBDatabase,
    readonly keepsSecrets: boolean,
  ) {}

  private async secretKey(): Promise<CryptoKey> {
    if (this.key) return this.key;
    const tx = this.db.transaction('keys', 'readonly');
    const existing = (await request(tx.objectStore('keys').get('secrets'))) as CryptoKey | undefined;
    if (existing) {
      this.key = existing;
      return existing;
    }
    const created = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
    const write = this.db.transaction('keys', 'readwrite');
    write.objectStore('keys').put(created, 'secrets');
    await done(write);
    this.key = created;
    return created;
  }

  async load(plugin: string): Promise<Loaded> {
    const tx = this.db.transaction(['state', 'kv', 'secrets'], 'readonly');
    const range = IDBKeyRange.bound([plugin, ''], [plugin, '￿']);
    const [state, kvKeys, kvValues, secretKeys, secretValues] = await Promise.all([
      request(tx.objectStore('state').get(plugin)) as Promise<PluginState | undefined>,
      request(tx.objectStore('kv').getAllKeys(range)) as Promise<[string, string][]>,
      request(tx.objectStore('kv').getAll(range)) as Promise<Uint8Array[]>,
      request(tx.objectStore('secrets').getAllKeys(range)) as Promise<[string, string][]>,
      request(tx.objectStore('secrets').getAll(range)) as Promise<{ iv: Uint8Array; data: ArrayBuffer }[]>,
    ]);
    const secrets: [string, string][] = [...(this.memory.get(plugin) ?? [])];
    if (secretKeys.length && this.keepsSecrets) {
      const key = await this.secretKey();
      for (let i = 0; i < secretKeys.length; i++) {
        const name = secretKeys[i][1];
        try {
          const plain = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: secretValues[i].iv as BufferSource, additionalData: encoder.encode(`${plugin}/${name}`) },
            key,
            secretValues[i].data,
          );
          secrets.push([name, decoder.decode(plain)]);
        } catch {
          // A secret that no longer decrypts (another key) is as good as gone.
        }
      }
    }
    return {
      state: state ?? null,
      kv: kvKeys.map((k, i) => [k[1], kvValues[i]]),
      secrets,
    };
  }

  async saveState(plugin: string, state: PluginState): Promise<void> {
    const tx = this.db.transaction('state', 'readwrite');
    tx.objectStore('state').put(state, plugin);
    await done(tx);
  }

  async setKv(plugin: string, key: string, value: Uint8Array | null): Promise<void> {
    const tx = this.db.transaction('kv', 'readwrite');
    if (value) tx.objectStore('kv').put(value, [plugin, key]);
    else tx.objectStore('kv').delete([plugin, key]);
    await done(tx);
  }

  async setSecret(plugin: string, name: string, value: string | null): Promise<void> {
    if (!this.keepsSecrets) {
      let inner = this.memory.get(plugin);
      if (!inner) this.memory.set(plugin, (inner = new Map()));
      if (value !== null) inner.set(name, value);
      else inner.delete(name);
      return;
    }
    let record: { iv: Uint8Array; data: ArrayBuffer } | null = null;
    if (value !== null) {
      const key = await this.secretKey();
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const data = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv, additionalData: encoder.encode(`${plugin}/${name}`) },
        key,
        encoder.encode(value),
      );
      record = { iv, data };
    }
    const tx = this.db.transaction('secrets', 'readwrite');
    if (record) tx.objectStore('secrets').put(record, [plugin, name]);
    else tx.objectStore('secrets').delete([plugin, name]);
    await done(tx);
  }

  async forgetSecrets(plugin: string): Promise<void> {
    this.memory.delete(plugin);
    const tx = this.db.transaction('secrets', 'readwrite');
    tx.objectStore('secrets').delete(IDBKeyRange.bound([plugin, ''], [plugin, '￿']));
    await done(tx);
  }

  async getGlobal<T>(name: string): Promise<T | null> {
    const tx = this.db.transaction('global', 'readonly');
    return ((await request(tx.objectStore('global').get(name))) as T | undefined) ?? null;
  }

  async setGlobal<T>(name: string, value: T): Promise<void> {
    const tx = this.db.transaction('global', 'readwrite');
    tx.objectStore('global').put(value, name);
    await done(tx);
  }
}

/** Keeps everything in memory (no IndexedDB). */
class MemoryStore implements Store {
  readonly persistent = false;
  readonly keepsSecrets = false;
  private states = new Map<string, PluginState>();
  private kv = new Map<string, Map<string, Uint8Array>>();
  private secrets = new Map<string, Map<string, string>>();
  private globals = new Map<string, unknown>();

  private of<T>(map: Map<string, Map<string, T>>, plugin: string): Map<string, T> {
    let inner = map.get(plugin);
    if (!inner) map.set(plugin, (inner = new Map()));
    return inner;
  }

  async load(plugin: string): Promise<Loaded> {
    return {
      state: this.states.get(plugin) ?? null,
      kv: [...this.of(this.kv, plugin)],
      secrets: [...this.of(this.secrets, plugin)],
    };
  }

  async saveState(plugin: string, state: PluginState): Promise<void> {
    this.states.set(plugin, structuredClone(state));
  }

  async setKv(plugin: string, key: string, value: Uint8Array | null): Promise<void> {
    if (value) this.of(this.kv, plugin).set(key, value);
    else this.of(this.kv, plugin).delete(key);
  }

  async setSecret(plugin: string, name: string, value: string | null): Promise<void> {
    if (value !== null) this.of(this.secrets, plugin).set(name, value);
    else this.of(this.secrets, plugin).delete(name);
  }

  async forgetSecrets(plugin: string): Promise<void> {
    this.secrets.delete(plugin);
  }

  async getGlobal<T>(name: string): Promise<T | null> {
    return (this.globals.get(name) as T | undefined) ?? null;
  }

  async setGlobal<T>(name: string, value: T): Promise<void> {
    this.globals.set(name, value);
  }
}

/** The store: IndexedDB with encrypted secrets, or memory when the browser offers no storage. */
export async function openStore(): Promise<Store> {
  try {
    const open = indexedDB.open(DB, VERSION);
    open.onupgradeneeded = () => {
      for (const name of STORES) {
        if (!open.result.objectStoreNames.contains(name)) open.result.createObjectStore(name);
      }
    };
    const db = await request(open);
    // WebCrypto exists only in secure contexts (https, or localhost).
    return new IndexedStore(db, typeof crypto !== 'undefined' && crypto.subtle !== undefined);
  } catch {
    return new MemoryStore();
  }
}

/** A fresh plugin state. */
export function defaultState(official: boolean): PluginState {
  return { enabled: official, strikes: 0, disabledReason: null, account: null, settings: null, acknowledged: false };
}
