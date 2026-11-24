<script lang="ts">
  // Add to Playlist… (docs/ui.md): New Playlist… (which asks for the name first), then the
  // Jellyfesh playlists by name. Choosing one adds the songs at its end.
  import { useApp } from '../lib/app';
  import type { Saved } from '../lib/core/types';
  import { t, tn } from '../lib/i18n/index.svelte';
  import { refusalText } from '../lib/state/collections.svelte';
  import { type Picking, sheets } from '../lib/state/sheets.svelte';
  import { toast } from '../lib/state/toast.svelte';
  import Icon from './Icon.svelte';
  import Sheet from './Sheet.svelte';

  let { picking, onclose }: { picking: Picking; onclose: () => void } = $props();
  const app = useApp();
  const playlists = $derived((void app.collections.revision, app.collections.playlists()));
  let busy = $state(false);

  async function songs(): Promise<Saved[] | null> {
    busy = true;
    try {
      return await picking.songs();
    } catch (error) {
      toast(error instanceof Error ? error.message : String(error));
      return null;
    } finally {
      busy = false;
    }
  }

  async function addTo(id: number, name: string) {
    const list = await songs();
    if (!list) return;
    try {
      app.collections.addToPlaylist(id, list);
      toast(t('Added to “{name}”', { name }));
    } catch (error) {
      toast(refusalText(error));
    }
    onclose();
  }

  function create() {
    const pick = picking;
    onclose();
    sheets.naming = {
      title: t('New Playlist'),
      action: t('Create'),
      initial: '',
      submit: (name) => {
        const id = app.collections.createPlaylist(name);
        const named = app.collections.playlist(id)?.name ?? name;
        void pick.songs().then((list) => {
          app.collections.addToPlaylist(id, list);
          toast(t('Added to “{name}”', { name: named }));
        });
      },
    };
  }
</script>

<Sheet title={t('Add to Playlist')} {onclose}>
  <ul class="list">
    <li>
      <button class="new" onclick={create} disabled={busy}><Icon name="plus" />{t('New Playlist…')}</button>
    </li>
    {#each playlists as playlist (playlist.id)}
      <li>
        <button onclick={() => addTo(playlist.id, playlist.name)} disabled={busy}>
          <Icon name="playlist" />
          <span class="name">{playlist.name}</span>
          <span class="count">{tn('{n} song', '{n} songs', playlist.entry_count)}</span>
        </button>
      </li>
    {/each}
  </ul>
</Sheet>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    width: 100%;
    min-height: 48px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: transparent;
    color: var(--jf-ink-90);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--jf-fill-hover);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .new {
    color: var(--jf-accent-text);
    font-weight: 600;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--jf-ink-70);
    font-size: calc(13px * var(--jf-text-scale, 1));
  }
</style>
