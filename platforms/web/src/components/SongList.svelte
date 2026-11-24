<script lang="ts">
  // Songs from anywhere (Favourites, a Jellyfesh playlist, Recently Played): each row with its own
  // source, in one grouped surface. Activating a row plays the list from it. A long list draws only
  // the rows near the screen (the page's column is the scroller; rows are one height), so a few
  // thousand songs cost what a screenful does. With `reorder`, each row has a handle: drag it, or
  // focus it and press ↑ or ↓, to move the song (docs/ui.md: "Songs reorder by dragging").
  import { untrack } from 'svelte';
  import type { Saved } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import type { MenuEntry } from '../lib/items';
  import ItemRow from './ItemRow.svelte';

  let {
    rows,
    label,
    onplay,
    extra,
    reorder,
    keyOf = (row: Saved, index: number) => `${row.plugin}\u001f${row.item.id}\u001f${index}`,
  }: {
    rows: Saved[];
    label: string;
    onplay: (index: number) => void;
    extra?: (index: number) => MenuEntry[];
    reorder?: (from: number, to: number) => void;
    keyOf?: (row: Saved, index: number) => string;
  } = $props();

  /** Lists this long draw only the rows near the screen. */
  const VIRTUAL_FROM = 60;
  /** Rows drawn beyond each edge of the screen. */
  const OVERSCAN = 12;
  /** The first rows of a list rise in one after another as it first shows. */
  const STAGGERED = 16;

  let list: HTMLDivElement | undefined = $state();
  let rowHeight = $state(0);
  let start = $state(0);
  let end = $state(VIRTUAL_FROM);
  const virtual = $derived(rows.length >= VIRTUAL_FROM);
  const shown = $derived(virtual ? rows.slice(start, end) : rows);
  const first = $derived(virtual ? start : 0);

  function scrollerOf(element: HTMLElement): HTMLElement {
    for (let node = element.parentElement; node; node = node.parentElement) {
      const overflow = getComputedStyle(node).overflowY;
      if (overflow === 'auto' || overflow === 'scroll') return node;
    }
    return document.scrollingElement as HTMLElement;
  }

  /** Which rows are near the screen; only a change of rows redraws. */
  function measure() {
    if (!list) return;
    if (!virtual) {
      start = 0;
      end = rows.length;
      return;
    }
    const sample = list.querySelector<HTMLElement>('.slot');
    if (sample && sample.offsetHeight > 0) rowHeight = sample.offsetHeight;
    const height = rowHeight || 56;
    const scroller = scrollerOf(list);
    const view = scroller.getBoundingClientRect();
    const top = list.getBoundingClientRect().top + list.clientTop;
    const from = Math.floor((view.top - top) / height) - OVERSCAN;
    const to = Math.ceil((view.bottom - top) / height) + OVERSCAN;
    start = Math.max(0, Math.min(rows.length, from));
    end = Math.max(start, Math.min(rows.length, Math.max(to, start + 1)));
  }

  $effect(() => {
    const element = list;
    if (!element || !virtual) return;
    const scroller = scrollerOf(element);
    let frame = 0;
    const later = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        measure();
      });
    };
    scroller.addEventListener('scroll', later, { passive: true });
    const observer = new ResizeObserver(later);
    observer.observe(scroller);
    untrack(measure);
    return () => {
      scroller.removeEventListener('scroll', later);
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  });
  // The list grew or shrank: the window follows.
  $effect(() => {
    void rows.length;
    untrack(measure);
  });

  // ---- reordering -----------------------------------------------------------------------------

  let dragging = $state<{ from: number; to: number } | null>(null);

  function targetAt(clientY: number): number {
    if (!list) return 0;
    const height = rowHeight || list.querySelector<HTMLElement>('.slot')?.offsetHeight || 56;
    const top = list.getBoundingClientRect().top + list.clientTop;
    return Math.max(0, Math.min(rows.length - 1, Math.floor((clientY - top) / height)));
  }

  function grab(event: PointerEvent, index: number) {
    if (!reorder || event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    dragging = { from: index, to: index };
  }

  function drag(event: PointerEvent) {
    if (dragging) dragging = { ...dragging, to: targetAt(event.clientY) };
  }

  function drop() {
    const moved = dragging;
    dragging = null;
    if (moved && moved.from !== moved.to) reorder?.(moved.from, moved.to);
  }

  function keys(event: KeyboardEvent, index: number) {
    if (!reorder) return;
    const to = event.key === 'ArrowUp' ? index - 1 : event.key === 'ArrowDown' ? index + 1 : null;
    if (to === null || to < 0 || to >= rows.length) return;
    event.preventDefault();
    reorder(index, to);
    // Keep the focus on the moved song's handle.
    requestAnimationFrame(() => list?.querySelector<HTMLElement>(`[data-index="${to}"] .handle`)?.focus());
  }
</script>

<div class="rows surface-mist" class:reordering={dragging !== null} bind:this={list} role="list" aria-label={label}>
  {#if virtual && start > 0}<div aria-hidden="true" style:height="{start * (rowHeight || 56)}px"></div>{/if}
  {#each shown as row, i (keyOf(row, first + i))}
    {@const index = first + i}
    <div
      class="slot"
      role="listitem"
      data-index={index}
      class:moving={dragging?.from === index}
      class:above={dragging !== null && dragging.to === index && dragging.to < dragging.from}
      class:below={dragging !== null && dragging.to === index && dragging.to > dragging.from}
    >
      {#if reorder}
        <button
          class="handle"
          aria-label={t('Move {title}', { title: row.item.title })}
          title={t('Drag to move, or press ↑ or ↓')}
          onpointerdown={(event) => grab(event, index)}
          onpointermove={drag}
          onpointerup={drop}
          onpointercancel={() => (dragging = null)}
          onkeydown={(event) => keys(event, index)}
        >
          <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="9" cy="6" r="1.6" /><circle cx="15" cy="6" r="1.6" /><circle cx="9" cy="12" r="1.6" />
            <circle cx="15" cy="12" r="1.6" /><circle cx="9" cy="18" r="1.6" /><circle cx="15" cy="18" r="1.6" />
          </svg>
        </button>
      {/if}
      <div class="row-body">
        <ItemRow
          plugin={row.plugin}
          item={row.item}
          enter={!virtual && index < STAGGERED ? index : null}
          onactivate={() => onplay(index)}
          extra={extra ? () => extra(index) : undefined}
        />
      </div>
    </div>
  {/each}
  {#if virtual && end < rows.length}<div aria-hidden="true" style:height="{(rows.length - end) * (rowHeight || 56)}px"></div>{/if}
</div>

<style>
  /* A list is a grouped surface: Mist without a blur layer, with shadow-1. */
  .rows {
    display: flex;
    flex-direction: column;
    padding: var(--jf-space-s1) var(--jf-space-s2);
    border-radius: var(--jf-radius-r-2xl);
  }
  .slot {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--jf-radius-r-md);
  }
  .row-body {
    flex: 1;
    min-width: 0;
  }
  .handle {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 40px;
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: var(--jf-ink-30);
    cursor: grab;
    touch-action: none;
  }
  .handle:hover {
    color: var(--jf-ink-70);
  }
  .handle:focus-visible {
    outline: none;
    color: var(--jf-ink-90);
    box-shadow: var(--jf-elevation-focus);
  }
  .reordering .handle {
    cursor: grabbing;
  }
  .moving {
    opacity: 0.5;
  }
  .above {
    box-shadow: inset 0 2px 0 var(--jf-accent);
  }
  .below {
    box-shadow: inset 0 -2px 0 var(--jf-accent);
  }
  @media (max-width: 760px) {
    .rows {
      padding: 2px var(--jf-space-s1);
    }
  }
</style>
