<script lang="ts">
  // An album, artist, playlist or folder in a grid: artwork and two lines of type. Luminous
  // (v0.0.6): covers have artwork.radius-grid (16 px) and shadow-2; hovering or focusing lifts the
  // cover by 4 px with shadow-3 and shows a 40 px glass play button in its bottom start corner
  // that plays the album or playlist; a press scales the cover to 97 %. The lift is a translate
  // and the deeper shadow fades in with opacity (compositor work only).
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { itemMenu, secondary, tracksOf } from '../lib/items';
  import { toast } from '../lib/state/toast.svelte';
  import Art from './Art.svelte';
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';

  let { plugin, item, onactivate }: { plugin: string; item: Item; onactivate: () => void } = $props();
  const app = useApp();
  const playable = $derived(item.kind === 'Album' || item.kind === 'Playlist');
  let starting = $state(false);

  async function play() {
    if (starting) return;
    starting = true;
    try {
      const entries = await tracksOf(app, plugin, item);
      if (entries.length > 0) app.player.playEntries(entries, 0);
      else toast(t('Nothing here yet.'));
    } finally {
      starting = false;
    }
  }
</script>

<div class="card">
  <button class="open" onclick={onactivate} aria-label={t('Open {title}', { title: item.title })}>
    <span class="cover"><Art {plugin} {item} size={168} radius="grid" /></span>
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
  <div class="more"><Menu items={itemMenu(app, plugin, item, toast)} label={t('More for {title}', { title: item.title })} /></div>
</div>

<style>
  .card {
    position: relative;
    min-width: 0;
    content-visibility: auto;
    contain-intrinsic-size: auto 230px;
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
    font-size: 15px;
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    min-height: 1.3em;
    font-size: 13px;
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
