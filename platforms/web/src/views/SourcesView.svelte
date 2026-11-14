<script lang="ts">
  // Sources: the bundled plugins, whether they are official, signed in and on, with sign-in,
  // sign-out, settings and the community disclosure (ADR-0012).
  import Icon from '../components/Icon.svelte';
  import SettingsForm from '../components/SettingsForm.svelte';
  import Sheet from '../components/Sheet.svelte';
  import SignInSheet from '../components/SignInSheet.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import { explain, type PresetView, type SourceView } from '../lib/state/sources.svelte';

  const app = useApp();
  let signingIn = $state<Plugin | null>(null);
  /** The service chosen in Add a Source, while its sign-in sheet is open. */
  let signingAs = $state<PresetView | null>(null);
  let editing = $state<Plugin | null>(null);
  let editError = $state<string | null>(null);
  let disclosing = $state<Plugin | null>(null);

  const community = $derived(app.sources.list.some((s) => !s.official));

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

  function presetLine(preset: PresetView): string {
    if (preset.account) return t('Signed in as {account}', { account: preset.account });
    switch (preset.id) {
      case 'jellyfin':
        return t('Your Jellyfin server');
      case 'navidrome':
        return t('Your Navidrome server');
      case 'subsonic':
        return t('Airsonic, Gonic and other Subsonic servers');
      default:
        return '';
    }
  }

  /** A service from Add a Source: its plugin on, then the sign-in sheet (address first). */
  async function connect(preset: PresetView) {
    const plugin = app.sources.get(preset.plugin);
    if (!plugin) return;
    if (!plugin.state.enabled) await plugin.setEnabled(true);
    signingAs = preset;
    signingIn = plugin;
  }

  async function signIn(source: SourceView) {
    const plugin = app.sources.get(source.id);
    if (!plugin) return;
    if (!source.official && (source.communityOff || !plugin.state.acknowledged)) {
      disclosing = plugin;
      return;
    }
    if (!source.enabled) await plugin.setEnabled(true);
    signingAs = null;
    signingIn = plugin;
  }
</script>

<section class="page" aria-labelledby="sources-title">
  <h1 id="sources-title" class="page-title">{t('Sources')}</h1>
  <p class="lede">{t('Sources bring music from servers and services into Jellyfesh.')}</p>

  {#if !app.store.persistent}
    <p class="note" role="note">
      <Icon name="alert" size={18} />{t('This browser does not let Jellyfesh Web keep data: you will sign in again next time.')}
    </p>
  {:else if !app.store.keepsSecrets}
    <p class="note" role="note">
      <Icon name="alert" size={18} />{t('This page is not served over https, so sign-ins are not kept: you will sign in again next time.')}
    </p>
  {/if}

  {#if app.sources.list.length === 0}
    <div class="empty">
      <h2>{t('No Sources Yet')}</h2>
      <p>{t('This copy of Jellyfesh Web was built without source plugins.')}</p>
    </div>
  {/if}

  {#if app.sources.presets.length > 0}
    <section class="add" aria-labelledby="add-title">
      <h2 id="add-title">{t('Add a Source')}</h2>
      <ul class="presets">
        {#each app.sources.presets as preset (preset.id)}
          <li>
            <button class="preset" onclick={() => connect(preset)}>
              <Icon name="globe" size={22} />
              <span class="text">
                <span class="title">{preset.name}</span>
                {#if presetLine(preset)}<span class="sub">{presetLine(preset)}</span>{/if}
              </span>
              <Icon name="forward" size={18} />
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <ul class="list">
    {#each app.sources.list as source (source.id)}
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
        <p class="status">
          {#if source.account}
            <Icon name="check" size={16} />{t('Signed in as {account}', { account: source.account })}
          {:else}
            {t('Not signed in')}
          {/if}
          {#if source.server}<span class="server"> · {source.server}</span>{/if}
        </p>
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
        </div>
      </li>
    {/each}
  </ul>

  {#if community}
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
                disclosing = app.sources.host.plugins.find((p) => !p.official) ?? null;
              } else void app.sources.host.setAllowCommunity(false);
            }}
          />
          <span aria-hidden="true"></span>
        </span>
      </label>
    </section>
  {/if}
</section>

{#if signingIn}
  <SignInSheet
    plugin={signingIn}
    preset={signingAs}
    onclose={() => {
      signingIn = null;
      signingAs = null;
    }}
  />
{/if}

{#if editing}
  {@const plugin = editing}
  <Sheet title={plugin.info?.settings_schema.title ?? plugin.name} onclose={() => (editing = null)}>
    <SettingsForm
      schema={plugin.info?.settings_schema ?? {}}
      values={plugin.state.settings ?? {}}
      submitLabel={t('Save')}
      error={editError}
      onsubmit={saveSettings}
    />
  </Sheet>
{/if}

{#if disclosing}
  {@const plugin = disclosing}
  <Sheet
    title={app.sources.allowCommunity ? t('Turn On “{name}”?', { name: plugin.name }) : t('Allow Community Plugins?')}
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
          await app.sources.host.acknowledge(plugin);
          disclosing = null;
        }}>{t('I Understand, Allow')}</button
      >
    </div>
  </Sheet>
{/if}

<style>
  .lede {
    margin: 0 0 var(--jf-space-s6);
    color: var(--jf-ink-70);
    max-width: var(--jf-layout-reading);
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
  .add {
    margin: 0 0 var(--jf-space-s6);
    max-width: var(--jf-layout-reading);
  }
  .add h2 {
    margin: 0 0 var(--jf-space-s3);
    font-size: var(--jf-type-roles-section-size);
  }
  .presets {
    display: grid;
    gap: var(--jf-space-s2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .preset {
    display: flex;
    gap: var(--jf-space-s3);
    align-items: center;
    width: 100%;
    padding: var(--jf-space-s3) var(--jf-space-s4);
    border: 1px solid var(--jf-line);
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: inherit;
    text-align: start;
    cursor: pointer;
  }
  .preset:hover {
    background: var(--jf-fill-hover);
  }
  .preset .text {
    display: grid;
    flex: 1;
    gap: 2px;
  }
  .preset .title {
    font-weight: 600;
  }
  .preset .sub {
    color: var(--jf-ink-70);
    font-size: 0.875em;
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
    font-size: var(--jf-type-roles-section-size);
  }
  .badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill-press);
    color: var(--jf-ink-70);
    font-size: 12px;
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
    font-weight: 400;
    color: var(--jf-ink-50);
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
  .community {
    margin-top: var(--jf-space-s8);
    max-width: var(--jf-layout-reading);
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
  .empty h2 {
    margin: 0 0 var(--jf-space-s2);
  }
</style>
