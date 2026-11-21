<script lang="ts">
  // A source's Discover page (docs/ui.md "Discover"; ADR-0022 §2), opened from Your Sources: its
  // name and status, Browse (its own folders) in the header, then its shelves, each with its title
  // and See All when it has more (which browses it). Row shelves scroll sideways, grid shelves
  // wrap, chart shelves are numbered lists. While loading: placeholders the shape of the shelves;
  // on failure: the error and Try Again. What the page holds comes from jf-web's discover rules
  // (src/lib/state/discover.ts).
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import { useApp } from '../lib/app';
  import type { DiscoverPage, Shelf } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { discoverPage } from '../lib/state/discover';
  import { explain } from '../lib/state/sources.svelte';

  let { plugin }: { plugin: string } = $props();
  const app = useApp();
  let page = $state.raw<DiscoverPage | null>(null);
  let loading = $state(true);
  let attempt = $state(0);
  const view = $derived(app.sources.list.find((s) => s.id === plugin));
  const name = $derived(view?.name ?? plugin);
  const status = $derived.by(() => {
    if (!view) return '';
    if (view.account) return t('Signed in as {account}', { account: view.account });
    return view.authKinds.length > 0 ? t('Not signed in') : t('Ready');
  });
  const canBrowse = $derived(app.sources.get(plugin)?.declares('Browse') ?? false);

  $effect(() => {
    void app.sources.revision;
    void attempt;
    const source = app.sources.get(plugin);
    let live = true;
    loading = true;
    if (!source) {
      page = { shelves: [], error: t('The source of this page is not available.'), see_all: {}, made: true };
      loading = false;
      return;
    }
    discoverPage(app.jf, source, (error) => explain(error, source))
      .catch(async (error): Promise<DiscoverPage> => ({ shelves: [], error: await explain(error, source), see_all: {}, made: true }))
      .then((answer) => {
        if (!live) return;
        page = answer;
        loading = false;
      });
    return () => {
      live = false;
    };
  });

  /** Shelves Jellyfesh makes are worded by id; a plugin's own are in its words. */
  function title(shelf: Shelf): string {
    return page?.made && shelf.id === 'for_you' ? t('For You') : shelf.title;
  }

  function seeAll(shelf: Shelf) {
    const item = page?.see_all[shelf.id];
    if (item) app.nav.open({ view: 'browse', plugin, item });
  }
</script>

<section class="page" aria-labelledby="source-title">
  <header class="head">
    <div class="who">
      <h1 id="source-title" class="page-title">{name}</h1>
      {#if status}<p class="status">{status}</p>{/if}
    </div>
    {#if canBrowse}
      <button class="browse" onclick={() => app.nav.open({ view: 'root', plugin })}>
        <Icon name="folder" size={18} />{t('Browse')}
      </button>
    {/if}
  </header>
  {#if loading && !page}
    <div class="shelves" aria-hidden="false">
      <Skeleton kind="cards" count={6} />
      <Skeleton kind="cards" count={6} />
    </div>
  {:else if page && page.shelves.length === 0}
    <div class="problem" role="alert">
      <p><Icon name="alert" size={18} />{page.error ?? t('Nothing here yet.')}</p>
      <button class="secondary" onclick={() => (attempt += 1)} disabled={loading}>{t('Try Again')}</button>
    </div>
  {:else if page}
    <div class="shelves">
      {#each page.shelves as shelf (shelf.id)}
        <section class="shelf" aria-labelledby="shelf-{shelf.id}">
          <div class="shelf-head">
            <h2 id="shelf-{shelf.id}" class="section">{title(shelf)}</h2>
            {#if shelf.more && page.see_all[shelf.id]}
              <button class="see-all" onclick={() => seeAll(shelf)} aria-label={t('See All {title}', { title: title(shelf) })}>
                {t('See All')}<Icon name="forward" size={16} />
              </button>
            {/if}
          </div>
          <ItemList {plugin} items={shelf.items} row={shelf.style === 'Row'} numbered={shelf.style === 'Chart'} />
        </section>
      {/each}
    </div>
  {/if}
</section>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--jf-space-s3);
    margin-bottom: var(--jf-space-s5);
  }
  .who {
    min-width: 0;
  }
  .who .page-title {
    margin-bottom: 0;
  }
  .status {
    margin: 4px 0 0;
    color: var(--jf-ink-70);
  }
  .browse,
  .secondary {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-height: 40px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .browse:hover,
  .secondary:hover {
    background: var(--jf-fill-hover);
  }
  .browse:focus-visible,
  .secondary:focus-visible,
  .see-all:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .shelves {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s6);
  }
  .shelf-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    margin-bottom: var(--jf-space-s3);
  }
  .shelf-head .section {
    margin: 0;
  }
  .see-all {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-accent-text);
    font: inherit;
    font-size: calc(14px * var(--jf-text-scale, 1));
    font-weight: 600;
    cursor: pointer;
  }
  .see-all:hover {
    background: var(--jf-fill-hover);
  }
  .problem {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--jf-space-s3);
  }
  .problem p {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    margin: 0;
    color: var(--jf-danger, var(--jf-ink-90));
  }
</style>
