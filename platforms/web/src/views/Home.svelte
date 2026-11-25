<script lang="ts">
  // Home (docs/ui.md): the library's size, then Recently Added (albums), Recently Played and Most
  // Played (songs from anywhere, kept in this browser) and, in a customized look, Favourites, in
  // the look's order; then each signed-in source's own start page (what its `browse` lists at the
  // top). With nothing at all, the welcome: add music folders or connect a server (or drop files,
  // which play without being kept).
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import SongList from '../components/SongList.svelte';
  import { useApp } from '../lib/app';
  import { homeSections } from '../lib/look';
  import type { HomeSection, Item, ItemStats, Saved } from '../lib/core/types';
  import { savedEntries } from '../lib/items';
  import { t, tn } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import { explain } from '../lib/state/sources.svelte';
  import { toast } from '../lib/state/toast.svelte';

  let { onopenfiles }: { onopenfiles: () => void } = $props();
  const app = useApp();
  // The look's Home sections (ADR-0020), in its order.
  const sections = $derived(homeSections(app.look.value.home));
  /** How many songs Recently Played, Most Played and Favourites show. */
  const SONGS = 10;

  interface Shelf {
    plugin: string;
    name: string;
    items: Item[];
    error: string | null;
    loading: boolean;
  }

  let shelves = $state.raw<Shelf[]>([]);
  const library = app.library;
  const recent = $derived((void library.revision, library.recentlyAdded(24)));
  const size = $derived(
    library.stats.tracks > 0
      ? [
          tn('{n} song', '{n} songs', library.stats.tracks),
          tn('{n} album', '{n} albums', library.stats.albums),
          tn('{n} artist', '{n} artists', library.stats.artists),
        ].join(' · ')
      : '',
  );

  const saved = (stats: ItemStats[]): Saved[] => stats.map((s) => ({ plugin: s.plugin, item: s.item }));
  const songs = $derived.by((): Partial<Record<HomeSection, Saved[]>> => {
    void app.collections.revision;
    void library.revision;
    return {
      RecentlyPlayed: saved(app.collections.recentlyPlayed(SONGS)),
      MostPlayed: saved(app.collections.mostPlayed(SONGS)),
      Favourites: saved(app.collections.favorites(0, SONGS)),
    };
  });

  function songsTitle(section: HomeSection): string {
    switch (section) {
      case 'RecentlyPlayed':
        return t('Recently Played');
      case 'MostPlayed':
        return t('Most Played');
      default:
        return t('Favourites');
    }
  }

  function playFrom(rows: Saved[], index: number) {
    const { entries, start } = savedEntries(rows, index);
    if (entries.length > 0) app.player.playEntries(entries, start);
  }

  async function addFolder() {
    try {
      await library.addFolder();
    } catch (error) {
      toast(error instanceof Error ? error.message : String(error));
    }
  }

  $effect(() => {
    void app.sources.revision;
    const plugins = app.sources.usable('Browse');
    let live = true;
    shelves = plugins.map((p) => ({ plugin: p.id, name: p.name, items: [], error: null, loading: true }));
    for (const plugin of plugins) {
      plugin
        .browse(null, null, 60)
        .then((page) => ({ items: page.items, error: null }))
        .catch(async (error) => ({ items: [] as Item[], error: await explain(error, plugin) }))
        .then((result) => {
          if (!live) return;
          shelves = shelves.map((s) => (s.plugin === plugin.id ? { ...s, ...result, loading: false } : s));
        });
    }
    return () => {
      live = false;
    };
  });
</script>

<section class="page" aria-labelledby="home-title">
  <h1 id="home-title" class="page-title">{t('Home')}</h1>
  {#if size && sections.includes('Summary')}<p class="size">{size}</p>{/if}
  {#if shelves.length === 0 && library.isEmpty && library.folders.length === 0}
    <div class="welcome">
      <h2>{t('Welcome to Jellyfesh')}</h2>
      <p>{t('Add your music folders, or connect a server in Sources.')}</p>
      <div class="actions">
        <button class="primary" onclick={addFolder}><Icon name="folder-plus" size={18} />{t('Add Music Folder…')}</button>
        <button class="secondary" onclick={() => app.nav.switchTab('sources')}><Icon name="sources" size={18} />{t('Add a Source')}</button>
        <button class="secondary" onclick={onopenfiles}><Icon name="file" size={18} />{t('Open Files…')}</button>
      </div>
      <p class="hint">{t('Files you open or drop on this page play without being added.')}</p>
    </div>
  {/if}
  {#each sections as section (section)}
    {#if section === 'RecentlyAdded' && recent.length > 0}
      <section class="shelf" aria-labelledby="recent-title">
        <h2 id="recent-title" class="section">{t('Recently Added')}</h2>
        <ItemList plugin={LIBRARY} items={recent} />
      </section>
    {:else if section !== 'Summary' && section !== 'RecentlyAdded' && (songs[section]?.length ?? 0) > 0}
      {@const rows = songs[section] ?? []}
      <section class="shelf" aria-labelledby="home-{section}">
        <h2 id="home-{section}" class="section">{songsTitle(section)}</h2>
        <SongList {rows} label={songsTitle(section)} onplay={(index) => playFrom(rows, index)} />
      </section>
    {/if}
  {/each}
  {#each shelves as shelf (shelf.plugin)}
    <section class="shelf" aria-label={shelf.name}>
      <h2 class="section">{shelf.name}</h2>
      {#if shelf.error}
        <p class="problem" role="alert"><Icon name="alert" size={18} />{shelf.error}</p>
      {:else if shelf.loading}
        <Skeleton kind="tiles" count={4} />
      {:else}
        <ItemList plugin={shelf.plugin} items={shelf.items} />
      {/if}
    </section>
  {/each}
</section>

<style>
  .size {
    margin: calc(-1 * var(--jf-space-s3)) 0 var(--jf-space-s6);
    color: var(--jf-ink-70);
  }
  .hint {
    margin: var(--jf-space-s4) 0 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .welcome {
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s7);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .welcome h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: calc(var(--jf-type-text-h3) * var(--jf-text-scale, 1));
  }
  .welcome p {
    margin: 0;
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s5);
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .actions button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .secondary {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  .shelf + .shelf {
    margin-top: var(--jf-space-s8);
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    align-items: flex-start;
    color: var(--jf-ink-70);
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
</style>
