<script lang="ts">
  // Sources (docs/ui.md): what sources are, Add a Source, Your Sources (each opens what it offers,
  // or asks to sign in), and Sources Settings…, which manages them in Settings › Sources.
  import AddSource from '../components/AddSource.svelte';
  import Icon from '../components/Icon.svelte';
  import SignInSheet from '../components/SignInSheet.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import type { SourceView } from '../lib/state/sources.svelte';

  const app = useApp();
  let signingIn = $state<Plugin | null>(null);

  function status(source: SourceView): string {
    if (source.communityOff) return t('Community plugins are turned off.');
    if (!source.enabled) return t('Turned off');
    if (source.account) return t('Signed in as {account}', { account: source.account });
    return source.authKinds.length > 0 ? t('Not signed in') : t('Ready');
  }

  async function open(source: SourceView) {
    const plugin = app.sources.get(source.id);
    if (!plugin) return;
    if (source.communityOff || (!source.official && !plugin.state.acknowledged) || !source.enabled) {
      app.nav.openSettings('sources');
      return;
    }
    if (!source.account && source.authKinds.length > 0) {
      signingIn = plugin;
      return;
    }
    app.nav.open({ view: 'source', plugin: source.id });
  }
</script>

<section class="page" aria-labelledby="sources-title">
  <h1 id="sources-title" class="page-title">{t('Sources')}</h1>
  <p class="lede">{t('Sources bring music from servers and services into Jellyfesh.')}</p>

  <AddSource />

  <section class="yours" aria-labelledby="yours-title">
    <h2 id="yours-title" class="section">{t('Your Sources')}</h2>
    {#if app.sources.list.length === 0}
      <p class="muted">{t('No Sources Installed')}</p>
    {:else}
      <ul class="rows">
        {#each app.sources.list as source (source.id)}
          <li>
            <button class="row" onclick={() => open(source)}>
              <Icon name="sources" size={22} />
              <span class="text">
                <span class="title">{source.name}</span>
                <span class="sub">{status(source)}</span>
              </span>
              <Icon name="forward" size={18} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <button class="settings" onclick={() => app.nav.openSettings('sources')}><Icon name="settings" size={18} />{t('Sources Settings…')}</button>
</section>

{#if signingIn}
  <SignInSheet plugin={signingIn} onclose={() => (signingIn = null)} />
{/if}

<style>
  .lede {
    margin: 0 0 var(--jf-space-s6);
    color: var(--jf-ink-70);
    max-width: var(--jf-layout-reading);
  }
  .yours {
    max-width: var(--jf-layout-reading);
    margin-bottom: var(--jf-space-s6);
  }
  .rows {
    display: grid;
    gap: var(--jf-space-s2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    gap: var(--jf-space-s3);
    align-items: center;
    width: 100%;
    padding: var(--jf-space-s3) var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-tint-mist);
    box-shadow: inset 0 0 0 1px var(--jf-glass-edge-soft);
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .row:hover {
    background: var(--jf-fill-hover);
  }
  .text {
    display: grid;
    flex: 1;
    gap: 2px;
  }
  .title {
    font-weight: 600;
  }
  .sub {
    color: var(--jf-ink-70);
    font-size: 0.875em;
  }
  .muted {
    color: var(--jf-ink-70);
  }
  .settings {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .row:focus-visible,
  .settings:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
