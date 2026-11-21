<script lang="ts">
  // Settings › Sources (docs/ui.md): Add a Source; Installed, with Install from File… and, for each
  // plugin, on or off, sign-in, sign-out, its settings, version and author, and Uninstall…; then
  // Community Plugins with the disclosure (ADR-0012); then Relay, the web's own: the address and
  // token of a relay the person runs, for services that refuse browsers (ADR-0022 §7,
  // docs/web-relay.md), checked when saved.
  import AddSource from '../components/AddSource.svelte';
  import ConfirmSheet from '../components/ConfirmSheet.svelte';
  import Icon from '../components/Icon.svelte';
  import SettingsForm from '../components/SettingsForm.svelte';
  import Sheet from '../components/Sheet.svelte';
  import SignInSheet from '../components/SignInSheet.svelte';
  import { useApp } from '../lib/app';
  import { language, t } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import { explain, type SourceView } from '../lib/state/sources.svelte';
  import { toast } from '../lib/state/toast.svelte';

  // With `extensions`, the same list for Settings › Extensions: the extensions installed (AI Lyrics,
  // Customize, others), each the same way, and Install from File… in the heading.
  let { extensions = false }: { extensions?: boolean } = $props();
  const app = useApp();
  const shown = $derived(app.sources.list.filter((source) => source.extension === extensions));
  let signingIn = $state<Plugin | null>(null);
  let editing = $state<Plugin | null>(null);
  let editError = $state<string | null>(null);
  /** The disclosure, for one plugin or for allowing community plugins at all. */
  let disclosing = $state<Plugin | 'all' | null>(null);
  let uninstalling = $state<SourceView | null>(null);
  let installing = $state(false);
  let packageInput: HTMLInputElement | undefined = $state();

  // Versions and authors come from the packages: read them (without running the plugins).
  $effect(() => {
    void app.sources.verifyAll();
  });

  let relayError = $state<string | null>(null);
  let savingRelay = $state(false);
  // Whether the relay answers, once each time Settings › Sources opens.
  $effect(() => {
    void app.sources.checkRelay();
  });

  async function saveRelay(event: SubmitEvent) {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    savingRelay = true;
    relayError = await app.sources.saveRelay(String(data.get('address') ?? ''), String(data.get('token') ?? ''));
    savingRelay = false;
    if (!relayError) (form.elements.namedItem('token') as HTMLInputElement).value = '';
  }

  async function removeRelay() {
    relayError = await app.sources.saveRelay('', '');
  }

  async function install(file: File) {
    installing = true;
    try {
      const plugin = await app.sources.install(file);
      toast(t('Installed {name}', { name: plugin.name }));
    } catch (error) {
      toast(t('Could not install {file}: {reason}', { file: file.name, reason: await explain(error) }));
    } finally {
      installing = false;
    }
  }

  async function uninstall(source: SourceView) {
    const plugin = app.sources.get(source.id);
    uninstalling = null;
    if (!plugin) return;
    await app.sources.uninstall(plugin);
    toast(t('Uninstalled {name}', { name: source.name }));
  }

  async function setEnabled(source: SourceView, on: boolean) {
    const plugin = app.sources.get(source.id);
    if (!plugin) return;
    if (on && !source.official && (!app.sources.allowCommunity || !plugin.state.acknowledged)) {
      disclosing = plugin;
      return;
    }
    await plugin.setEnabled(on);
  }

  async function saveSettings(values: Record<string, unknown>) {
    if (!editing) return;
    try {
      await editing.setSettings({ ...editing.settings, ...values });
      editing = null;
      editError = null;
    } catch (error) {
      editError = await explain(error, editing ?? undefined);
    }
  }

  async function openSettings(source: SourceView) {
    const plugin = app.sources.get(source.id);
    if (!plugin) return;
    await plugin.verify().catch(() => undefined);
    editError = null;
    editing = plugin;
  }

  /** The plugin's settings in the app's language, where the package translates them (API 1.2). */
  function schemaFor(plugin: Plugin) {
    const info = plugin.info;
    if (!info) return {};
    const localized = info.localized_schemas ?? {};
    const best = app.jf.best_language(JSON.stringify(Object.keys(localized)), language());
    return (best ? localized[best] : undefined) ?? info.settings_schema;
  }

  async function signIn(source: SourceView) {
    const plugin = app.sources.get(source.id);
    if (!plugin) return;
    if (!source.official && (source.communityOff || !plugin.state.acknowledged)) {
      disclosing = plugin;
      return;
    }
    if (!source.enabled) await plugin.setEnabled(true);
    signingIn = plugin;
  }
</script>

<section class="sources-settings" aria-label={extensions ? t('Extensions') : t('Sources')}>
  {#if extensions}
    <!-- Only the list. -->
  {:else if !app.store.persistent}
    <p class="note" role="note">
      <Icon name="alert" size={18} />{t('This browser does not let Jellyfesh Web keep data: you will sign in again next time.')}
    </p>
  {:else if !app.store.keepsSecrets}
    <p class="note" role="note">
      <Icon name="alert" size={18} />{t('This page is not served over https, so sign-ins are not kept: you will sign in again next time.')}
    </p>
  {/if}

  {#if !extensions}<AddSource />{/if}

  <section class="group" aria-labelledby="installed-title">
    <header class="group-head">
      <h2 id="installed-title" class="section">{extensions ? t('Extensions') : t('Installed')}</h2>
      <button class="secondary small" disabled={installing} onclick={() => packageInput?.click()}>
        <Icon name="install" size={16} />{installing ? t('Installing…') : t('Install from File…')}
      </button>
    </header>
    {#if shown.length === 0}
      <div class="empty">
        {#if extensions}
          <p>{t('Install an extension from a .jfpkg file.')}</p>
        {:else}
          <h3>{t('No Sources Installed')}</h3>
          <p>{t('Install a source plugin from a .jfpkg file.')}</p>
        {/if}
      </div>
    {/if}
  <ul class="list">
    {#each shown as source (source.id)}
      <li class="source">
        <div class="top">
          <div class="name">
            <h2>{source.name}</h2>
            <span class="badge" class:official={source.official}>{source.official ? t('Official') : t('Community · unofficial')}</span>
          </div>
          <label class="switch">
            <input
              type="checkbox"
              role="switch"
              checked={source.enabled && !source.communityOff}
              aria-label={t('Enabled')}
              onchange={(event) => setEnabled(source, (event.currentTarget as HTMLInputElement).checked)}
            />
            <span aria-hidden="true"></span>
          </label>
        </div>
        {#if source.description}<p class="description">{source.description}</p>{/if}
        <p class="facts">
          {source.author ? t('Version {version} · {author}', { version: source.version, author: source.author }) : t('Version {version}', { version: source.version })}
        </p>
        <p class="status">
          {#if source.account}
            <Icon name="check" size={16} />{t('Signed in as {account}', { account: source.account })}
          {:else}
            {t('Not signed in')}
          {/if}
        </p>
        {#if source.server}<p class="server">{source.server}</p>{/if}
        {#if source.blocked && !source.communityOff && source.enabled}
          <p class="problem" role="alert"><Icon name="alert" size={16} />{source.blocked}</p>
        {/if}
        <div class="actions">
          {#if source.account}
            <button class="secondary" onclick={() => app.sources.signOut(app.sources.get(source.id)!)}>
              <Icon name="signout" size={18} />{t('Sign Out')}
            </button>
          {:else}
            <button class="primary" onclick={() => signIn(source)}>{t('Sign In')}</button>
          {/if}
          <button class="secondary" onclick={() => openSettings(source)}><Icon name="settings" size={18} />{t('Settings')}</button>
          <button class="secondary danger" onclick={() => (uninstalling = source)}>{t('Uninstall…')}</button>
        </div>
      </li>
    {/each}
  </ul>
  </section>

  {#if !extensions}
    <section class="community" aria-labelledby="community-title">
      <h2 id="community-title" class="section">{t('Community Plugins')}</h2>
      <p class="description">
        {t('Official sources are signed by Jellyfesh. Community plugins are made by others and are off unless you allow them.')}
      </p>
      <label class="row-switch">
        <span>{t('Allow Community Plugins')}</span>
        <span class="switch">
          <input
            type="checkbox"
            role="switch"
            checked={app.sources.allowCommunity}
            onchange={(event) => {
              const on = (event.currentTarget as HTMLInputElement).checked;
              if (on) {
                (event.currentTarget as HTMLInputElement).checked = false;
                disclosing = 'all';
              } else void app.sources.host.setAllowCommunity(false);
            }}
          />
          <span aria-hidden="true"></span>
        </span>
      </label>
    </section>

    <section class="relay" id="relay" aria-labelledby="relay-title">
      <h2 id="relay-title" class="section">{t('Relay')}</h2>
      <p class="description">
        {t('A relay you run yourself lets Jellyfesh Web reach services that browsers can’t. Jellyfesh doesn’t run one for you.')}
      </p>
      <form class="relay-form" onsubmit={saveRelay}>
        <div class="field">
          <label for="relay-address">{t('Relay Address')}</label>
          <input
            id="relay-address"
            name="address"
            type="url"
            inputmode="url"
            placeholder="http://127.0.0.1:8790"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            value={app.sources.relay?.address ?? ''}
          />
        </div>
        <div class="field">
          <label for="relay-token">{t('Relay Token')}</label>
          <input
            id="relay-token"
            name="token"
            type="password"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            aria-describedby={app.sources.relay ? 'relay-token-help' : undefined}
          />
          {#if app.sources.relay}<p class="help" id="relay-token-help">{t('Leave it empty to keep the saved token.')}</p>{/if}
        </div>
        {#if relayError}
          <p class="problem" role="alert"><Icon name="alert" size={16} />{relayError}</p>
        {:else if app.sources.relayStatus?.state === 'checking'}
          <p class="relay-status" aria-live="polite">{t('Checking…')}</p>
        {:else if app.sources.relayStatus?.state === 'connected'}
          <p class="relay-status ok" aria-live="polite"><Icon name="check" size={16} />{t('Connected to the relay.')}</p>
        {:else if app.sources.relayStatus?.state === 'problem'}
          <p class="problem" role="alert"><Icon name="alert" size={16} />{app.sources.relayStatus.message}</p>
        {/if}
        <div class="actions">
          <button class="primary" type="submit" disabled={savingRelay}>{t('Save')}</button>
          {#if app.sources.relay}
            <button class="secondary" type="button" onclick={removeRelay}>{t('Remove')}</button>
          {/if}
        </div>
      </form>
    </section>
  {/if}
</section>

{#if signingIn}
  <SignInSheet plugin={signingIn} onclose={() => (signingIn = null)} />
{/if}

{#if uninstalling}
  {@const source = uninstalling}
  <ConfirmSheet
    title={t('Uninstall {name}?', { name: source.name })}
    message={t('Its settings and sign-in are removed.')}
    action={t('Uninstall')}
    onconfirm={() => uninstall(source)}
    oncancel={() => (uninstalling = null)}
  />
{/if}

<input
  bind:this={packageInput}
  class="package-input"
  type="file"
  accept=".jfpkg"
  aria-hidden="true"
  tabindex="-1"
  onchange={(event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (file) void install(file);
  }}
/>

{#if editing}
  {@const plugin = editing}
  <Sheet title={plugin.info?.settings_schema.title ?? plugin.name} onclose={() => (editing = null)}>
    <SettingsForm
      schema={schemaFor(plugin)}
      values={plugin.state.settings ?? {}}
      submitLabel={t('Save')}
      error={editError}
      onsubmit={saveSettings}
    />
  </Sheet>
{/if}

{#if disclosing}
  {@const plugin = disclosing === 'all' ? null : disclosing}
  <Sheet
    title={plugin && app.sources.allowCommunity ? t('Turn On “{name}”?', { name: plugin.name }) : t('Allow Community Plugins?')}
    onclose={() => (disclosing = null)}
  >
    <ul class="disclosure">
      <li>{t('Community plugins are not made or checked by Jellyfesh.')}</li>
      <li>{t('They may break the terms of the service they connect to.')}</li>
      <li>{t('They may stop working at any time.')}</li>
      <li>{t('Use them for personal use only.')}</li>
    </ul>
    <div class="disclosure-actions">
      <button class="secondary" onclick={() => (disclosing = null)}>{t('Cancel')}</button>
      <button
        class="primary"
        onclick={async () => {
          if (plugin) await app.sources.host.acknowledge(plugin);
          else await app.sources.host.setAllowCommunity(true);
          disclosing = null;
        }}>{t('I Understand, Allow')}</button
      >
    </div>
  </Sheet>
{/if}

<style>
  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
  }
  .small {
    min-height: 36px;
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .secondary.danger {
    color: var(--jf-danger);
  }
  .facts {
    margin: 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .package-input {
    display: none;
  }
  .note {
    display: flex;
    gap: var(--jf-space-s2);
    align-items: flex-start;
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s3);
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-70);
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
    gap: var(--jf-space-s4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .source {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s2);
    padding: var(--jf-space-s5);
    border-radius: var(--jf-radius-r-xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--jf-space-s3);
  }
  .name h2 {
    margin: 0 0 4px;
    font-size: calc(var(--jf-type-roles-section-size) * var(--jf-text-scale, 1));
  }
  .badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill-press);
    color: var(--jf-ink-70);
    font-size: calc(12px * var(--jf-text-scale, 1));
    font-weight: 600;
  }
  .badge.official {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  .description,
  .status {
    margin: 0;
    color: var(--jf-ink-70);
    line-height: 1.45;
  }
  .status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    font-weight: 600;
  }
  .status :global(svg) {
    color: var(--jf-success);
  }
  .server {
    margin: 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
    overflow-wrap: anywhere;
  }
  .problem {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 0;
    color: var(--jf-danger);
  }
  .problem :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .actions,
  .disclosure-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s2);
  }
  .disclosure-actions {
    justify-content: flex-end;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .secondary:hover {
    background: var(--jf-fill-hover);
  }
  /* Groups are cards of Mist glass with their heading inside, as in the other sections. */
  .community,
  .relay {
    margin-top: var(--jf-space-s5);
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s5) var(--jf-space-s6);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .community > .section,
  .relay > .section {
    margin-bottom: var(--jf-space-s1);
  }
  .relay-form {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s4);
    margin-top: var(--jf-space-s4);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field label {
    font-weight: 600;
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .field input {
    min-height: 44px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    box-shadow: inset 0 0 0 1px var(--jf-line);
  }
  .field input:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .help {
    margin: 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .relay-status {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 0;
    color: var(--jf-ink-70);
  }
  .relay-status.ok :global(svg) {
    color: var(--jf-success);
  }
  .row-switch {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    margin-top: var(--jf-space-s4);
    font-weight: 600;
  }
  .switch {
    position: relative;
    display: inline-block;
    flex: none;
    width: 52px;
    height: 32px;
  }
  .switch input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .switch span {
    position: absolute;
    inset: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill-press);
    box-shadow: inset 0 0 0 1px var(--jf-line);
    pointer-events: none;
    transition: background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .switch span::after {
    content: '';
    position: absolute;
    top: 4px;
    left: 4px;
    width: 24px;
    height: 24px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-cloud);
    box-shadow: var(--jf-elevation-shadow-1);
    transition: transform var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .switch input:checked + span {
    background: var(--jf-accent);
  }
  .switch input:checked + span::after {
    transform: translateX(20px);
  }
  .switch input:focus-visible + span {
    box-shadow: var(--jf-elevation-focus);
  }
  .disclosure {
    margin: 0 0 var(--jf-space-s4);
    padding-inline-start: 1.2em;
    line-height: 1.6;
    color: var(--jf-ink-70);
  }
  .empty h3 {
    margin: 0 0 var(--jf-space-s2);
  }
</style>
