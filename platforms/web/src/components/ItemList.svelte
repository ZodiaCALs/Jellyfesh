<script lang="ts">
  // A source's items: containers in a grid, tracks in rows. Activating a track plays the list's
  // tracks from it; a container opens. Lazy: rows render when near the screen (content-visibility),
  // artwork loads when visible, and more pages load as the end comes into view.
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { tracksFrom } from '../lib/items';
  import Icon from './Icon.svelte';
  import ItemCard from './ItemCard.svelte';
  import ItemRow from './ItemRow.svelte';

  let {
    plugin,
    items,
    numbered = false,
    hasMore = false,
    loadingMore = false,
    onmore,
    alsoIn,
    onplay,
  }: {
    plugin: string;
    items: Item[];
    numbered?: boolean;
    hasMore?: boolean;
    loadingMore?: boolean;
    onmore?: () => void;
    alsoIn?: (item: Item) => string[];
    /** Plays from a track instead of the tracks listed (a list longer than what is loaded). */
    onplay?: (item: Item) => void;
  } = $props();

  const app = useApp();
  const folders = $derived(items.filter((i) => i.kind === 'Folder'));
  const containers = $derived(items.filter((i) => i.kind !== 'Track' && i.kind !== 'Folder'));
  const tracks = $derived(items.filter((i) => i.kind === 'Track'));
  let sentinel: HTMLElement | undefined = $state();

  $effect(() => {
    if (!sentinel || !hasMore || !onmore) return;
    const more = onmore;
    const observer = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && more(), {
      rootMargin: '400px',
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  /** How many cards and rows rise in one after another as a list first shows. */
  const STAGGERED = 16;

  function activate(item: Item, origin: HTMLElement | null = null) {
    if (item.kind === 'Track' && onplay) {
      onplay(item);
    } else if (item.kind === 'Track') {
      const { entries, start } = tracksFrom(plugin, tracks, item);
      app.player.playEntries(entries, start);
    } else {
      app.nav.open({ view: 'browse', plugin, item }, origin);
    }
  }
</script>

{#if folders.length}
  <div class="folders">
    {#each folders as item (item.id)}
      <button class="folder" onclick={() => activate(item)} aria-label={t('Open {title}', { title: item.title })}>
        <Icon name="folder" /><span>{item.title}</span><Icon name="forward" size={16} />
      </button>
    {/each}
  </div>
{/if}
{#if containers.length}
  <div class="grid">
    {#each containers as item, index (item.kind + item.id)}
      <ItemCard {plugin} {item} index={index < STAGGERED ? index : null} onactivate={(origin) => activate(item, origin)} />
    {/each}
  </div>
{/if}
{#if tracks.length}
  <div class="rows surface-mist">
    {#each tracks as item, index (item.id + ':' + index)}
      <ItemRow
        {plugin}
        {item}
        enter={index < STAGGERED ? index : null}
        number={numbered ? (item.track_number ?? index + 1) : null}
        alsoIn={alsoIn?.(item) ?? []}
        onactivate={() => activate(item)}
      />
    {/each}
  </div>
{/if}
{#if hasMore}
  <div class="more" bind:this={sentinel}>
    <button onclick={() => onmore?.()} disabled={loadingMore}>{t('Load More')}</button>
  </div>
{/if}

<style>
  .folders {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--jf-space-s2);
  }
  .folder {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-height: 56px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-xl);
    background: var(--jf-tint-mist);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    text-align: start;
    cursor: pointer;
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .folder span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .folder :global(svg:first-child) {
    color: var(--jf-accent-text);
  }
  .folder :global(svg:last-child) {
    color: var(--jf-ink-30);
  }
  .folder:hover {
    background: var(--jf-fill-hover);
  }
  .folder:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .folders + .grid,
  .folders + .rows {
    margin-top: var(--jf-space-s6);
  }
  /* Luminous: large covers, four to a row on a 1280 px window. */
  /* Each card's button reaches 8 px beyond its cover (a larger target, and room for the lift);
     the grid steps out by as much, so covers line up with the page's titles. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(196px, 1fr));
    gap: var(--jf-space-s3) var(--jf-space-s3);
    margin-inline: -8px;
  }
  /* A list is a grouped surface: Mist without a blur layer, with shadow-1. */
  .rows {
    display: flex;
    flex-direction: column;
    padding: var(--jf-space-s1) var(--jf-space-s2);
    border-radius: var(--jf-radius-r-2xl);
  }
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
      gap: var(--jf-space-s1);
    }
    .rows {
      padding: 2px var(--jf-space-s1);
    }
  }
  .grid + .rows {
    margin-top: var(--jf-space-s6);
  }
  .more {
    display: flex;
    justify-content: center;
    padding: var(--jf-space-s4);
  }
  .more button {
    min-height: 40px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .more button:hover {
    background: var(--jf-fill-hover);
  }
</style>
