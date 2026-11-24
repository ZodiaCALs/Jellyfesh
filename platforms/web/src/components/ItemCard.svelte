<script lang="ts">
  // An album, artist, playlist or folder in a grid: artwork and two lines of type. Luminous
  // (v0.0.6): covers have artwork.radius-grid (16 px) and shadow-2; hovering or focusing lifts the
  // cover by 4 px with shadow-3 and shows a 40 px glass play button in its bottom start corner
  // that plays the album or playlist; a press scales the cover to 97 %. The lift is a translate
  // and the deeper shadow fades in with opacity (compositor work only).
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { itemMenu, type MenuEntry, secondary, tracksOf } from '../lib/items';
  import type { NewEntry } from '../lib/core/types';
  import { toast } from '../lib/state/toast.svelte';
  import Art from './Art.svelte';
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';

  let {
    plugin,
    item,
    onactivate,
    index = null,
    art,
    entries,
    menu,
  }: {
    plugin: string;
    item: Item;
    /** Opens the item; `origin` is the cover, which grows into the page where browsers can. */
    onactivate: (origin: HTMLElement | null) => void;
    /** Its place among the first cards of a list, which rise in one after another as it first shows. */
    index?: number | null;
    /** The artwork to show instead of the item's own (a Jellyfesh playlist shows its first song's). */
    art?: { plugin: string; item: Item } | null;
    /** What its play button plays, instead of the source's songs of the item. */
    entries?: () => NewEntry[];
    /** Its More menu, instead of the item's. */
    menu?: () => MenuEntry[];
  } = $props();
  const app = useApp();
  let cover: HTMLElement | undefined = $state();
  const playable = $derived(item.kind === 'Album' || item.kind === 'Playlist');
  let starting = $state(false);

  async function play() {
    if (starting) return;
    starting = true;
    try {
      const list = entries ? entries() : await tracksOf(app, plugin, item);
      if (list.length > 0) app.player.playEntries(list, 0);
      else toast(t('Nothing here yet.'));
    } finally {
      starting = false;
    }
  }
</script>

<div class="card" class:enter={index !== null} style:--i={index}>
  <button class="open" onclick={() => onactivate(cover ?? null)} aria-label={t('Open {title}', { title: item.title })}>
    <span class="cover" bind:this={cover}>
      {#if art === undefined}
        <Art {plugin} {item} size={168} radius="grid" />
      {:else}
        <Art plugin={art?.plugin ?? null} item={art?.item ?? null} kind={item.kind} size={168} radius="grid" />
      {/if}
    </span>
    <span class="title">{item.title}</span>
    <span class="second">{secondary(item)}</span>
  </button>
  {#if playable}
    <span class="overlay">
      <button class="play" aria-label={t('Play {title}', { title: item.title })} title={t('Play')} disabled={starting} onclick={play}>
        <Icon name="play" size={18} />
      </button>
    </span>
  {/if}
  <div class="more"><Menu items={() => (menu ? menu() : itemMenu(app, plugin, item, toast))} label={t('More for {title}', { title: item.title })} /></div>
</div>

<style>
  .card {
    position: relative;
    min-width: 0;
    content-visibility: auto;
    contain-intrinsic-size: auto 230px;
  }
  /* The first cards of a list rise in one after another (dur-3 Fluid, 30 ms apart). */
  .card.enter {
    animation: jf-page-in var(--jf-motion-dur-3) var(--jf-motion-ease-fluid) calc(var(--i) * 30ms) both;
  }
  :global(:root[data-motion='reduced']) .card.enter {
    animation: none;
  }
  .open {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    padding: 8px;
    border: 0;
    border-radius: var(--jf-radius-r-xl);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .open:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .cover,
  .overlay {
    transition:
      translate var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      scale var(--jf-motion-dur-0) var(--jf-motion-ease-spring);
  }
  .cover {
    position: relative;
    display: block;
    margin-bottom: 8px;
    border-radius: var(--jf-artwork-radius-grid);
    box-shadow: var(--jf-elevation-shadow-2);
  }
  .cover :global(.art) {
    width: 100%;
    height: auto;
    aspect-ratio: 1;
  }
  /* shadow-3 under the lifted cover, faded in with opacity. */
  .cover::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: var(--jf-elevation-shadow-3);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .card:hover .cover,
  .card:hover .overlay,
  .card:has(.open:focus-visible) .cover,
  .card:has(.open:focus-visible) .overlay {
    translate: 0 -4px;
  }
  .card:hover .cover::after,
  .card:has(.open:focus-visible) .cover::after {
    opacity: 1;
  }
  .open:active .cover {
    scale: 0.97;
  }
  .card:has(.open:active) .overlay {
    scale: 0.97;
  }
  :global(:root[data-motion='reduced']) .card .cover,
  :global(:root[data-motion='reduced']) .card .overlay {
    translate: none;
    scale: none;
  }
  /* Over the cover: the card's padding, then a square as wide as the cover. */
  .overlay {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    aspect-ratio: 1;
    pointer-events: none;
  }
  .play {
    position: absolute;
    inset-inline-start: 12px;
    bottom: 12px;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-tint-veil);
    backdrop-filter: blur(var(--jf-glass-veil-blur)) saturate(var(--jf-glass-sat));
    color: var(--jf-ink-90);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge),
      var(--jf-elevation-shadow-1);
    cursor: pointer;
    pointer-events: auto;
    opacity: 0;
    transition:
      opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      scale var(--jf-motion-dur-0) var(--jf-motion-ease-spring);
  }
  :global([data-effects='reduced']) .play {
    background: var(--jf-cloud);
    backdrop-filter: none;
  }
  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
  }
  .play:active:not(:disabled) {
    scale: 0.92;
  }
  .play:focus-visible {
    outline: none;
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge),
      var(--jf-elevation-focus);
  }
  .play:disabled {
    cursor: progress;
  }
  /* Touch screens have no hover: the cover opens the album, whose page has Play. */
  @media (hover: none) {
    .overlay {
      display: none;
    }
  }
  .title,
  .second {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-size: calc(15px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    min-height: 1.3em;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  /* Phones (two lifts' budget of blur is taken by the capsules): the play button is Veil without
     its own blur layer. */
  @media (max-width: 760px) {
    .play {
      backdrop-filter: none;
    }
  }
  .more {
    position: absolute;
    top: 12px;
    right: 12px;
    opacity: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-tint-veil);
    transition: opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .card:hover .more,
  .more:focus-within {
    opacity: 1;
  }
  @media (hover: none) {
    .more {
      opacity: 1;
    }
  }
</style>
