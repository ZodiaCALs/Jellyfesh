// The plugin host in the browser: the bundled official plugins, each in its own Web Worker
// (./worker.ts), with the native host's rules where a browser allows them (ADR-0003, ADR-0004;
// differences in docs/web.md):
//
// * a plugin's calls run one at a time, in order;
// * every call has a wall-clock budget (10 s, 30 s for login steps, 8 s for search): when it runs
//   out, the worker is terminated and started afresh on the next call. A trap (crash) or more than
//   64 MiB of memory after a call restarts it too. Each is a strike; three in a row turn the plugin
//   off with a visible reason. (There is no separate CPU budget: a browser cannot interrupt
//   WebAssembly except by terminating its worker.)
// * the host calls only what the manifest declares and the plugin reports, and checks every answer
//   with jf-web (jf_plugin_model::sanitize): pages cut and cleaned, stream and artwork URLs only
//   from the allowlist;
// * packages not signed with the Jellyfesh release key are Community plugins: off until community
//   plugins are allowed and the person accepts the disclosure (ADR-0012);
// * sign-out deletes the plugin's secrets and account whatever the plugin answers (ADR-0015).

import type { Jf } from '../core/wasm';
import type {
  AllowRule,
  AuthKind,
  Capability,
  ImageRef,
  Item,
  ItemPage,
  Lyrics,
  PackageInfo,
  PluginError,
  Quality,
  StreamInfo,
} from '../core/types';
import {
  capabilityFromJco,
  imageFromJco,
  itemToJco,
  lyricsFromJco,
  pageFromJco,
  pageRequestToJco,
  pluginErrorFromJco,
  pluginErrorText,
  qualityToJco,
  streamFromJco,
} from './convert';
import { preparePackage } from './install';
import type { CallFailure, FromWorker, ToWorker } from './protocol';
import { defaultState, type InstalledPlugin, type PluginState, type Store } from './storage';

/** Wall-clock budgets (ADR-0004). */
export const CALL_MS = 10_000;
export const LOGIN_MS = 30_000;
export const SEARCH_MS = 8_000;
const START_MS = 30_000;
/** The page waits this much longer than the worker's own deadline before terminating it. */
const GRACE_MS = 1_000;
const MEMORY_LIMIT = 64 * 1024 * 1024;
const STRIKES = 3;

/** One bundled plugin as plugins/index.json lists it. */
export interface BundledPlugin {
  id: string;
  name: string;
  version: string;
  official: boolean;
  /** From the package at build time, for the views until a worker verified it. */
  description: string | null;
  capabilities: Capability[];
  auth_kinds: AuthKind[];
  package: string;
  module: string;
  wasm_sha256: string;
}

/** A plugin's files: bundled with the app (URLs under plugins/), or installed from a file. */
export interface PluginEntry extends BundledPlugin {
  installed: InstalledPlugin | null;
}

function bundledEntry(bundled: BundledPlugin): PluginEntry {
  return { ...bundled, installed: null };
}

function installedEntry(installed: InstalledPlugin): PluginEntry {
  const info = installed.info;
  return {
    id: info.id,
    name: info.name,
    version: info.version,
    official: info.official,
    description: info.description,
    capabilities: info.capabilities,
    auth_kinds: info.auth_kinds,
    package: `${info.id}.jfpkg`,
    module: 'plugin.js',
    wasm_sha256: info.wasm_sha256,
    installed,
  };
}

export type CallErrorKind =
  | 'not-installed'
  | 'disabled'
  | 'community-disabled'
  | 'not-declared'
  | 'plugin'
  | 'timeout'
  | 'crashed'
  | 'memory'
  | 'load'
  | 'invalid-response';

/** Why a call produced no result (the native `CallError`). */
export class CallError extends Error {
  constructor(
    readonly kind: CallErrorKind,
    message: string,
    readonly plugin: PluginError | null = null,
  ) {
    super(message);
  }

  /** A limit violation: three in a row turn the plugin off. */
  get isViolation(): boolean {
    return this.kind === 'timeout' || this.kind === 'crashed' || this.kind === 'memory';
  }
}

interface Pending {
  resolve(value: unknown): void;
  reject(error: CallError): void;
  timer: ReturnType<typeof setTimeout>;
}

export interface CallOptions {
  wallMs?: number;
  protect?: string[];
  keystore?: boolean;
}

/** A plugin: its package, state, worker and calls. */
export class Plugin {
  info: PackageInfo | null = null;
  /** What the plugin reports it can do now, limited to what it declares. */
  capabilities: string[] = [];
  state: PluginState;
  /** Why the worker could not start, if it could not. */
  loadError: string | null = null;
  /** The last request that found no server: origin and path, and why. */
  networkError: { url: string; kind: 'mixed-content' | 'network' } | null = null;
  private worker: Worker | null = null;
  /** The worker runs the plugin (not only verified its package). */
  private running = false;
  private starting: Promise<void> | null = null;
  private seq = 0;
  private pending = new Map<number, Pending>();
  private queue: Promise<unknown> = Promise.resolve();
  private kv: [string, Uint8Array][] = [];
  private secrets: [string, string][] = [];
  private source: InstanceType<Jf['Source']> | null = null;
  /** Capabilities the plugin reported but its manifest does not declare (an install refuses it). */
  undeclared: string[] = [];
  /** Object URLs of an installed plugin's files, made on its first start. */
  private blobs: { packageUrl: string; moduleUrl: string; coreUrls: Record<string, string> } | null = null;

  constructor(
    readonly entry: PluginEntry,
    private readonly host: PluginHost,
    loaded: { state: PluginState | null; kv: [string, Uint8Array][]; secrets: [string, string][] },
  ) {
    this.state = loaded.state ?? defaultState(entry.official);
    this.kv = loaded.kv;
    this.secrets = loaded.secrets;
  }

  get id(): string {
    return this.entry.id;
  }

  get name(): string {
    return this.info?.name ?? this.entry.name;
  }

  /** Signed by the Jellyfesh release key (verified by the worker; the build's word until then). */
  get official(): boolean {
    return this.info?.official ?? this.entry.official;
  }

  /** The settings in use: stored values fitted to the schema, or the defaults. */
  get settings(): Record<string, unknown> {
    if (!this.info) return this.state.settings ?? {};
    const json = this.host.jf.effective_settings(
      JSON.stringify(this.info.settings_schema),
      this.state.settings ? JSON.stringify(this.state.settings) : undefined,
    );
    return JSON.parse(json);
  }

  /** Why calls are refused right now, or null. */
  get blocked(): CallError | null {
    if (this.loadError) return new CallError('load', this.loadError);
    if (!this.official && !this.host.allowCommunity) {
      return new CallError('community-disabled', 'Community plugins are turned off.');
    }
    if (!this.state.enabled) {
      return new CallError('disabled', this.state.disabledReason ?? 'Turned off in Sources.');
    }
    return null;
  }

  /** Whether the manifest declares `capability` (the verified one once a worker read it). */
  declares(capability: string): boolean {
    return (this.info?.capabilities ?? this.entry.capabilities ?? []).includes(capability as never);
  }

  /** The sign-in kinds the manifest lists. */
  get authKinds(): AuthKind[] {
    return this.info?.auth_kinds ?? this.entry.auth_kinds ?? [];
  }

  /** Reads and verifies the package, without running the plugin, if that is not done yet. */
  async verify(): Promise<void> {
    if (this.info) return;
    await this.start(false);
  }

  /** Starts the worker: runs the plugin (`instantiate`), or only verifies its package. */
  async start(instantiate = true): Promise<void> {
    while (this.starting) await this.starting.catch(() => undefined);
    if (this.worker && (this.running || !instantiate)) return;
    if (this.worker) this.stop();
    this.starting = new Promise<void>((resolve, reject) => {
      const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module', name: `plugin ${this.id}` });
      this.worker = worker;
      const timer = setTimeout(() => {
        this.stop();
        reject(new CallError('timeout', 'The plugin did not start in time.'));
      }, START_MS);
      worker.onmessage = (event: MessageEvent<FromWorker>) => {
        const message = event.data;
        if (message.type === 'ready') {
          clearTimeout(timer);
          this.running = instantiate;
          this.ready(message.info, message.capabilities, instantiate);
          if (!instantiate) {
            // Verified; the worker is not needed until the plugin runs.
            this.stop();
          }
          resolve();
        } else if (message.type === 'fatal') {
          clearTimeout(timer);
          this.loadError = message.message;
          this.stop();
          reject(new CallError('load', message.message));
        } else {
          this.receive(message);
        }
      };
      worker.onerror = (event) => {
        clearTimeout(timer);
        this.loadError = event.message || 'The plugin could not be loaded.';
        this.stop();
        reject(new CallError('load', this.loadError));
      };
      const urls = this.urls();
      const init: ToWorker = {
        type: 'init',
        plugin: this.id,
        packageUrl: urls.packageUrl,
        moduleUrl: urls.moduleUrl,
        coreUrls: urls.coreUrls,
        wasmSha256: this.entry.wasm_sha256,
        settings: JSON.stringify(this.state.settings ?? {}),
        kv: this.kv,
        secrets: this.secrets,
        startMs: START_MS,
        instantiate,
      };
      worker.postMessage(init);
    }).finally(() => {
      this.starting = null;
    });
    return this.starting;
  }

  private ready(info: PackageInfo, reported: string[], instantiated: boolean) {
    this.info = info;
    this.loadError = null;
    if (instantiated) {
      const declared = new Set<string>(info.capabilities);
      const all = reported.map(capabilityFromJco);
      this.capabilities = all.filter((c) => declared.has(c));
      this.undeclared = all.filter((c) => !declared.has(c));
    }
    // Never changed by the person: official packages are on, others off (ADR-0012).
    if (!this.host.known(this.id)) this.state.enabled = info.official;
    this.rebuildSource();
    this.host.changed();
  }

  private rebuildSource() {
    if (!this.info) return;
    this.source?.free();
    this.source = new this.host.jf.Source(
      this.id,
      this.info.name,
      JSON.stringify(this.info.allow),
      JSON.stringify(this.settings),
      JSON.stringify(this.info.content_policy),
      JSON.stringify(this.info.links ?? null),
    );
  }

  private receive(message: FromWorker) {
    switch (message.type) {
      case 'result': {
        const pending = this.pending.get(message.seq);
        if (!pending) return;
        this.pending.delete(message.seq);
        clearTimeout(pending.timer);
        if (message.memory > MEMORY_LIMIT) {
          this.stop();
          pending.reject(new CallError('memory', 'The plugin went over its memory limit.'));
        } else if (message.ok) {
          pending.resolve(message.value);
        } else {
          pending.reject(this.failure(message.error));
        }
        break;
      }
      case 'kv':
        this.kv = this.kv.filter(([k]) => k !== message.key);
        if (message.value) this.kv.push([message.key, message.value]);
        void this.host.store.setKv(this.id, message.key, message.value);
        break;
      case 'secret':
        this.secrets = this.secrets.filter(([k]) => k !== message.name);
        if (message.value !== null) this.secrets.push([message.name, message.value]);
        void this.host.store.setSecret(this.id, message.name, message.value);
        break;
      case 'log':
        this.host.log(this.id, message.level, message.message);
        break;
      case 'network-error':
        this.networkError = { url: message.url, kind: message.kind };
        break;
    }
  }

  private failure(error: CallFailure): CallError {
    if (error.kind === 'plugin') {
      const plugin = pluginErrorFromJco(error.payload);
      return new CallError('plugin', pluginErrorText(plugin), plugin);
    }
    this.stop();
    return new CallError('crashed', `The plugin crashed: ${error.message}`);
  }

  /** Terminates the worker; the next call starts a fresh one. */
  stop() {
    this.worker?.terminate();
    this.worker = null;
    this.running = false;
    for (const [, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(new CallError('crashed', 'The plugin was restarted.'));
    }
    this.pending.clear();
  }

  /** Where the worker reads the package and the transpiled component from. */
  private urls(): { packageUrl: string; moduleUrl: string; coreUrls: Record<string, string> | null } {
    const installed = this.entry.installed;
    if (!installed) {
      const base = new URL('plugins/', document.baseURI);
      return {
        packageUrl: new URL(this.entry.package, base).href,
        moduleUrl: new URL(this.entry.module, base).href,
        coreUrls: null,
      };
    }
    if (!this.blobs) {
      const url = (bytes: Uint8Array, type: string) => URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
      this.blobs = {
        packageUrl: url(installed.package, 'application/octet-stream'),
        moduleUrl: url(installed.module, 'text/javascript'),
        coreUrls: Object.fromEntries(installed.cores.map(([name, bytes]) => [name, url(bytes, 'application/wasm')])),
      };
    }
    return this.blobs;
  }

  /** What an update keeps: the stored state, kv values and secrets. */
  get kept(): { state: PluginState; kv: [string, Uint8Array][]; secrets: [string, string][] } {
    return { state: this.state, kv: this.kv, secrets: this.secrets };
  }

  /** Stops the plugin for good (uninstalled or replaced) and frees its files' URLs. */
  dispose() {
    this.stop();
    this.source?.free();
    this.source = null;
    if (this.blobs) {
      URL.revokeObjectURL(this.blobs.packageUrl);
      URL.revokeObjectURL(this.blobs.moduleUrl);
      for (const url of Object.values(this.blobs.coreUrls)) URL.revokeObjectURL(url);
      this.blobs = null;
    }
  }

  /** Calls `method` (jco's `interface.function`) with jco-shaped arguments. */
  call<T>(method: string, args: unknown[], options: CallOptions = {}): Promise<T> {
    const run = async (): Promise<T> => {
      const blocked = this.blocked;
      if (blocked) throw blocked;
      await this.start();
      // The verified package may say otherwise than the build's index.
      const refused = this.blocked;
      if (refused) throw refused;
      const capability = capabilityOf(method);
      if (capability && !this.declares(capability)) {
        throw new CallError('not-declared', `The plugin does not declare ${capability}.`);
      }
      const wallMs = options.wallMs ?? CALL_MS;
      try {
        const value = await new Promise<unknown>((resolve, reject) => {
          const seq = ++this.seq;
          const timer = setTimeout(() => {
            this.pending.delete(seq);
            this.stop();
            reject(new CallError('timeout', 'The plugin did not answer in time.'));
          }, wallMs + GRACE_MS);
          this.pending.set(seq, { resolve, reject, timer });
          const message: ToWorker = {
            type: 'call',
            seq,
            method,
            args,
            wallMs,
            protect: options.protect ?? [],
            keystore: options.keystore ?? false,
          };
          this.worker!.postMessage(message);
        });
        await this.strike(false);
        return value as T;
      } catch (error) {
        if (error instanceof CallError && error.isViolation) await this.strike(true, error);
        throw error;
      }
    };
    const result = this.queue.then(run, run);
    this.queue = result.catch(() => undefined);
    return result;
  }

  private async strike(violation: boolean, error?: CallError) {
    if (!violation) {
      if (this.state.strikes !== 0) {
        this.state.strikes = 0;
        await this.save();
      }
      return;
    }
    this.state.strikes += 1;
    if (this.state.strikes >= STRIKES) {
      this.state.enabled = false;
      this.state.disabledReason = `Turned off after repeated problems: ${error?.message ?? 'limits'}`;
      this.stop();
    }
    await this.save();
    this.host.changed();
  }

  async save() {
    this.host.remember(this.id);
    await this.host.store.saveState(this.id, this.state);
  }

  /** Changes the settings after checking them against the schema; throws why they do not fit. */
  async setSettings(values: Record<string, unknown>) {
    if (!this.info) await this.start();
    const checked = JSON.parse(
      this.host.jf.validate_settings(JSON.stringify(this.info!.settings_schema), JSON.stringify(values)),
    ) as Record<string, unknown>;
    this.state.settings = checked;
    await this.save();
    this.rebuildSource();
    const message: ToWorker = {
      type: 'settings',
      settings: JSON.stringify(checked),
      allow: this.info!.allow as AllowRule[],
      maxRps: this.info!.max_rps,
    };
    this.worker?.postMessage(message);
    this.host.changed();
  }

  async setEnabled(enabled: boolean) {
    this.state.enabled = enabled;
    if (enabled) {
      this.state.strikes = 0;
      this.state.disabledReason = null;
    }
    await this.save();
    this.host.changed();
  }

  /** Signs out: asks the plugin to revoke, then deletes its secrets and account regardless. */
  async logout() {
    try {
      if (!this.blocked && this.declares('Auth')) await this.call('auth.logout', [], { wallMs: LOGIN_MS });
    } catch {
      // The credentials go either way.
    }
    this.secrets = [];
    await this.host.store.forgetSecrets(this.id);
    this.state.account = null;
    await this.save();
    this.stop();
    this.host.changed();
  }

  // ---- calls with checked answers -------------------------------------------------------------

  private checker(): InstanceType<Jf['Source']> {
    if (!this.source) throw new CallError('load', 'The plugin is not loaded.');
    return this.source;
  }

  /** Whether the allowlist covers `url` (pages and images the plugin names). */
  allows(url: string): boolean {
    return this.source?.allows(url) ?? false;
  }

  /**
   * The https link that opens `item` in the service, filled in by the core from the manifest's
   * `[links]` (API 1.1), or null: Open in {Service}. Known once the package has been read.
   */
  link(item: Item): string | null {
    return this.source?.link(item.kind, item.id) ?? null;
  }

  async search(query: string, limit = 50, wallMs = SEARCH_MS): Promise<ItemPage> {
    const page = await this.call('catalog.search', [query, { kinds: [] }, pageRequestToJco(null, limit)], { wallMs });
    return JSON.parse(this.checker().check_page(JSON.stringify(pageFromJco(page)), limit));
  }

  async browse(node: Item | null, cursor: string | null, limit = 100): Promise<ItemPage> {
    const page = await this.call('catalog.browse', [node ? itemToJco(node) : undefined, pageRequestToJco(cursor, limit)]);
    return JSON.parse(this.checker().check_page(JSON.stringify(pageFromJco(page)), limit));
  }

  async resolveStream(item: Item, quality: Quality): Promise<StreamInfo> {
    const info = await this.call('playback.resolveStream', [itemToJco(item), qualityToJco(quality)]);
    try {
      return JSON.parse(this.checker().check_stream(JSON.stringify(streamFromJco(info))));
    } catch (error) {
      throw new CallError('invalid-response', (error as Error).message);
    }
  }

  async lyrics(item: Item): Promise<Lyrics | null> {
    const payload = await this.call<unknown>('lyrics.lyrics', [itemToJco(item)]);
    if (payload === undefined || payload === null) return null;
    return JSON.parse(this.checker().lyrics(JSON.stringify(lyricsFromJco(payload))));
  }

  async artwork(item: Item, size: number): Promise<ImageRef | null> {
    const image = await this.call<unknown>('artwork.artwork', [itemToJco(item), Math.min(3000, Math.max(64, size))]);
    if (image === undefined || image === null) return null;
    try {
      return JSON.parse(this.checker().check_artwork(JSON.stringify(imageFromJco(image))));
    } catch {
      return null;
    }
  }

  async recommendations(item: Item, limit = 50): Promise<ItemPage> {
    const page = await this.call('recommendations.forItem', [itemToJco(item), pageRequestToJco(null, limit)]);
    return JSON.parse(this.checker().check_page(JSON.stringify(pageFromJco(page)), limit));
  }
}

/** The capability a method needs (`catalog.search` needs Search). */
function capabilityOf(method: string): string | null {
  switch (method) {
    case 'catalog.search':
      return 'Search';
    case 'catalog.browse':
      return 'Browse';
    case 'playback.resolveStream':
      return 'ResolveStream';
    case 'lyrics.lyrics':
      return 'Lyrics';
    case 'artwork.artwork':
      return 'Artwork';
    case 'recommendations.forItem':
    case 'recommendations.forUser':
      return 'Recommendations';
    case 'auth.begin':
    case 'auth.poll':
    case 'auth.submit':
    case 'auth.logout':
      return 'Auth';
    default:
      return null;
  }
}

/** The plugins (bundled with the app or installed from files) and their shared policy. */
export class PluginHost {
  plugins: Plugin[] = [];
  allowCommunity = false;
  private remembered = new Set<string>();
  private listeners = new Set<() => void>();
  /** The ids the app bundles. */
  private bundled = new Set<string>();
  /** Bundled plugins the person uninstalled: not loaded again (as in the native apps). */
  private declined = new Set<string>();

  constructor(
    readonly jf: Jf,
    readonly store: Store,
  ) {}

  /**
   * Loads plugins/index.json, the plugins installed from files and each plugin's stored state;
   * workers start on demand. A package installed from a file stays until the app bundles a newer
   * version of it (signed the same way), as native apps update their bundled plugins.
   */
  async load(): Promise<void> {
    this.allowCommunity = (await this.store.getGlobal<boolean>('allowCommunity')) ?? false;
    this.declined = new Set((await this.store.getGlobal<string[]>('declined')) ?? []);
    let list: BundledPlugin[] = [];
    try {
      const response = await fetch(new URL('plugins/index.json', document.baseURI), { cache: 'no-cache' });
      if (response.ok) list = ((await response.json()) as { plugins: BundledPlugin[] }).plugins;
    } catch {
      // No plugins bundled (a build without them): files still play.
    }
    this.bundled = new Set(list.map((p) => p.id));
    const installed = await this.store.installed().catch(() => [] as InstalledPlugin[]);
    const entries: PluginEntry[] = [];
    for (const bundled of list) {
      const own = installed.find((p) => p.id === bundled.id);
      if (own) {
        if (own.info.official === bundled.official && this.jf.bundled_updates(own.info.version, bundled.version)) {
          await this.store.removeInstalled(own.id).catch(() => undefined);
          entries.push(bundledEntry(bundled));
        } else {
          entries.push(installedEntry(own));
        }
      } else if (!this.declined.has(bundled.id)) {
        entries.push(bundledEntry(bundled));
      }
    }
    for (const own of installed) {
      if (!this.bundled.has(own.id)) entries.push(installedEntry(own));
    }
    this.plugins = await Promise.all(
      entries.map(async (entry) => {
        const loaded = await this.store.load(entry.id);
        if (loaded.state) this.remembered.add(entry.id);
        // Sign-ins were not kept (no WebCrypto on this page): nobody is signed in any more.
        if (loaded.state && !this.store.keepsSecrets) loaded.state.account = null;
        return new Plugin(entry, this, loaded);
      }),
    );
    this.changed();
  }

  /**
   * Installs (or updates) a plugin from a `.jfpkg` file, as the native host does: checks the
   * package and its signature, refuses remote sources (no remote sessions in the browser,
   * ADR-0018), community packages while they are off and updates signed by another key (the
   * core's `install_refusal`), transpiles the component for this browser, starts it once and
   * checks the capabilities it reports. Official packages start on; community ones wait for the
   * disclosure. An update keeps the settings, account and secrets. Throws why it was refused.
   */
  async install(file: File): Promise<Plugin> {
    const limit = this.jf.max_package_bytes();
    if (file.size > limit) throw new Error(`the package is larger than ${Math.round(limit / 1024 / 1024)} MiB`);
    const bytes = new Uint8Array(await file.arrayBuffer());
    const prepared = await preparePackage(bytes);
    const info = prepared.info;
    const existing = this.get(info.id);
    if (existing) await existing.verify().catch(() => undefined);
    const refusal = this.jf.install_refusal(
      JSON.stringify({ id: info.id, name: info.name, signature: info.signature, playback_mode: info.playback_mode }),
      existing?.info ? JSON.stringify(existing.info.signature) : undefined,
      this.allowCommunity,
    );
    if (refusal) throw new Error(refusal);
    const record: InstalledPlugin = {
      id: info.id,
      info,
      package: bytes,
      module: prepared.module,
      cores: prepared.cores,
      installedAt: Date.now(),
    };
    const kept = existing?.kept ?? { state: null, kv: [], secrets: [] };
    const plugin = new Plugin(installedEntry(record), this, kept);
    try {
      await plugin.start(true);
    } catch (error) {
      plugin.dispose();
      throw new Error(`the plugin failed to start: ${error instanceof Error ? error.message : String(error)}`);
    }
    if (plugin.undeclared.length > 0) {
      plugin.dispose();
      throw new Error(
        `invalid manifest: the plugin reports capability ${plugin.undeclared[0]}, which the manifest does not declare`,
      );
    }
    plugin.stop();
    await this.store.saveInstalled(record);
    existing?.dispose();
    this.plugins = existing ? this.plugins.map((p) => (p === existing ? plugin : p)) : [...this.plugins, plugin];
    if (this.declined.delete(info.id)) await this.store.setGlobal('declined', [...this.declined]);
    await plugin.save();
    this.changed();
    return plugin;
  }

  /**
   * Removes a plugin with its settings, kv values and secrets. A bundled plugin stays removed
   * (it is not loaded again) until a package for it is installed from a file.
   */
  async uninstall(plugin: Plugin) {
    plugin.dispose();
    await this.store.forget(plugin.id);
    if (plugin.entry.installed) await this.store.removeInstalled(plugin.id);
    if (this.bundled.has(plugin.id)) {
      this.declined.add(plugin.id);
      await this.store.setGlobal('declined', [...this.declined]);
    }
    this.remembered.delete(plugin.id);
    this.plugins = this.plugins.filter((p) => p !== plugin);
    this.changed();
  }

  get(id: string): Plugin | undefined {
    return this.plugins.find((p) => p.id === id);
  }

  known(id: string): boolean {
    return this.remembered.has(id);
  }

  /** `plugin` has stored state: its defaults no longer apply. */
  remember(plugin: string) {
    this.remembered.add(plugin);
  }

  async setAllowCommunity(allow: boolean) {
    this.allowCommunity = allow;
    await this.store.setGlobal('allowCommunity', allow);
    this.changed();
  }

  /** The disclosure was accepted for `plugin`: community plugins on, and this one on. */
  async acknowledge(plugin: Plugin) {
    plugin.state.acknowledged = true;
    await this.setAllowCommunity(true);
    await plugin.setEnabled(true);
  }

  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  changed() {
    for (const listener of this.listeners) listener();
  }

  log(plugin: string, level: string, message: string) {
    const line = `[${plugin}] ${message}`;
    if (level === 'error') console.error(line);
    else if (level === 'warn') console.warn(line);
    else if (import.meta.env.DEV) console.debug(line);
  }
}
