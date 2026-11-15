<script lang="ts" module>
  // One observer for every thumbnail: artwork is asked for only once a row is near the screen.
  const waiting = new Map<Element, () => void>();
  let observer: IntersectionObserver | null = null;
  function observe(element: Element, visible: () => void): () => void {
    observer ??= new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          waiting.get(entry.target)?.();
          waiting.delete(entry.target);
          observer?.unobserve(entry.target);
        }
      },
      { rootMargin: '200px' },
    );
    waiting.set(element, visible);
    observer.observe(element);
    return () => {
      waiting.delete(element);
      observer?.unobserve(element);
    };
  }
</script>

<script lang="ts">
  import { useApp } from '../lib/app';
  import type { Item, ItemKind } from '../lib/core/types';
  import { LIBRARY } from '../lib/state/library.svelte';
  import Icon from './Icon.svelte';

  let {
    plugin = null,
    item = null,
    src = null,
    size = 48,
    kind = 'Track',
    radius = 'list',
    alt = '',
  }: {
    plugin?: string | null;
    item?: Item | null;
    src?: string | null;
    size?: number;
    kind?: ItemKind;
    radius?: 'list' | 'grid' | 'hero';
    alt?: string;
  } = $props();

  const { sources, artworks, library } = useApp();
  let element: HTMLElement | undefined = $state();
  let resolved = $state<string | null>(null);
  let failed = $state(false);
  const shown = $derived(src ?? resolved);

  $effect(() => {
    const target = element;
    const current = item;
    const from = plugin;
    resolved = null;
    failed = false;
    if (!target || !current || src) return;
    let live = true;
    const stop = observe(target, () => {
      if (from === LIBRARY) {
        // The library's covers come from its own store.
        const url = current.artwork?.url;
        if (!url) return;
        void library.coverUrl(url).then((found) => {
          if (live) resolved = found;
        });
        return;
      }
      const owner = from ? sources.get(from) : undefined;
      void artworks
        .ref(owner, current, Math.min(1024, Math.round(size * devicePixelRatio)))
        .then((image) => (image ? artworks.url(image) : null))
        .then((url) => {
          if (live) resolved = url;
        });
    });
    return () => {
      live = false;
      stop();
    };
  });

  const glyph = $derived(
    { Track: 'note', Album: 'album', Artist: 'artist', Playlist: 'playlist', Folder: 'folder' }[item?.kind ?? kind] ?? 'note',
  );
</script>

<span class="art {radius}" class:round={(item?.kind ?? kind) === 'Artist'} style:--size="{size}px" bind:this={element}>
  {#if shown && !failed}
    <img src={shown} {alt} loading="lazy" decoding="async" draggable="false" onerror={() => (failed = true)} />
  {:else}
    <span class="placeholder" aria-hidden="true"><Icon name={glyph} size={Math.max(16, Math.round(size * 0.42))} /></span>
  {/if}
</span>

<style>
  .art {
    position: relative;
    display: block;
    flex: none;
    width: var(--size);
    height: var(--size);
    overflow: hidden;
    background: var(--jf-fill);
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  .art.list {
    border-radius: var(--jf-artwork-radius-list);
  }
  .art.grid {
    border-radius: var(--jf-artwork-radius-grid);
  }
  .art.hero {
    border-radius: var(--jf-artwork-radius-hero);
  }
  .art.round {
    border-radius: var(--jf-radius-r-full);
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--jf-ink-30);
    background: linear-gradient(135deg, var(--jf-aurora-1), var(--jf-aurora-2));
    opacity: 0.55;
  }
</style>
