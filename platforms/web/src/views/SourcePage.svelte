<script lang="ts">
  // A source's own start page (what its `browse` lists at the top), opened from Your Sources.
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { explain } from '../lib/state/sources.svelte';

  let { plugin }: { plugin: string } = $props();
  const app = useApp();
  let items = $state.raw<Item[]>([]);
  let next = $state<string | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  const name = $derived(app.sources.get(plugin)?.name ?? plugin);

  async function load(cursor: string | null) {
    const source = app.sources.get(plugin);
    if (!source) {
      error = t('The source of this page is not available.');
      loading = false;
      return;
    }
    loading = true;
    try {
      const page = await source.browse(null, cursor, 100);
      items = cursor ? [...items, ...page.items] : page.items;
      next = page.next;
      error = null;
    } catch (e) {
      error = await explain(e, source);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void app.sources.revision;
    items = [];
    void load(null);
  });
</script>

<section class="page" aria-labelledby="source-title">
  <h1 id="source-title" class="page-title">{name}</h1>
  {#if error}
    <p class="problem" role="alert"><Icon name="alert" size={18} />{error}</p>
  {:else if loading && items.length === 0}
    <p class="muted" aria-live="polite">{t('Loading…')}</p>
  {:else if items.length === 0}
    <p class="muted">{t('Nothing here yet.')}</p>
  {/if}
  <ItemList {plugin} {items} hasMore={next !== null} loadingMore={loading} onmore={() => !loading && next && load(next)} />
</section>

<style>
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    align-items: center;
    color: var(--jf-ink-70);
  }
  .problem :global(svg) {
    color: var(--jf-warning);
  }
  .muted {
    color: var(--jf-ink-70);
  }
</style>
