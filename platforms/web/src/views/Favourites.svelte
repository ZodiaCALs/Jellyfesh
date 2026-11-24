<script lang="ts">
  // Favourites (docs/ui.md): the songs marked with the heart, the latest first, from the library
  // and every source, with Play and Shuffle. Kept in this browser. Empty: "No Favourites Yet".
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import SongList from '../components/SongList.svelte';
  import { useApp } from '../lib/app';
  import type { Saved } from '../lib/core/types';
  import { t, tn } from '../lib/i18n/index.svelte';
  import { savedEntries } from '../lib/items';

  const app = useApp();
  const rows = $derived.by((): Saved[] => {
    void app.collections.revision;
    void app.library.revision;
    return app.collections.favorites().map((s) => ({ plugin: s.plugin, item: s.item }));
  });

  function play(index?: number) {
    const { entries, start } = savedEntries(rows, index);
    if (entries.length > 0) app.player.playEntries(entries, start);
  }

  function shuffle() {
    const { entries } = savedEntries(rows);
    if (entries.length > 0) app.player.shuffleEntries(entries);
  }
</script>

<section class="page" aria-labelledby="favourites-title">
  <h1 id="favourites-title" class="page-title">{t('Favourites')}</h1>
  {#if rows.length === 0}
    <EmptyState title={t('No Favourites Yet')} text={t('Songs you mark with the heart appear here.')} />
  {:else}
    <div class="page-actions">
      <button class="pill primary" onclick={() => play()}><Icon name="play" size={18} />{t('Play')}</button>
      <button class="pill secondary" onclick={shuffle}><Icon name="shuffle" size={18} />{t('Shuffle')}</button>
      <span class="count">{tn('{n} song', '{n} songs', rows.length)}</span>
    </div>
    <SongList {rows} label={t('Favourites')} onplay={play} />
  {/if}
</section>

<style>
  .count {
    margin-inline-start: var(--jf-space-s2);
    color: var(--jf-ink-70);
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
</style>
