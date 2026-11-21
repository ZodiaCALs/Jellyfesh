<script lang="ts">
  // A source's page for one album, artist, playlist or folder. After Apple Music and Media Player:
  // large artwork (240 px, the full width on a phone), the title, the artist in accent text (it
  // opens the artist), the year and song count, then Play (the page's accent fill) and Shuffle
  // (glass); the songs, numbered on an album; and a footer, "12 songs, 48 minutes". The plugins
  // give no genre, so none is shown.
  import Art from '../components/Art.svelte';
  import Icon from '../components/Icon.svelte';
  import ItemList from '../components/ItemList.svelte';
  import Menu from '../components/Menu.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import { useApp } from '../lib/app';
  import type { Item } from '../lib/core/types';
  import { t, tn } from '../lib/i18n/index.svelte';
  import { artistOf, itemMenu, linkOf, openLink, secondary, summary, tracksFrom } from '../lib/items';
  import { toast } from '../lib/state/toast.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import { explain } from '../lib/state/sources.svelte';

  let { plugin, item }: { plugin: string; item: Item } = $props();
  const app = useApp();

  let items = $state.raw<Item[]>([]);
  let next = $state<string | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let generation = 0;

  async function load(cursor: string | null) {
    if (plugin === LIBRARY) {
      items = app.library.browse(item).items;
      next = null;
      error = null;
      loading = false;
      return;
    }
    const source = app.sources.get(plugin);
    if (!source) {
      error = t('The source of this page is not available.');
      loading = false;
      return;
    }
    const mine = generation;
    loading = true;
    try {
      const page = await source.browse(item, cursor, 100);
      if (mine !== generation) return;
      items = cursor ? [...items, ...page.items] : page.items;
      next = page.next;
      error = null;
    } catch (e) {
      if (mine === generation) error = await explain(e, source);
    } finally {
      if (mine === generation) loading = false;
    }
  }

  $effect(() => {
    void item;
    void app.sources.revision;
    void app.library.revision;
    generation += 1;
    items = [];
    next = null;
    void load(null);
  });

  const tracks = $derived(items.filter((i) => i.kind === 'Track'));
  const playable = $derived(tracks.some((i) => i.playable));
  const complete = $derived(!loading && next === null);
  // Open in {Service} (API 1.1): in More, and the main action of a page whose songs cannot play here.
  const link = $derived((void app.sources.revision, linkOf(app, plugin, item)));
  const opensLink = $derived(link !== null && complete && tracks.length > 0 && !playable);
  const menu = $derived(itemMenu(app, plugin, item, toast, { queue: playable }));
  /** Albums and playlists get the large header. */
  const collection = $derived(item.kind === 'Album' || item.kind === 'Playlist');
  const artist = $derived(item.kind === 'Album' ? artistOf(item) : null);
  const meta = $derived.by(() => {
    if (!collection) return secondary(item);
    const songs = complete ? tracks.length : item.child_count;
    return [item.kind === 'Album' ? item.year?.toString() : null, songs !== null && songs > 0 ? tn('{n} song', '{n} songs', songs) : null]
      .filter(Boolean)
      .join(' · ');
  });

  function play(shuffle: boolean) {
    const { entries } = tracksFrom(plugin, items);
    if (entries.length === 0) return;
    app.player.playEntries(entries, shuffle ? Math.floor(Math.random() * entries.length) : 0, shuffle);
  }
</script>

<section class="page" class:collection aria-labelledby="browse-title">
  <header class="head">
    <span class="art hero-art"><Art {plugin} {item} size={collection ? 360 : 160} radius="grid" /></span>
    <div class="text">
      <p class="kind">{t(item.kind === 'Track' ? 'Song' : item.kind)}</p>
      <h1 id="browse-title">{item.title}</h1>
      {#if collection && item.artist}
        {#if artist}
          <button class="artist" onclick={() => artist && app.nav.open({ view: 'browse', plugin, item: artist })}>{item.artist}</button>
        {:else}
          <p class="artist">{item.artist}</p>
        {/if}
      {/if}
      {#if meta}<p class="meta">{meta}</p>{/if}
      {#if playable}
        <div class="actions">
          <button class="primary" onclick={() => play(false)}><Icon name="play" size={18} />{t('Play')}</button>
          <button class="secondary" onclick={() => play(true)}><Icon name="shuffle" size={18} />{t('Shuffle')}</button>
          <Menu items={menu} label={t('More for {title}', { title: item.title })} />
        </div>
      {:else if opensLink && link}
        <div class="actions">
          <button class="primary" onclick={() => openLink(link.url)}>
            <Icon name="external" size={18} />{t('Open in {service}', { service: link.service })}
          </button>
          <Menu items={menu} label={t('More for {title}', { title: item.title })} />
        </div>
      {:else if link}
        <div class="actions">
          <Menu items={menu} label={t('More for {title}', { title: item.title })} />
        </div>
      {/if}
    </div>
  </header>

  {#if error}
    <div class="problem" role="alert">
      <Icon name="alert" />
      <div>
        <p class="problem-title">{t('This Source Did Not Answer')}</p>
        <p>{error}</p>
      </div>
    </div>
  {:else if !loading && items.length === 0}
    <p class="empty">{t('Nothing here yet.')}</p>
  {/if}

  <ItemList
    {plugin}
    {items}
    numbered={item.kind === 'Album'}
    hasMore={next !== null}
    loadingMore={loading}
    onmore={() => !loading && next && load(next)}
  />
  {#if loading && items.length === 0}
    <Skeleton kind={item.kind === 'Album' || item.kind === 'Playlist' ? 'rows' : 'cards'} count={item.kind === 'Album' ? 8 : 6} />
  {/if}
  {#if complete && tracks.length > 0}
    <p class="footer">{summary(tracks)}</p>
  {/if}
</section>

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--jf-space-s6);
    margin-bottom: var(--jf-space-s7);
  }
  .art {
    flex: none;
    display: block;
    border-radius: var(--jf-artwork-radius-grid);
  }
  .collection .art {
    box-shadow: var(--jf-elevation-shadow-2);
  }
  /* 240 px on wider screens; the image is asked for at the phone's full width. */
  .collection .art :global(.art) {
    width: 240px;
    height: 240px;
  }
  .text {
    flex: 1 1 260px;
    min-width: 0;
  }
  .kind {
    margin: 0;
    font-size: calc(12px * var(--jf-text-scale, 1));
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--jf-ink-70);
  }
  h1 {
    margin: 4px 0;
    font-size: calc(var(--jf-type-text-h2) * var(--jf-text-scale, 1));
    font-weight: 700;
    line-height: 1.1;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .artist {
    display: inline-block;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: var(--jf-radius-r-sm);
    background: none;
    color: var(--jf-accent-text);
    font: inherit;
    font-size: calc(var(--jf-type-text-lede) * var(--jf-text-scale, 1));
    font-weight: 600;
    text-align: start;
  }
  button.artist {
    cursor: pointer;
  }
  button.artist:hover {
    text-decoration: underline;
  }
  .meta {
    margin: 4px 0 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  .actions {
    display: flex;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s4);
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
    transition: scale var(--jf-motion-dur-0) var(--jf-motion-ease-spring);
  }
  .actions button:active {
    scale: 0.97;
  }
  :global(:root[data-motion='reduced']) .actions button:active {
    scale: none;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .primary:hover {
    background: var(--jf-accent-hover);
  }
  /* Glass: Mist with its edge (no live blur needed over the plain page). */
  .secondary {
    background: var(--jf-tint-mist);
    color: var(--jf-accent-text);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .secondary:hover {
    background: var(--jf-tint-veil);
  }
  .actions .secondary:focus-visible {
    box-shadow: var(--jf-elevation-focus);
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s3);
    padding: var(--jf-space-s4);
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  .problem p {
    margin: 0;
  }
  .problem-title {
    font-weight: 700;
  }
  .empty,
  .footer {
    margin: var(--jf-space-s4) var(--jf-space-s2) 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  /* Phones: the artwork takes the full width. */
  @media (max-width: 760px) {
    .collection .art {
      width: 100%;
    }
    .collection .art :global(.art) {
      width: 100%;
      height: auto;
      aspect-ratio: 1;
    }
  }
</style>
