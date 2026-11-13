<script lang="ts">
  // Home: each signed-in source's own start page (what its `browse` lists at the top), or a
  // welcome that points to Sources and to dropping files.
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { explain } from '../lib/state/sources.svelte';

  let { onopenfiles }: { onopenfiles: () => void } = $props();
  const app = useApp();

  interface Shelf {
    plugin: string;
    name: string;
    items: Item[];
    error: string | null;
    loading: boolean;
  }

  let shelves = $state.raw<Shelf[]>([]);

  $effect(() => {
    void app.sources.revision;
    const plugins = app.sources.usable('Browse');
    let live = true;
    shelves = plugins.map((p) => ({ plugin: p.id, name: p.name, items: [], error: null, loading: true }));
    for (const plugin of plugins) {
      plugin
        .browse(null, null, 60)
        .then((page) => ({ items: page.items, error: null }))
        .catch(async (error) => ({ items: [] as Item[], error: await explain(error, plugin) }))
        .then((result) => {
          if (!live) return;
          shelves = shelves.map((s) => (s.plugin === plugin.id ? { ...s, ...result, loading: false } : s));
        });
    }
    return () => {
      live = false;
    };
  });
</script>

<section class="page" aria-labelledby="home-title">
  <h1 id="home-title" class="page-title">{t('Home')}</h1>
  {#if shelves.length === 0}
    <div class="welcome">
      <h2>{t('Welcome to Jellyfesh')}</h2>
      <p>
        {t('Sign in to your Subsonic, Navidrome or Jellyfin server to browse and play your music here. Audio files you drop on this page play too; they are not saved.')}
      </p>
      <div class="actions">
        <button class="primary" onclick={() => app.nav.switchTab('sources')}><Icon name="sources" size={18} />{t('Sign In to a Server')}</button>
        <button class="secondary" onclick={onopenfiles}><Icon name="file" size={18} />{t('Open Files…')}</button>
      </div>
    </div>
  {/if}
  {#each shelves as shelf (shelf.plugin)}
    <section class="shelf" aria-label={shelf.name}>
      <h2 class="section">{shelf.name}</h2>
      {#if shelf.error}
        <p class="problem" role="alert"><Icon name="alert" size={18} />{shelf.error}</p>
      {:else if shelf.loading}
        <p class="muted">{t('Loading…')}</p>
      {:else}
        <ItemList plugin={shelf.plugin} items={shelf.items} />
      {/if}
    </section>
  {/each}
</section>

<style>
  .welcome {
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s7);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .welcome h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: var(--jf-type-text-h3);
  }
  .welcome p {
    margin: 0;
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s5);
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .actions button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .secondary {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  .shelf + .shelf {
    margin-top: var(--jf-space-s8);
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    align-items: flex-start;
    color: var(--jf-ink-70);
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  .muted {
    color: var(--jf-ink-50);
  }
</style>
