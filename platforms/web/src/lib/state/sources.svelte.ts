// The sources (bundled plugins) as the views see them, sign-in flows, and errors in words a person
// can act on, including the browser-only ones: CORS and mixed content (docs/web.md).

import type { Jf } from '../core/wasm';
import type { AuthKind, AuthStep, Capability, FieldValue, FlowAction, SignatureStatus } from '../core/types';
import { t } from '../i18n/index.svelte';
import { CallError, LOGIN_MS, type Plugin, type PluginHost } from '../plugins/host';
import { fieldsToJco, flowStepFromJco, flowToJco } from '../plugins/convert';
import { diagnose, isMixedContent } from '../plugins/diagnose';

export interface SourceView {
  id: string;
  name: string;
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

/** Why a call failed, for people: browser-level network problems are diagnosed. */
export async function explain(error: unknown, plugin?: Plugin): Promise<string> {
  if (error instanceof CallError) {
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

export class Sources {
  list = $state.raw<SourceView[]>([]);
  /** Bumps when accounts or availability change, so views reload what they show. */
  revision = $state(0);
  /** Community plugins may run (after the disclosure). */
  allowCommunity = $state(false);

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
      };
    });
    this.allowCommunity = this.host.allowCommunity;
    this.revision += 1;
  }

  get(id: string): Plugin | undefined {
    return this.host.get(id);
  }

  /** Sources that can answer `capability` now: on, signed in (or needing no sign-in). */
  usable(capability: Capability): Plugin[] {
    return this.host.plugins.filter(
      (p) =>
        p.blocked === null &&
        p.declares(capability) &&
        (p.state.account !== null || !p.declares('Auth')),
    );
  }

  async signOut(plugin: Plugin) {
    await plugin.logout();
    this.refresh();
  }
}

export type SignInStage =
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
    if (serverOf(plugin)) this.view = { stage: 'choose' };
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
            this.view = { stage: 'failed', message: step.Failed.message };
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
