<script lang="ts" generics="T">
  // A long list that renders only the rows in view (and a few around them), fetching them from
  // the core in windows: a queue of thousands costs what a screenful does.
  import type { Snippet } from 'svelte';

  let {
    count,
    rowHeight,
    rows,
    key,
    row,
    revision = 0,
    label,
  }: {
    count: number;
    rowHeight: number;
    rows: (start: number, end: number) => T[];
    key: (item: T) => string | number;
    row: Snippet<[T, number]>;
    revision?: number;
    label: string;
  } = $props();

  const OVERSCAN = 6;
  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(600);

  $effect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(() => (height = viewport?.clientHeight ?? height));
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN));
  const end = $derived(Math.min(count, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN));
  const visible = $derived((void revision, rows(start, end)));
</script>

<div class="viewport" bind:this={viewport} onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)} role="list" aria-label={label}>
  <div style:height="{start * rowHeight}px" aria-hidden="true"></div>
  {#each visible as item, i (key(item))}
    <div class="slot" role="listitem" style:height="{rowHeight}px">{@render row(item, start + i)}</div>
  {/each}
  <div style:height="{Math.max(0, count - end) * rowHeight}px" aria-hidden="true"></div>
</div>

<style>
  .viewport {
    height: 100%;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .slot {
    box-sizing: border-box;
  }
</style>
