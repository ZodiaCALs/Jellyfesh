<script lang="ts">
  // One item in a list: artwork (or the track number), two lines of type, the length or a
  // chevron, and "More". After Apple Music and Media Player: hover fills the row, a track number
  // turns into a play glyph, and the playing row is accent-soft with accent text and a level glyph
  // that moves only while playing.
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { formatTime, t } from '../lib/i18n/index.svelte';
  import { itemMenu, secondary } from '../lib/items';
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
  }: {
    plugin: string;
    item: Item;
    onactivate: () => void;
    alsoIn?: string[];
    number?: number | null;
    showArt?: boolean;
  } = $props();

  const app = useApp();
  const playing = $derived(
    item.kind === 'Track' &&
      app.player.current?.kind === 'source' &&
      app.player.current.plugin === plugin &&
      app.player.current.id === item.id,
  );
  const unplayable = $derived(item.kind === 'Track' && !item.playable);
  const second = $derived(secondary(item));
  const label = $derived(
    item.kind === 'Track' ? t('Play {title}', { title: item.title }) : t('Open {title}', { title: item.title }),
  );
</script>

<div class="row" class:playing class:unplayable>
  <button class="main" onclick={onactivate} disabled={unplayable} aria-label={label} aria-current={playing ? 'true' : undefined}>
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
        {#if playing}<span class="art-level"><Level moving={app.player.playing} /></span>{/if}
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
    {#if item.kind === 'Track'}
      <span class="length">{item.duration_ms ? formatTime(item.duration_ms) : ''}</span>
    {:else}
      <span class="chevron" aria-hidden="true"><Icon name="forward" size={18} /></span>
    {/if}
  </button>
  <Menu items={itemMenu(app, plugin, item, toast)} label={t('More for {title}', { title: item.title })} />
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
  .row.playing {
    background: var(--jf-accent-soft);
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
    color: var(--jf-ink-50);
    font-size: 14px;
  }
  .glyph {
    display: none;
    color: var(--jf-ink-90);
  }
  /* The number turns into a play glyph on hover or keyboard focus. */
  .row:not(.unplayable):hover .digit,
  .main:focus-visible .digit {
    display: none;
  }
  .row:not(.unplayable):hover .glyph,
  .main:focus-visible .glyph {
    display: grid;
  }
  .art {
    position: relative;
    flex: none;
    display: grid;
  }
  .art-level {
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
    font-size: var(--jf-type-roles-list-title-size);
    line-height: var(--jf-type-roles-list-title-line-height);
    font-weight: var(--jf-type-roles-list-title-weight);
    color: var(--jf-ink-90);
  }
  .playing .title {
    color: var(--jf-accent-text);
    font-weight: 600;
  }
  .second {
    font-size: var(--jf-type-roles-list-secondary-size);
    line-height: var(--jf-type-roles-list-secondary-line-height);
    color: var(--jf-ink-50);
  }
  .also {
    color: var(--jf-ink-50);
  }
  .unplayable .title,
  .unplayable .number {
    color: var(--jf-ink-30);
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
    font-size: 10px;
    font-weight: 700;
    vertical-align: 1px;
  }
  .length {
    flex: none;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-50);
  }
  .chevron {
    flex: none;
    color: var(--jf-ink-30);
  }
</style>
