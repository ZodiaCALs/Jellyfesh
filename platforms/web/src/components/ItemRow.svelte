<script lang="ts">
  // One item in a list: artwork (or the track number), two lines of type, the length or a
  // chevron, and "More". After Apple Music and Media Player: hover fills the row, and the playing
  // row is accent-soft with accent text and a level glyph that moves only while playing. A song
  // that cannot play here but has a link (API 1.1) opens it in its service instead, next to its
  // reason (docs/ui.md, Open in {Service}). A favourite shows a small filled heart beside its
  // length. Rows and cards (v0.0.9): while the pointer is over the row or it has keyboard focus,
  // a play button shows over its artwork (or its number) and More at its end; the play button is
  // the row's own, so it plays from that row as activating the row does. Touch screens have no
  // hover and show More on every row. Right-click opens the More menu at the pointer, and so does
  // a long press on a touch screen (src/lib/contextMenu.ts).
  import { useApp } from '../lib/app';
  import { contextMenu } from '../lib/contextMenu';
  import type { Item } from '../lib/core/types';
  import { formatTime, t } from '../lib/i18n/index.svelte';
  import { itemMenu, linkOf, type MenuEntry, openLink, secondary } from '../lib/items';
  import { LIBRARY, LIBRARY_ID } from '../lib/state/library.svelte';
  import { toast } from '../lib/state/toast.svelte';
  import Art from './Art.svelte';
  import Icon from './Icon.svelte';
  import Level from './Level.svelte';
  import Menu from './Menu.svelte';

  let {
    plugin,
    item,
    onactivate,
    alsoIn = [],
    number = null,
    showArt = true,
    enter = null,
    extra,
  }: {
    plugin: string;
    item: Item;
    onactivate: () => void;
    alsoIn?: string[];
    number?: number | null;
    showArt?: boolean;
    /** Its place among the first rows of a list, which rise in one after another as it first shows. */
    enter?: number | null;
    /** Entries its More menu adds after Add to Playlist… (a playlist's Remove from Playlist). */
    extra?: () => MenuEntry[];
  } = $props();

  const app = useApp();
  const playing = $derived(
    item.kind === 'Track' &&
      (plugin === LIBRARY
        ? app.player.current?.kind === 'file' && app.player.current.id === `${LIBRARY_ID}${item.id}`
        : app.player.current?.kind === 'source' && app.player.current.plugin === plugin && app.player.current.id === item.id),
  );
  const unplayable = $derived(item.kind === 'Track' && !item.playable);
  // Known once the source's package has been read (it has, for anything it listed).
  const link = $derived((void app.sources.revision, linkOf(app, plugin, item)));
  /** The row's main action opens the item in its service. */
  const opensLink = $derived(unplayable && link !== null);
  const second = $derived(secondary(item));
  const label = $derived(
    opensLink && link
      ? t('Open {title} in {service}', { title: item.title, service: link.service })
      : item.kind === 'Track'
        ? t('Play {title}', { title: item.title })
        : t('Open {title}', { title: item.title }),
  );

  const favourite = $derived(item.kind === 'Track' && (void app.collections.revision, app.collections.isFavorite(plugin, item.id)));
  /** Its main action plays it here: the play button shows over its artwork on hover or focus. */
  const plays = $derived(item.kind === 'Track' && !unplayable);

  let main: HTMLButtonElement | undefined = $state();
  let menu: ReturnType<typeof Menu> | undefined = $state();

  function activate() {
    if (opensLink && link) openLink(link.url);
    else onactivate();
  }
</script>

<div
  class="row"
  class:playing
  class:unplayable
  class:linked={opensLink}
  class:enter={enter !== null}
  style:--i={enter}
  use:contextMenu={(place) => menu?.openAt(place, main ?? null)}
>
  <button
    class="main"
    bind:this={main}
    onclick={activate}
    disabled={unplayable && !opensLink}
    aria-label={label}
    aria-current={playing ? 'true' : undefined}
  >
    {#if number !== null}
      <span class="number" aria-hidden="true">
        {#if playing}
          <Level moving={app.player.playing} />
        {:else}
          <span class="digit">{number}</span>
          <span class="glyph"><Icon name="play" size={16} /></span>
        {/if}
      </span>
    {:else if showArt}
      <span class="art">
        <Art {plugin} {item} size={40} />
        {#if playing}
          <span class="art-level"><Level moving={app.player.playing} /></span>
        {:else if plays}
          <span class="art-play" aria-hidden="true"><Icon name="play" size={18} /></span>
        {/if}
      </span>
    {/if}
    <span class="text">
      <span class="title">{item.title}{#if item.explicit}<span class="badge" title={t('Explicit')}>E</span>{/if}</span>
      {#if second || alsoIn.length}
        <span class="second">
          {second}{#if alsoIn.length}<span class="also">{` · ${t('Also in {sources}', { sources: alsoIn.join(', ') })}`}</span>{/if}
          {#if unplayable && item.unplayable?.message}<span class="also">{` · ${item.unplayable.message}`}</span>{/if}
        </span>
      {/if}
    </span>
    {#if opensLink}
      <span class="length">{item.duration_ms ? formatTime(item.duration_ms) : ''}</span>
      <span class="chevron" aria-hidden="true"><Icon name="external" size={18} /></span>
    {:else if item.kind === 'Track'}
      {#if favourite}<span class="heart" title={t('Favourite')}><Icon name="heart-filled" size={14} /></span>{/if}
      <span class="length">{item.duration_ms ? formatTime(item.duration_ms) : ''}</span>
    {:else}
      <span class="chevron" aria-hidden="true"><Icon name="forward" size={18} /></span>
    {/if}
  </button>
  <span class="more">
    <Menu
      bind:this={menu}
      items={() => itemMenu(app, plugin, item, toast, { extra: extra?.() ?? [] })}
      label={t('More for {title}', { title: item.title })}
    />
  </span>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s1);
    min-height: var(--jf-player-row-height);
    border-radius: var(--jf-radius-r-md);
    content-visibility: auto;
    contain-intrinsic-size: auto var(--jf-player-row-height);
  }
  .row:hover {
    background: var(--jf-fill);
  }
  /* Press feedback: the row darkens while pressed. */
  .row:has(.main:active:not(:disabled)) {
    background: var(--jf-fill-press);
  }
  .row.playing {
    background: var(--jf-accent-soft);
  }
  /* The first rows of a list rise in one after another (dur-3 Fluid, 24 ms apart). */
  .row.enter {
    animation: jf-page-in var(--jf-motion-dur-3) var(--jf-motion-ease-fluid) calc(var(--i) * 24ms) both;
  }
  :global(:root[data-motion='reduced']) .row.enter {
    animation: none;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-height: var(--jf-player-row-height);
    padding: 6px var(--jf-space-s2);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .main:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .main:disabled {
    cursor: default;
  }
  .number {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-70);
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .glyph {
    display: none;
    color: var(--jf-ink-90);
  }
  /* The number turns into a play glyph on hover or keyboard focus. */
  .row:not(.unplayable) .main:focus-visible .digit {
    display: none;
  }
  .row:not(.unplayable) .main:focus-visible .glyph {
    display: grid;
  }
  .art {
    position: relative;
    flex: none;
    display: grid;
  }
  .art-level,
  .art-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: var(--jf-artwork-radius-list);
    background: var(--jf-scrim);
  }
  .art-level :global(.level) {
    color: var(--jf-on-accent);
  }
  /* The play button over the artwork, on hover or keyboard focus. */
  .art-play {
    color: var(--jf-on-accent);
    opacity: 0;
    transition: opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .main:focus-visible .art-play {
    opacity: 1;
  }
  .more {
    flex: none;
    display: grid;
  }
  /* Pointer screens: the play button and More show while the pointer is over the row, or it has
     the focus (More's menu open included). */
  @media (hover: hover) {
    .row:not(.unplayable):hover .digit {
      display: none;
    }
    .row:not(.unplayable):hover .glyph {
      display: grid;
    }
    .row:hover .art-play {
      opacity: 1;
    }
    .more {
      opacity: 0;
      transition: opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
    }
    .row:hover .more,
    .row:focus-within .more {
      opacity: 1;
    }
  }
  /* Touch screens: a long press opens the menu, so it selects no text and shows no callout. */
  @media (hover: none) {
    .row {
      -webkit-touch-callout: none;
      -webkit-user-select: none;
      user-select: none;
    }
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title,
  .second {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-size: calc(var(--jf-type-roles-list-title-size) * var(--jf-text-scale, 1));
    line-height: var(--jf-type-roles-list-title-line-height);
    font-weight: var(--jf-type-roles-list-title-weight);
    color: var(--jf-ink-90);
  }
  .playing .title {
    color: var(--jf-accent-text);
    font-weight: 600;
  }
  .second {
    font-size: calc(var(--jf-type-roles-list-secondary-size) * var(--jf-text-scale, 1));
    line-height: var(--jf-type-roles-list-secondary-line-height);
    color: var(--jf-ink-70);
  }
  .also {
    color: var(--jf-ink-70);
  }
  .unplayable .title,
  .unplayable .number {
    color: var(--jf-ink-30);
  }
  /* It opens in its service: still readable, as a link is something to do. */
  .unplayable.linked .title {
    color: var(--jf-ink-70);
  }
  .linked .main {
    cursor: pointer;
  }
  .badge {
    display: inline-grid;
    place-items: center;
    margin-inline-start: 6px;
    width: 16px;
    height: 16px;
    border-radius: var(--jf-radius-r-xs);
    background: var(--jf-ink-10);
    color: var(--jf-ink-70);
    font-size: calc(10px * var(--jf-text-scale, 1));
    font-weight: 700;
    vertical-align: 1px;
  }
  .length {
    flex: none;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-70);
  }
  .chevron {
    flex: none;
    color: var(--jf-ink-30);
  }
  .heart {
    flex: none;
    display: grid;
    color: var(--jf-accent-text);
  }

</style>
