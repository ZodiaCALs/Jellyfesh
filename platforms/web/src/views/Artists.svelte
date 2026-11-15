<script lang="ts">
  // Artists: the library's artists by name (docs/ui.md); each opens their albums and songs.
  import ItemList from '../components/ItemList.svelte';
  import LibraryState from '../components/LibraryState.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';

  const app = useApp();
  const PAGE = 120;
  let count = $state(PAGE);
  const items = $derived((void app.library.revision, app.library.artists(0, count)));
  const total = $derived(app.library.stats.artists);
</script>

<section class="page" aria-labelledby="artists-title">
  <h1 id="artists-title" class="page-title">{t('Artists')}</h1>
  <LibraryState title={t('No Artists Yet')} text={t('Add a music folder to see your artists here.')} />
  {#if total > 0}
    <ItemList plugin={LIBRARY} {items} hasMore={count < total} onmore={() => (count += PAGE)} />
  {/if}
</section>
