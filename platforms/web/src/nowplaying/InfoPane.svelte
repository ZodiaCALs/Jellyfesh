<script lang="ts">
  // Now Playing's Info: About This Song and Audio for what plays, as Get Info shows them (docs/ui.md
  // "Details"), with the format actually being played; then the lyrics' kind, and Open in
  // {Service} when the song has a link (API 1.1). A source's details come from its plugin.
  import Icon from '../components/Icon.svelte';
  import { useApp } from '../lib/app';
  import type { InfoRow, ItemDetails, KnownSong, SongRows } from '../lib/core/types';
  import { rowLabel, rowValue } from '../lib/details';
  import { t } from '../lib/i18n/index.svelte';
  import { linkOf, openLink } from '../lib/items';
  import { LIBRARY_ID } from '../lib/state/library.svelte';

  const app = useApp();
  const player = app.player;
  const row = $derived(player.current);
  const found = $derived.by(() => {
    void app.sources.revision;
    return row ? player.sourceItem(row) : null;
  });
  const link = $derived(found ? linkOf(app, found.plugin.id, found.item) : null);
  let details = $state.raw<ItemDetails | null>(null);

  $effect(() => {
    const source = found;
    details = null;
    if (!source?.plugin.declares('Details')) return;
    let live = true;
    source.plugin
      .details(source.item)
      .then((answer) => live && (details = answer))
      .catch(() => {
        // What the page knows stays.
      });
    return () => {
      live = false;
    };
  });

  const rows = $derived.by((): SongRows | null => {
    if (!row) return null;
    const known: KnownSong = { title: row.title, artist: row.artist, album: row.album, duration_ms: player.durationMs ?? row.duration_ms };
    if (found) {
      known.source = found.plugin.name;
      if (found.item.year !== null) known.year = String(found.item.year);
    } else if (row.id.startsWith(LIBRARY_ID)) {
      const track = app.library.track(row.id.slice(LIBRARY_ID.length));
      const folder = track ? app.library.folders.find((f) => f.id === track.folder) : undefined;
      if (track) {
        known.file = folder ? `${folder.name}/${track.path}` : track.path;
        known.size_bytes = track.size;
        known.replaygain = track.replaygain;
        if (track.year !== null) known.year = String(track.year);
      }
    } else {
      known.file = t('Dropped on this page; not saved');
    }
    return JSON.parse(
      app.jf.song_rows(details ? JSON.stringify(details) : undefined, JSON.stringify(known), player.format ? JSON.stringify(player.format) : undefined),
    );
  });
  const lyrics = $derived.by(() => {
    const state = player.lyrics;
    if (state.status !== 'ready') return state.status === 'loading' ? t('Loading…') : t('None found');
    const timing = state.lyrics.doc.timing;
    return timing === 'word' ? t('Word by word') : timing === 'line' ? t('Line by line') : t('Unsynced');
  });
</script>

<div class="info">
  <h2 class="section">{t('About This Song')}</h2>
  {#if rows}
    {@render list(rows.about)}
    {#if rows.audio.length}
      <h3 class="section audio">{t('Audio')}</h3>
      {@render list(rows.audio)}
    {/if}
    <dl class="lyrics">
      <dt>{t('Lyrics')}</dt>
      <dd>{lyrics}</dd>
    </dl>
    {#if link}
      {@const target = link}
      <button class="open-in" onclick={() => openLink(target.url)}>
        <Icon name="external" size={18} />{t('Open in {service}', { service: target.service })}
      </button>
    {/if}
  {:else}
    <p class="muted">{t('Nothing Playing')}</p>
  {/if}
</div>

{#snippet list(items: InfoRow[])}
  <dl>
    {#each items as item (item.key)}
      <dt>{rowLabel(item)}</dt>
      <dd>{rowValue(item)}</dd>
    {/each}
  </dl>
{/snippet}

<style>
  .info {
    padding: 0 var(--jf-space-s2);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--jf-space-s3) var(--jf-space-s5);
    margin: 0;
  }
  dt {
    color: var(--jf-ink-70);
    font-weight: 600;
  }
  dd {
    margin: 0;
    color: var(--jf-ink-90);
    overflow-wrap: anywhere;
  }
  .muted {
    color: var(--jf-ink-70);
  }
  .audio {
    margin-top: var(--jf-space-s5);
  }
  .lyrics {
    margin-top: var(--jf-space-s3);
  }
  .open-in {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    margin-top: var(--jf-space-s6);
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-tint-veil);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
    cursor: pointer;
  }
  .open-in:hover {
    background: var(--jf-cloud);
  }
  .open-in:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
