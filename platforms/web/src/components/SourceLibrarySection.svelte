<script lang="ts">
  // One source's section on Favourites or Playlists (ADR-0024, docs/ui.md): its name, the credit
  // its terms ask for ("Content from Spotify"), when the kept lists were updated ("Updated 5
  // minutes ago"), then its lists: songs as rows that play, albums, artists and playlists as cards
  // that open their pages (a playlist whose songs the service lists only in its own app says so
  // there), each with Show More while the source has more. Read-only: the heart marks a song in
  // Jellyfesh only.
  import { useApp } from '../lib/app';
  import type { Item, SourceLibrary, SourceLibraryPart } from '../lib/core/types';
  import { t, updatedText } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';
  import ItemList from './ItemList.svelte';

  let { source }: { source: SourceLibrary } = $props();
  const app = useApp();

  // The age line ticks with the minute, while the page shows.
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(timer);
  });

  const updated = $derived.by(() => {
    const list = source.lists.find((l) => !l.loading && l.refreshed_at_ms !== null);
    return list?.refreshed_at_ms ? updatedText(list.refreshed_at_ms, now) : null;
  });

  function title(part: SourceLibraryPart): string {
    switch (part) {
      case 'liked_songs':
        return t('Songs');
      case 'liked_albums':
        return t('Albums');
      case 'liked_artists':
        return t('Artists');
      case 'playlists':
        return t('Playlists');
    }
  }
</script>

<section class="source" aria-labelledby="source-{source.plugin}">
  <h2 id="source-{source.plugin}" class="section">{source.name}</h2>
  {#if source.attribution}
    <p class="note">{t('Content from {service}', { service: source.name })}</p>
  {/if}
  {#if updated}<p class="note quiet">{updated}</p>{/if}
  {#each source.lists as list (list.part)}
    {@const key = `${source.plugin}|${list.part}`}
    {@const loadingMore = app.sourceLibraries.loadingMore.has(key)}
    {@const listed = new Map(list.entries.map((e) => [e.item, e.songs_listed]))}
    <div class="list" role="group" aria-label={title(list.part)}>
      {#if source.lists.length > 1}<h3>{title(list.part)}</h3>{/if}
      {#if list.error}
        <p class="problem"><Icon name="alert" size={18} />{list.error}</p>
      {/if}
      {#if list.entries.length === 0 && list.loading}
        <p class="note">{t('Loading…')}</p>
      {/if}
      <ItemList plugin={source.plugin} items={list.entries.map((e) => e.item)} listed={(item: Item) => listed.get(item) ?? true} />
      {#if list.next}
        <div class="show-more">
          <button class="pill secondary" disabled={loadingMore} onclick={() => app.sourceLibraries.more(source.plugin, list.part)}
            >{loadingMore ? t('Loading…') : t('Show More')}</button
          >
        </div>
      {/if}
    </div>
  {/each}
</section>

<style>
  .source {
    margin-top: var(--jf-space-s8);
  }
  .source h2 {
    margin-bottom: 2px;
  }
  .note {
    margin: 0 0 var(--jf-space-s2);
    color: var(--jf-ink-70);
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .quiet {
    color: var(--jf-ink-50);
  }
  .list {
    margin-top: var(--jf-space-s4);
  }
  h3 {
    margin: 0 0 var(--jf-space-s2);
    color: var(--jf-ink-70);
    font-size: calc(14px * var(--jf-text-scale, 1));
    font-weight: 600;
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
  .show-more {
    display: flex;
    justify-content: center;
    margin-top: var(--jf-space-s3);
  }
</style>
