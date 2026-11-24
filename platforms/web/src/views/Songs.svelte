<script lang="ts">
  // Songs: Play and Shuffle, the sort menu, then every song (docs/ui.md). Playing a row plays
  // the whole list in this order from that song.
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import LibraryState from '../components/LibraryState.svelte';
  import SortMenu from '../components/SortMenu.svelte';
  import { useApp } from '../lib/app';
  import type { Item, TrackSort } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import { remember, remembered } from '../lib/state/prefs';

  const app = useApp();
  const PAGE = 200;
  let sort = $state<TrackSort>(remembered('songs.sort', 'artist'));
  let descending = $state(remembered('songs.descending', false));
  let count = $state(PAGE);
  $effect(() => {
    remember('songs.sort', sort);
    remember('songs.descending', descending);
  });
  const items = $derived((void app.library.revision, app.library.songs(sort, descending, 0, count)));
  const total = $derived(app.library.stats.tracks);

  function playAll(from: Item | null, shuffle: boolean) {
    const keys = app.library.songs(sort, descending).map((i) => i.id);
    if (keys.length === 0) return;
    const entries = keys.map((key) => app.library.entry(key));
    // Shuffle starts on a random song (the core picks it).
    if (shuffle) app.player.shuffleEntries(entries);
    else app.player.playEntries(entries, from ? Math.max(0, keys.indexOf(from.id)) : 0);
  }
</script>

<section class="page" aria-labelledby="songs-title">
  <header class="page-head">
    <h1 id="songs-title" class="page-title">{t('Songs')}</h1>
    {#if total > 0}
      <div class="tools">
        <button class="primary" onclick={() => playAll(null, false)}><Icon name="play" size={18} />{t('Play')}</button>
        <button class="secondary" onclick={() => playAll(null, true)}><Icon name="shuffle" size={18} />{t('Shuffle')}</button>
        <SortMenu
          options={[
            ['title', t('Title')],
            ['artist', t('Artist')],
            ['album', t('Album')],
            ['year', t('Year')],
            ['added', t('Recently Added')],
          ]}
          bind:sort
          bind:descending
        />
      </div>
    {/if}
  </header>
  <LibraryState title={t('No Songs Yet')} text={t('Add a music folder to see your songs here.')} />
  {#if total > 0}
    <ItemList plugin={LIBRARY} {items} hasMore={count < total} onmore={() => (count += PAGE)} onplay={(item) => playAll(item, false)} />
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
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--jf-space-s2);
  }
  .primary,
  .secondary {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-height: 40px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
