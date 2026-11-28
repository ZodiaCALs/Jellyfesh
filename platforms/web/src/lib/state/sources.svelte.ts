// The sources (bundled plugins and those installed from files) as the views see them, sign-in flows, and errors in words a person
// can act on, including the browser-only ones: CORS and mixed content (docs/web.md), services that refuse browsers and the relay
// that reaches them (docs/web-relay.md, ADR-0022 §7).

import type { Jf } from '../core/wasm';
import type { AuthKind, AuthStep, Capability, FieldValue, FlowAction, SignatureStatus } from '../core/types';
import { t } from '../i18n/index.svelte';
import { CallError, LOGIN_MS, type Plugin, type PluginHost } from '../plugins/host';
import { fieldsToJco, flowStepFromJco, flowToJco } from '../plugins/convert';
import { diagnose, isMixedContent } from '../plugins/diagnose';
import { pingRelay, type RelayConfig, type RelayErrorCode } from '../plugins/relay';

export interface SourceView {
  id: string;
  name: string;
  /** An extension (Settings › Extensions): makes lyrics or changes the look. */
  extension: boolean;
  version: string;
  description: string | null;
  official: boolean;
  signature: SignatureStatus | null;
  enabled: boolean;
  account: string | null;
  /** Why calls are refused, for people. */
  blocked: string | null;
  communityOff: boolean;
  authKinds: AuthKind[];
  capabilities: Capability[];
  server: string | null;
  ready: boolean;
  /** The author, once the package has been read. */
  author: string | null;
  /** Installed from a file rather than bundled with the app. */
  fromFile: boolean;
  /** The service, when it does not let browsers connect to it directly. */
  refusing: string | null;
  /** Its requests go through the relay. */
  relayed: boolean;
}

/** What the ready rule reads of a source (a `Plugin`). */
export interface ReadySource {
  /** Why calls are refused now (turned off, a community plugin while they are not allowed, not loaded), or null. */
  readonly blocked: unknown;
  declares(capability: Capability): boolean;
  readonly state: { readonly account: string | null };
}

/**
 * Whether a source is ready to be asked for `capability` (ADR-0027 §3, the rule the core's
 * `SourceLibrary` uses): switched on and allowed (official, or community while community plugins
 * are allowed), declaring the capability, and signed in when it needs a sign-in (it declares
 * `Auth`). Search asks only ready sources; one that isn't set up yet is left out, and Settings ›
 * Sources says what it needs.
 */
export function isReady(source: ReadySource, capability: Capability): boolean {
  return source.blocked === null && source.declares(capability) && (source.state.account !== null || !source.declares('Auth'));
}

/** The server address in a plugin's settings, if it has one. */
export function serverOf(plugin: Plugin): string | null {
  const value = plugin.settings.server_url;
  return typeof value === 'string' && value ? value : null;
}

/** A problem reaching `server`, in words, or null when it answers. */
export async function reachability(server: string): Promise<string | null> {
  switch (await diagnose(server)) {
    case 'ok':
      return null;
    case 'invalid':
      return t('Enter the full address, starting with http:// or https://.');
    case 'mixed-content':
      return t(
        'This page is on https, so the browser does not let it reach the http address {server}. Put the server behind https, or open Jellyfesh Web from the same network over http (see “Hosting it yourself”).',
        { server },
      );
    case 'cors':
      return t(
        'The server answered, but it does not allow requests from {origin}. Allow this address in the server’s CORS settings (see “Connecting to your server”).',
        { origin: location.origin },
      );
    default:
      return t('Could not reach {server}. Check the address, and that the server is running and reachable from this device.', {
        server,
      });
  }
}

/** "{Service} doesn't let browsers connect to it directly …" (docs/ui.md), before signing in. */
export function refusal(service: string): string {
  return t(
    '{service} doesn’t let browsers connect to it directly. Use the Jellyfesh app on your computer or phone, or set up a relay in Settings › Sources › Relay.',
    { service },
  );
}

/** The service of a source that refuses browsers, by the core's name for it or the source's own. */
export function serviceOf(plugin: Plugin): string {
  return plugin.refusingService ?? plugin.name;
}

/** Why the relay did not pass a request on, in words a person can act on. */
export function relayProblem(code: RelayErrorCode, address: string | null, detail = ''): string {
  switch (code) {
    case 'origin':
      return t('The relay doesn’t accept this page. Start it with --origin {origin}.', { origin: location.origin });
    case 'token':
      return t('The relay refused the token. Enter the token the relay was started with.');
    case 'unreachable':
      return t('Could not reach the relay at {address}. Check that it is running.', { address: address ?? '' });
    case 'host':
      return t('The relay doesn’t forward to this service. Start it with --allow for it, or update it.');
    case 'timeout':
      return t('The service did not answer the relay in time.');
    default:
      return t('The relay could not reach the service: {reason}', { reason: detail || code });
  }
}

/**
 * What a request that found no server says about the source, or null when it says nothing: the
 * relay's refusal; mixed content or CORS for the person's own server; and for a service's fixed
 * host that answers without letting this page read it, that the service refuses browsers (kept,
 * so its requests go through the relay from now on, when there is one).
 */
export async function networkProblem(plugin: Plugin): Promise<string | null> {
  const failure = plugin.networkError;
  if (!failure) return null;
  if (failure.kind === 'relay') return relayProblem(failure.reason ?? 'unreachable', plugin.relay?.address ?? null);
  const server = serverOf(plugin);
  if (failure.kind === 'mixed-content' || (server && isMixedContent(new URL(server)))) {
    return reachability(server ?? failure.url);
  }
  let origin: string | null = null;
  try {
    origin = new URL(failure.url).origin;
  } catch {
    return null;
  }
  if (server && new URL(server).origin === origin) return reachability(server);
  if ((await diagnose(failure.url)) !== 'cors') return null;
  await plugin.learnRefusesBrowsers();
  return plugin.relay
    ? t('{service} doesn’t let browsers connect to it directly, so Jellyfesh Web now goes through your relay for it. Try again.', {
        service: serviceOf(plugin),
      })
    : refusal(serviceOf(plugin));
}

/** Why a call failed, for people: browser-level network problems are diagnosed. */
export async function explain(error: unknown, plugin?: Plugin): Promise<string> {
  if (error instanceof CallError) {
    if (plugin?.networkError?.kind === 'relay' || (plugin?.networkError && !serverOf(plugin))) {
      const problem = await networkProblem(plugin);
      if (problem) return problem;
    }
    const network = error.plugin !== null && typeof error.plugin === 'object' && 'Network' in error.plugin;
    if (plugin && (network || plugin.networkError)) {
      const server = serverOf(plugin);
      if (plugin.networkError?.kind === 'mixed-content' || (server && isMixedContent(new URL(server)))) {
        return (await reachability(server ?? plugin.networkError!.url)) ?? error.message;
      }
      if (server) {
        const problem = await reachability(server);
        if (problem) return problem;
      }
    }
    switch (error.kind) {
      case 'timeout':
        return t('{name} did not answer in time.', { name: plugin?.name ?? '' });
      case 'community-disabled':
        return t('Community plugins are turned off.');
      case 'disabled':
        return error.message;
      case 'plugin':
        if (error.plugin === 'AuthRequired') return t('Sign in again to {name}.', { name: plugin?.name ?? '' });
        return error.message;
      default:
        return error.message;
    }
  }
  return error instanceof Error ? error.message : String(error);
}

/** A service in Sources → Add a Source (the list the apps share, `source_presets`). */
export interface PresetView {
  id: string;
  name: string;
  /** The bundled official plugin that connects to it. */
  plugin: string;
  /** An address to show as an example in the server field. */
  example: string | null;
  account: string | null;
}

interface PresetDef {
  id: string;
  name: string;
  plugin: string;
  address_setting: string | null;
  address_example: string | null;
}

export class Sources {
  list = $state.raw<SourceView[]>([]);
  /** The services offered in Add a Source: those whose official plugin is bundled. */
  presets = $state.raw<PresetView[]>([]);
  private presetDefs: PresetDef[] | null = null;
  /** Bumps when accounts or availability change, so views reload what they show. */
  revision = $state(0);
  /** Community plugins may run (after the disclosure). */
  allowCommunity = $state(false);
  /** The relay set up in Settings › Sources › Relay (its address; the token stays with the host). */
  relay = $state.raw<{ address: string } | null>(null);
  /** How the last check of the relay went: checking, connected, or why not. */
  relayStatus = $state.raw<{ state: 'checking' } | { state: 'connected' } | { state: 'problem'; message: string } | null>(null);

  constructor(
    readonly jf: Jf,
    readonly host: PluginHost,
  ) {
    host.onChange(() => this.refresh());
  }

  /** Loads the list and stored state; packages are verified when a plugin first runs. */
  async init() {
    await this.host.load();
    this.refresh();
  }

  refresh() {
    this.list = this.host.plugins.map((p) => {
      const blocked = p.blocked;
      return {
        id: p.id,
        name: p.name,
        extension: p.info?.extension ?? p.entry.extension ?? false,
        version: p.info?.version ?? p.entry.version,
        description: p.info?.description ?? p.entry.description ?? null,
        official: p.official,
        signature: p.info?.signature ?? null,
        enabled: p.state.enabled,
        account: p.state.account,
        blocked: blocked?.message ?? null,
        communityOff: blocked?.kind === 'community-disabled',
        authKinds: p.authKinds,
        capabilities: p.info?.capabilities ?? p.entry.capabilities ?? [],
        server: serverOf(p),
        ready: p.info !== null,
        author: p.info?.author ?? p.entry.installed?.info.author ?? null,
        fromFile: p.entry.installed !== null,
        refusing: p.needsRelay ? serviceOf(p) : null,
        relayed: p.relay !== null,
      };
    });
    this.relay = this.host.relay ? { address: this.host.relay.address } : null;
    this.presetDefs ??= JSON.parse(this.jf.source_presets()) as PresetDef[];
    this.presets = this.presetDefs.flatMap((def) => {
      const plugin = this.host.plugins.find((p) => p.id === def.plugin && p.official);
      if (!plugin) return [];
      return [{ id: def.id, name: def.name, plugin: def.plugin, example: def.address_example, account: plugin.state.account }];
    });
    this.allowCommunity = this.host.allowCommunity;
    this.revision += 1;
  }

  get(id: string): Plugin | undefined {
    return this.host.get(id);
  }

  /** The sources ready to answer `capability` now (`isReady`): on, allowed, and signed in when they need it. */
  usable(capability: Capability): Plugin[] {
    return this.host.plugins.filter((p) => isReady(p, capability));
  }

  async signOut(plugin: Plugin) {
    await plugin.logout();
    this.refresh();
  }

  /** Installs or updates a plugin from a `.jfpkg` file; throws why it was refused. */
  async install(file: File): Promise<Plugin> {
    return this.host.install(file);
  }

  async uninstall(plugin: Plugin) {
    await this.host.uninstall(plugin);
  }

  /**
   * Settings › Sources › Relay: keeps the relay (an empty address removes it) and checks that it
   * answers this page with this token. Returns why the address cannot be used, or null; how the
   * check went is in `relayStatus`.
   */
  async saveRelay(address: string, token: string): Promise<string | null> {
    if (address.trim() === '') {
      await this.host.setRelay(null);
      this.relayStatus = null;
      return null;
    }
    let normal: string;
    try {
      normal = this.jf.relay_address(address, location.protocol === 'https:');
    } catch (error) {
      switch ((error as Error).message) {
        case 'credentials':
          return t('Leave the user name and password out of the address; the relay uses its token.');
        case 'query':
          return t('Enter the relay’s address without ? or #.');
        case 'mixed-content':
          return t('This page is on https, so the browser only lets it reach a relay on https, or one on this computer (http://127.0.0.1).');
        default:
          return t('Enter the relay’s full address, starting with http:// or https://.');
      }
    }
    // An empty token keeps the saved one (the field never shows it).
    const kept = token.trim() || this.host.relay?.token || '';
    if (kept === '') return t('Enter the token the relay was started with.');
    const relay: RelayConfig = { address: normal, token: kept };
    await this.host.setRelay(relay);
    await this.checkRelay();
    return null;
  }

  /** Checks the relay that is set up (Settings shows how it went). */
  async checkRelay() {
    const relay = this.host.relay;
    if (!relay) {
      this.relayStatus = null;
      return;
    }
    this.relayStatus = { state: 'checking' };
    const problem = await pingRelay(relay);
    if (this.host.relay !== relay) return;
    this.relayStatus = problem
      ? { state: 'problem', message: relayProblem(problem.code, relay.address, problem.message) }
      : { state: 'connected' };
  }

  /** Reads every package not read yet (for versions and authors), without running the plugins. */
  async verifyAll() {
    await Promise.all(this.host.plugins.map((p) => p.verify().catch(() => undefined)));
  }
}

export type SignInStage =
  /** The service does not let browsers connect, and no relay is set up: say so before trying. */
  | { stage: 'refused'; service: string }
  | { stage: 'settings'; error: string | null; busy: boolean }
  | { stage: 'choose' }
  | { stage: 'step'; step: AuthStep; busy: boolean; error: string | null }
  | { stage: 'done'; account: string }
  | { stage: 'failed'; message: string };

/** One sign-in: the server address first, then the flow the person picks, driven by the core. */
export class SignInFlow {
  view = $state<SignInStage>({ stage: 'settings', error: null, busy: false });
  kind = $state<AuthKind | null>(null);
  private machine: InstanceType<Jf['SignIn']> | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private wake: (() => void) | null = null;
  private over = false;

  constructor(
    private readonly jf: Jf,
    readonly plugin: Plugin,
    private readonly sources: Sources,
  ) {
    if (plugin.needsRelay && !plugin.relay) this.view = { stage: 'refused', service: serviceOf(plugin) };
    else if (serverOf(plugin)) this.view = { stage: 'choose' };
  }

  get kinds(): AuthKind[] {
    // A browser cannot catch a loopback redirect, and cookies cannot be imported into a page.
    return this.plugin.authKinds.filter((k) => k !== 'Oauth' && k !== 'CookieImport');
  }

  /** Saves the settings form and checks that the server can be reached from this page. */
  async saveSettings(values: Record<string, unknown>) {
    this.view = { stage: 'settings', error: null, busy: true };
    try {
      await this.plugin.setSettings({ ...this.plugin.settings, ...values });
    } catch (error) {
      this.view = { stage: 'settings', error: await explain(error, this.plugin), busy: false };
      return;
    }
    const server = serverOf(this.plugin);
    const problem = server ? await reachability(server) : null;
    if (problem) {
      this.view = { stage: 'settings', error: problem, busy: false };
      return;
    }
    this.view = { stage: 'choose' };
    if (this.kinds.length === 1) void this.start(this.kinds[0]);
  }

  editSettings() {
    this.stop();
    this.view = { stage: 'settings', error: null, busy: false };
  }

  async start(kind: AuthKind) {
    this.stop();
    this.over = false;
    this.kind = kind;
    if (this.plugin.needsRelay && !this.plugin.relay) {
      this.view = { stage: 'refused', service: serviceOf(this.plugin) };
      return;
    }
    this.plugin.networkError = null;
    if (!this.plugin.info) {
      this.view = { stage: 'step', step: { Waiting: { message: '' } }, busy: true, error: null };
      try {
        await this.plugin.verify();
      } catch (error) {
        this.view = { stage: 'failed', message: await explain(error, this.plugin) };
        return;
      }
    }
    const info = this.plugin.info;
    if (!info || this.over) return;
    const id = `web-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    this.machine = new this.jf.SignIn(kind, id, JSON.stringify(info.allow), JSON.stringify(this.plugin.settings));
    void this.run(JSON.parse(this.machine.start()) as FlowAction);
  }

  /** What the person typed for an AskFields step. */
  submit(fields: FieldValue[]) {
    if (!this.machine || this.view.stage !== 'step') return;
    this.view = { ...this.view, busy: true, error: null };
    let action: FlowAction;
    try {
      action = JSON.parse(this.machine.submit(JSON.stringify(fields), Date.now()));
    } catch (error) {
      this.view = { ...this.view, busy: false, error: (error as Error).message };
      return;
    }
    void this.run(action);
  }

  private async run(action: FlowAction) {
    const machine = this.machine;
    while (!this.over && machine === this.machine && machine) {
      switch (action.next) {
        case 'call': {
          if (this.view.stage === 'step') this.view = { ...this.view, busy: true };
          let step: string | null = null;
          let failure: string | null = null;
          try {
            const flow = flowToJco(action.flow);
            const args = action.call === 'submit' ? [flow, fieldsToJco(action.fields)] : [flow];
            const value = await this.plugin.call(`auth.${action.call}`, args, {
              wallMs: LOGIN_MS,
              protect: action.protect,
              keystore: action.keystore,
            });
            step = JSON.stringify(flowStepFromJco(value));
          } catch (error) {
            failure = await explain(error, this.plugin);
          }
          // A password went through this instance: start a fresh one (ADR-0015).
          if (action.restart) this.plugin.stop();
          if (this.over || machine !== this.machine) return;
          action = JSON.parse(machine.reply(step, failure, Date.now()));
          break;
        }
        case 'poll': {
          this.view = { stage: 'step', step: action.step, busy: false, error: null };
          await new Promise<void>((resolve) => {
            this.wake = resolve;
            this.timer = setTimeout(resolve, action.next === 'poll' ? action.after_ms : 0);
          });
          this.wake = null;
          if (this.over || machine !== this.machine) return;
          action = JSON.parse(machine.wake(Date.now()));
          break;
        }
        case 'form':
        case 'redirect':
          this.view = { stage: 'step', step: action.step, busy: false, error: this.errorOf(action.step) };
          return;
        case 'finish': {
          const step = action.step;
          if ('Done' in step) {
            this.plugin.state.account = step.Done.account;
            this.plugin.state.enabled = true;
            await this.plugin.save();
            this.sources.refresh();
            this.view = { stage: 'done', account: step.Done.account };
          } else if ('Failed' in step) {
            // A plugin that could not reach its service says so in its own words; the page
            // knows why a browser could not (CORS, the relay) and says that instead.
            const problem = await networkProblem(this.plugin);
            if (this.over || machine !== this.machine) return;
            this.view =
              problem === null
                ? { stage: 'failed', message: step.Failed.message }
                : this.plugin.needsRelay && !this.plugin.relay
                  ? { stage: 'refused', service: serviceOf(this.plugin) }
                  : { stage: 'failed', message: problem };
          } else {
            this.view = { stage: 'failed', message: t('The sign-in failed.') };
          }
          this.stop();
          return;
        }
      }
    }
  }

  private errorOf(step: AuthStep): string | null {
    return 'Failed' in step ? step.Failed.message : null;
  }

  /** Ends the flow (the sheet closed, or another kind started). */
  stop() {
    this.over = true;
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.wake?.();
    this.machine?.free();
    this.machine = null;
  }
}
