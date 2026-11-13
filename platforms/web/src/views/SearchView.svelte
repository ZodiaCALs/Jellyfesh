<script lang="ts">
  // Search every signed-in source at once; one section per source, in the order they answer.
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';

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
  const searched = $derived(search.query.trim().length > 0);
  const done = $derived(search.sections.length > 0 && search.sections.every((s) => s.status !== 'loading'));
  const empty = $derived(done && search.sections.every((s) => s.rows.length === 0 && s.status === 'done'));

  function names(also: string[]): string[] {
    return also.map((id) => app.sources.get(id)?.name ?? id);
  }
</script>

<section class="page" aria-labelledby="search-title">
  <h1 id="search-title" class="page-title">{t('Search')}</h1>
  <form class="field" role="search" onsubmit={(event) => (event.preventDefault(), search.run())}>
    <Icon name="search" />
    <input
      bind:this={input}
      type="search"
      value={search.query}
      placeholder={t('Songs, albums, artists')}
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
        {usable > 0
          ? t('Find songs, albums and artists on every server you have signed in to.')
          : t('Sign in to a server in Sources to search it.')}
      </p>
    </div>
  {:else if empty}
    <div class="hint">
      <h2>{t('No Results')}</h2>
      <p>{t('Try other words.')}</p>
    </div>
  {/if}

  <div aria-live="polite" aria-busy={!done}>
    {#each search.sections as section (section.source)}
      {#if section.status !== 'done' || section.rows.length > 0}
        <section class="results" aria-label={section.name}>
          <h2 class="section">{section.name}</h2>
          {#if section.status === 'loading'}
            <p class="muted">{t('Searching…')}</p>
          {:else if section.status === 'error'}
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
  </div>
</section>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    max-width: var(--jf-layout-reading);
    min-height: 48px;
    padding: 0 var(--jf-space-s4);
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-50);
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
    font-size: 16px;
    outline: none;
  }
  .hint {
    margin-top: var(--jf-space-s8);
    max-width: var(--jf-layout-reading);
  }
  .hint h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: var(--jf-type-text-h3);
  }
  .hint p {
    margin: 0;
    color: var(--jf-ink-70);
  }
  .results {
    margin-top: var(--jf-space-s7);
  }
  .muted {
    color: var(--jf-ink-50);
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
</style>
