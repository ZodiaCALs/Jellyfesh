<script lang="ts">
  // Albums: the library's albums in a grid, sorted by Title, Artist, Year or Recently Added
  // (docs/ui.md). Pages of the catalogue load as the end comes into view.
  import ItemList from '../components/ItemList.svelte';
  import LibraryState from '../components/LibraryState.svelte';
  import SortMenu from '../components/SortMenu.svelte';
  import { useApp } from '../lib/app';
  import type { AlbumSort } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import { remember, remembered } from '../lib/state/prefs';

  const app = useApp();
  const PAGE = 120;
  let sort = $state<AlbumSort>(remembered('albums.sort', 'title'));
  let descending = $state(remembered('albums.descending', false));
  let count = $state(PAGE);
  $effect(() => {
    remember('albums.sort', sort);
    remember('albums.descending', descending);
  });
  const items = $derived((void app.library.revision, app.library.albums(sort, descending, 0, count)));
  const total = $derived(app.library.stats.albums);
</script>

<section class="page" aria-labelledby="albums-title">
  <header class="page-head">
    <h1 id="albums-title" class="page-title">{t('Albums')}</h1>
    {#if total > 0}
      <SortMenu
        options={[
          ['title', t('Title')],
          ['artist', t('Artist')],
          ['year', t('Year')],
          ['added', t('Recently Added')],
        ]}
        bind:sort
        bind:descending
      />
    {/if}
  </header>
  <LibraryState title={t('No Albums Yet')} text={t('Add a music folder to see your albums here.')} />
  {#if total > 0}
    <ItemList plugin={LIBRARY} {items} hasMore={count < total} onmore={() => (count += PAGE)} />
  {/if}
</section>

<style>
  .page-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
  }
</style>
