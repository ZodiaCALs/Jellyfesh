<script lang="ts">
  // Add a Source (docs/ui.md): the services the apps share (`source_presets`), offered while their
  // official plugin is here. Choosing one turns its plugin on and opens the sign-in sheet, which
  // asks for the server's address first. On the Sources page and in Settings › Sources.
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import type { PresetView } from '../lib/state/sources.svelte';
  import Icon from './Icon.svelte';
  import SignInSheet from './SignInSheet.svelte';

  const app = useApp();
  let signingIn = $state<{ plugin: Plugin; preset: PresetView } | null>(null);

  function presetLine(preset: PresetView): string {
    if (preset.account) return t('Signed in as {account}', { account: preset.account });
    switch (preset.id) {
      case 'jellyfin':
        return t('Your Jellyfin server');
      case 'navidrome':
        return t('Your Navidrome server');
      case 'subsonic':
        return t('Airsonic, Gonic and other Subsonic servers');
      case 'qqmusic':
        return t('Your QQ Music account');
      case 'spotify':
        return t('Your Spotify devices');
      default:
        return '';
    }
  }

  async function connect(preset: PresetView) {
    const plugin = app.sources.get(preset.plugin);
    if (!plugin) return;
    if (!plugin.state.enabled) await plugin.setEnabled(true);
    signingIn = { plugin, preset };
  }
</script>

{#if app.sources.presets.length > 0}
  <section class="add" aria-labelledby="add-title">
    <h2 id="add-title" class="section">{t('Add a Source')}</h2>
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

{#if signingIn}
  <SignInSheet plugin={signingIn.plugin} preset={signingIn.preset} onclose={() => (signingIn = null)} />
{/if}

<style>
  .add {
    margin: 0 0 var(--jf-space-s6);
    max-width: var(--jf-layout-reading);
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
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .preset:hover {
    background: var(--jf-fill-hover);
  }
  .preset:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
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
</style>
