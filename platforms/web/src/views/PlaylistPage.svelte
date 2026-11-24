<script lang="ts">
  // A Jellyfesh playlist (kept in this browser), laid out as an album page: its first song's
  // cover, "Playlist", the name, the song count, Play, Shuffle and More (Play Next, Play Last,
  // Rename…, Delete Playlist…), then its songs. Songs reorder by dragging their handle (or with ↑
  // and ↓ on it); each song's More adds Remove from Playlist. The footer: "12 songs, 48 minutes".
  import Art from '../components/Art.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import Menu from '../components/Menu.svelte';
  import SongList from '../components/SongList.svelte';
  import { useApp } from '../lib/app';
  import type { Saved } from '../lib/core/types';
  import { t, tn } from '../lib/i18n/index.svelte';
  import { savedEntries, summary } from '../lib/items';
  import { playlistMenu } from '../lib/playlists';

  let { id }: { id: number } = $props();
  const app = useApp();
  const playlist = $derived((void app.collections.revision, app.collections.playlist(id)));
  const entries = $derived.by(() => {
    void app.collections.revision;
    void app.library.revision;
    return app.collections.entries(id);
  });
  const rows = $derived<Saved[]>(entries.map((entry) => ({ plugin: entry.plugin, item: entry.item })));

  function play(index?: number) {
    const { entries: list, start } = savedEntries(rows, index);
    if (list.length > 0) app.player.playEntries(list, start);
  }

  function shuffle() {
    const { entries: list } = savedEntries(rows);
    if (list.length > 0) app.player.shuffleEntries(list);
  }

  function move(from: number, to: number) {
    const entry = entries[from];
    if (entry) app.collections.moveEntry(id, entry.entry_id, to);
  }
</script>

{#if playlist}
  <section class="page" aria-labelledby="playlist-title">
    <header class="head">
      <span class="art hero-art">
        <Art plugin={playlist.cover?.plugin ?? null} item={playlist.cover?.item ?? null} kind="Playlist" size={360} radius="grid" />
      </span>
      <div class="text">
        <p class="kind">{t('Playlist')}</p>
        <h1 id="playlist-title">{playlist.name}</h1>
        <p class="meta">{tn('{n} song', '{n} songs', playlist.entry_count)}</p>
        <div class="actions">
          {#if rows.length > 0}
            <button class="pill primary" onclick={() => play()}><Icon name="play" size={18} />{t('Play')}</button>
            <button class="pill secondary" onclick={shuffle}><Icon name="shuffle" size={18} />{t('Shuffle')}</button>
          {/if}
          <Menu items={() => playlistMenu(app, playlist, () => app.nav.back())} label={t('More for {title}', { title: playlist.name })} />
        </div>
      </div>
    </header>

    {#if rows.length === 0}
      <EmptyState title={t('No Songs Yet')} text={t('Add songs with Add to Playlist… in a song’s More menu.')} />
    {:else}
      <SongList
        {rows}
        label={playlist.name}
        onplay={play}
        reorder={move}
        keyOf={(_, index) => String(entries[index]?.entry_id ?? index)}
        extra={(index) => [
          {
            label: t('Remove from Playlist'),
            action: () => {
              const entry = entries[index];
              if (entry) app.collections.removeFromPlaylist(id, [entry.entry_id]);
            },
          },
        ]}
      />
      <p class="footer">{summary(rows.map((r) => r.item))}</p>
    {/if}
  </section>
{:else}
  <section class="page">
    <EmptyState title={t('This playlist no longer exists.')} text={t('It was deleted, here or in another tab.')} />
  </section>
{/if}

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
    box-shadow: var(--jf-elevation-shadow-2);
  }
  .art :global(.art) {
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
  .footer {
    margin: var(--jf-space-s4) var(--jf-space-s2) 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  @media (max-width: 760px) {
    .art {
      width: 100%;
    }
    .art :global(.art) {
      width: 100%;
      height: auto;
      aspect-ratio: 1;
    }
  }
</style>
