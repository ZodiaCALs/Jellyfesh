<script lang="ts">
  // Search (docs/ui.md; ADR-0024 §4): tabs All, Library and one per source that answered.
  // * All: the library's songs, albums and artists, then From Your Sources, one section per source
  //   whose terms allow merging, in the order they answered, then a link to the tab of each source
  //   that keeps its results apart ("Results from Spotify").
  // * Library: the library's results only.
  // * A source's tab: its songs, albums, artists and playlists as it listed them, each with Show
  //   More while it has more, and its attribution.
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import { useApp } from '../lib/app';
  import type { Item, ItemKind, SectionRow } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import type { SearchTabId } from '../lib/state/search.svelte';

  const app = useApp();
  const search = app.search;
  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    input?.focus();
  });

  // Search again when sources sign in or out.
  let seen = app.sources.revision;
  $effect(() => {
    const revision = app.sources.revision;
    if (revision !== seen) {
      seen = revision;
      if (search.query.trim()) search.run();
    }
  });

  const usable = $derived((void app.sources.revision, app.sources.usable('Search').length));
  const view = $derived(search.view);
  const searched = $derived(search.query.trim().length > 0 && view !== null);
  const library = $derived(view?.library ?? []);
  const sections = $derived(view?.sections ?? []);
  const apart = $derived((view?.tabs ?? []).filter((tab) => !tab.merged));
  const fromSources = $derived(sections.some((s) => s.status !== 'done' || s.rows.length > 0));
  const showLibraryTab = $derived(!app.library.isEmpty);

  /** The source of the tab shown, or null for All and Library. */
  const shownSource = $derived(search.tab.startsWith('source:') ? search.tab.slice('source:'.length) : null);
  const sourceTab = $derived(shownSource ? search.tabOf(shownSource) : null);
  const stillSearching = $derived(
    shownSource !== null && sections.some((s) => s.source === shownSource && s.status === 'searching'),
  );
  const attribution = $derived(shownSource ? (app.sources.get(shownSource)?.info?.content_policy.attribution ?? null) : null);

  // A tab that has nothing to show once the search is over (a new query found nothing there)
  // gives way to All.
  $effect(() => {
    if (!view) return;
    const tab = search.tab;
    if (tab === 'library' && !showLibraryTab) search.tab = 'all';
    if (shownSource && !stillSearching && !view.tabs.some((x) => x.source === shownSource)) {
      if (view.done || !sections.some((s) => s.source === shownSource)) search.tab = 'all';
    }
  });

  function names(also: string[]): string[] {
    return also.map((id) => app.sources.get(id)?.name ?? id);
  }

  function ofKind(rows: SectionRow[], kind: ItemKind): SectionRow[] {
    return rows.filter((r) => r.item.kind === kind);
  }

  function kindTitle(kind: ItemKind): string {
    switch (kind) {
      case 'Track':
        return t('Songs');
      case 'Album':
        return t('Albums');
      case 'Artist':
        return t('Artists');
      case 'Playlist':
        return t('Playlists');
      default:
        return t('Folders');
    }
  }

  function choose(tab: SearchTabId) {
    search.tab = tab;
  }

  /** ← and → move between tabs, as in a tab list. */
  function keys(event: KeyboardEvent) {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    const tabs = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    const at = tabs.indexOf(document.activeElement as HTMLButtonElement);
    const next = tabs[(at + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length];
    event.preventDefault();
    next?.focus();
    next?.click();
  }
</script>

{#snippet libraryResults(rows: SectionRow[])}
  {@const also = new Map(rows.map((r) => [r.item, names(r.also_in)]))}
  {#each ['Track', 'Album', 'Artist'] as const as kind (kind)}
    {@const found = ofKind(rows, kind)}
    {#if found.length > 0}
      <section class="results" aria-labelledby="lib-{kind}">
        <h2 id="lib-{kind}" class="section">{kindTitle(kind)}</h2>
        <ItemList plugin={LIBRARY} items={found.map((r) => r.item)} alsoIn={(item: Item) => also.get(item) ?? []} />
      </section>
    {/if}
  {/each}
{/snippet}

<section class="page" aria-labelledby="search-title">
  <h1 id="search-title" class="page-title">{t('Search')}</h1>
  <form class="field" role="search" onsubmit={(event) => (event.preventDefault(), search.run())}>
    <Icon name="search" />
    <input
      bind:this={input}
      type="search"
      value={search.query}
      placeholder={t('Songs, albums and artists')}
      aria-label={t('Search')}
      autocomplete="off"
      spellcheck="false"
      enterkeyhint="search"
      oninput={(event) => search.input((event.currentTarget as HTMLInputElement).value)}
    />
  </form>

  {#if !searched}
    <div class="hint">
      <h2>{t('Search Your Music')}</h2>
      <p>
        {usable > 0 || !app.library.isEmpty
          ? t('Find songs, albums and artists in your library and on every server you have signed in to.')
          : t('Add a music folder, or sign in to a server in Sources, to search your music.')}
      </p>
    </div>
  {:else if view}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div class="tabs" role="tablist" aria-label={t('Results')} tabindex="-1" onkeydown={keys}>
      <button
        role="tab"
        id="search-tab-all"
        aria-selected={search.tab === 'all'}
        aria-controls="search-panel"
        tabindex={search.tab === 'all' ? 0 : -1}
        class:on={search.tab === 'all'}
        onclick={() => choose('all')}>{t('All')}</button
      >
      {#if showLibraryTab}
        <button
          role="tab"
          id="search-tab-library"
          aria-selected={search.tab === 'library'}
          aria-controls="search-panel"
          tabindex={search.tab === 'library' ? 0 : -1}
          class:on={search.tab === 'library'}
          onclick={() => choose('library')}>{t('Library')}</button
        >
      {/if}
      {#each view.tabs as tab (tab.source)}
        {@const id = `source:${tab.source}` as const}
        <button
          role="tab"
          id="search-tab-{tab.source}"
          aria-selected={search.tab === id}
          aria-controls="search-panel"
          tabindex={search.tab === id ? 0 : -1}
          class:on={search.tab === id}
          onclick={() => choose(id)}>{tab.name}</button
        >
      {/each}
    </div>

    <div
      id="search-panel"
      role="tabpanel"
      aria-labelledby="search-tab-{shownSource ?? search.tab}"
      aria-live="polite"
      aria-busy={!view.done}
    >
      {#if search.tab === 'all'}
        {#if view.empty}
          <div class="hint">
            <h2>{t('No Results')}</h2>
            <p>{t('Try other words.')}</p>
          </div>
        {/if}
        {@render libraryResults(library)}
        {#if fromSources}
          <h2 class="from">{t('From Your Sources')}</h2>
        {/if}
        {#each sections as section (section.source)}
          {#if section.status !== 'done' || section.rows.length > 0}
            <section class="results" aria-label={section.name}>
              <h2 class="section">{section.name}</h2>
              {#if section.status === 'searching'}
                <p class="muted">{t('Searching…')}</p>
              {:else if section.status === 'failed'}
                <p class="problem"><Icon name="alert" size={18} />{t('This Source Did Not Answer')}: {section.error}</p>
              {:else}
                {@const also = new Map(section.rows.map((r) => [r.item, names(r.also_in)]))}
                <ItemList
                  plugin={section.source}
                  items={section.rows.map((r) => r.item)}
                  alsoIn={(item: Item) => also.get(item) ?? []}
                />
              {/if}
            </section>
          {/if}
        {/each}
        {#if apart.length > 0}
          <div class="apart">
            {#each apart as tab (tab.source)}
              <button class="link" onclick={() => choose(`source:${tab.source}`)}>
                {t('Results from {service}', { service: tab.name })}<Icon name="forward" size={16} />
              </button>
            {/each}
          </div>
        {/if}
      {:else if search.tab === 'library'}
        {#if library.length === 0}
          <div class="hint">
            <h2>{t('No Results')}</h2>
            <p>{t('Try other words.')}</p>
          </div>
        {:else}
          {@render libraryResults(library)}
        {/if}
      {:else if sourceTab}
        {#each sourceTab.groups as group (group.kind)}
          {@const key = `${sourceTab.source}|${group.kind}`}
          {@const loading = search.loadingMore.has(key)}
          {@const problem = search.moreErrors.get(key)}
          <section class="results" aria-labelledby="kind-{group.kind}">
            <h2 id="kind-{group.kind}" class="section">{kindTitle(group.kind)}</h2>
            <ItemList plugin={sourceTab.source} items={group.items} />
            {#if problem}
              <p class="problem"><Icon name="alert" size={18} />{problem}</p>
            {/if}
            {#if group.next}
              <div class="show-more">
                <button class="pill secondary" disabled={loading} onclick={() => search.more(sourceTab.source, group.kind)}
                  >{loading ? t('Loading…') : t('Show More')}</button
                >
              </div>
            {/if}
          </section>
        {/each}
        {#if attribution}
          <p class="attribution">{t('Content from {service}', { service: sourceTab.name })}</p>
        {/if}
      {:else if stillSearching}
        <p class="muted searching">{t('Searching…')}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    width: fit-content;
    max-width: 100%;
    margin-top: var(--jf-space-s5);
    padding: 4px;
    overflow-x: auto;
    scrollbar-width: none;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tabs button {
    flex: none;
    min-height: 36px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow: var(--jf-elevation-shadow-1);
  }
  .tabs button:focus-visible,
  .link:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .from {
    margin: var(--jf-space-s7) 0 var(--jf-space-s2);
    font-size: calc(var(--jf-type-text-h3) * var(--jf-text-scale, 1));
  }
  .field {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    max-width: var(--jf-layout-reading);
    min-height: 48px;
    padding: 0 var(--jf-space-s4);
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-70);
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  .field:focus-within {
    box-shadow: var(--jf-elevation-focus);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 48px;
    border: 0;
    background: transparent;
    color: var(--jf-ink-90);
    font: inherit;
    font-size: calc(16px * var(--jf-text-scale, 1));
    outline: none;
  }
  .hint {
    margin-top: var(--jf-space-s8);
    max-width: var(--jf-layout-reading);
  }
  .hint h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: calc(var(--jf-type-text-h3) * var(--jf-text-scale, 1));
  }
  .hint p {
    margin: 0;
    color: var(--jf-ink-70);
  }
  .results {
    margin-top: var(--jf-space-s7);
  }
  .muted {
    color: var(--jf-ink-70);
  }
  .searching {
    margin-top: var(--jf-space-s7);
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    color: var(--jf-ink-70);
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  .apart {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s7);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s1);
    min-height: 44px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .link:hover {
    background: var(--jf-fill-hover);
  }
  .show-more {
    display: flex;
    justify-content: center;
    margin-top: var(--jf-space-s3);
  }
  .attribution {
    margin-top: var(--jf-space-s6);
    color: var(--jf-ink-70);
    font-size: calc(var(--jf-type-text-caption, 12px) * var(--jf-text-scale, 1));
  }
</style>
