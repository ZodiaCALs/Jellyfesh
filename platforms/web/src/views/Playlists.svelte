<script lang="ts">
  // Playlists (docs/ui.md): New Playlist… (it asks for the name first), then the Jellyfesh
  // playlists by name, each with its first song's cover; kept in this browser. Then one section
  // per signed-in source that keeps playlists (API 1.3), refreshed when the page opens. Empty: "No
  // Playlists Yet", "Make a playlist to keep songs together.", New Playlist….
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import ItemCard from '../components/ItemCard.svelte';
  import SourceLibrarySection from '../components/SourceLibrarySection.svelte';
  import { useSourceLibrary } from '../lib/state/sourceLibrary.svelte';
  import { useApp } from '../lib/app';
  import type { Item, PlaylistSummary } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { playlistItem, playlistMenu, playlistSongs } from '../lib/playlists';
  import { savedEntries } from '../lib/items';
  import { sheets } from '../lib/state/sheets.svelte';

  const app = useApp();
  const playlists = $derived((void app.collections.revision, app.collections.playlists()));

  function create() {
    sheets.naming = {
      title: t('New Playlist'),
      action: t('Create'),
      initial: '',
      submit: (name) => {
        const id = app.collections.createPlaylist(name);
        app.nav.open({ view: 'playlist', id });
      },
    };
  }

  function card(playlist: PlaylistSummary): Item {
    return playlistItem(playlist);
  }

  useSourceLibrary(app, 'playlists');
  const sources = $derived(app.sourceLibraries.playlists);
</script>

<section class="page" aria-labelledby="playlists-title">
  <h1 id="playlists-title" class="page-title">{t('Playlists')}</h1>
  {#if playlists.length === 0 && sources.length === 0}
    <EmptyState title={t('No Playlists Yet')} text={t('Make a playlist to keep songs together.')}>
      <button class="pill primary" onclick={create}><Icon name="plus" size={18} />{t('New Playlist…')}</button>
    </EmptyState>
  {:else}
    <div class="page-actions">
      <button class="pill secondary" onclick={create}><Icon name="plus" size={18} />{t('New Playlist…')}</button>
    </div>
    <div class="grid">
      {#each playlists as playlist, index (playlist.id)}
        <ItemCard
          plugin="jellyfesh"
          item={card(playlist)}
          art={playlist.cover}
          index={index < 16 ? index : null}
          entries={() => savedEntries(playlistSongs(app, playlist.id)).entries}
          menu={() => playlistMenu(app, playlist)}
          onactivate={(origin) => app.nav.open({ view: 'playlist', id: playlist.id }, origin)}
        />
      {/each}
    </div>
  {/if}
  {#each sources as source (source.plugin)}
    <SourceLibrarySection {source} />
  {/each}
</section>

<style>
  /* As the library's grids: four large covers to a row on a 1280 px window. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(196px, 1fr));
    gap: var(--jf-space-s3);
    margin-inline: -8px;
  }
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
      gap: var(--jf-space-s1);
    }
  }
</style>
