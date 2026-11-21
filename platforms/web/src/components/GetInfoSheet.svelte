<script lang="ts">
  // Get Info (docs/ui.md "Details"; ADR-0022 §3). For a song: About This Song (Title, Artist,
  // Album, Version, Year, Genre, Composer, Lyricist, Label, Length) and Audio (Quality, Format,
  // Bitrate, Size, ReplayGain, File or Source), rows without a value left out. For an album or an
  // artist: its picture, its notes or biography ("More" expands them) and its facts. A source's
  // details come from its plugin (API 1.2); what the page knows shows until then, and without
  // them. jf-web's details rules pick the rows; src/lib/details.ts words them.
  import { useApp } from '../lib/app';
  import type { InfoRow, Item, ItemDetails, SongRows } from '../lib/core/types';
  import { formatOf, knownOf, rowLabel, rowValue } from '../lib/details';
  import { t } from '../lib/i18n/index.svelte';
  import { LIBRARY } from '../lib/state/library.svelte';
  import Art from './Art.svelte';
  import Sheet from './Sheet.svelte';

  let { plugin, item, onclose }: { plugin: string; item: Item; onclose: () => void } = $props();
  const app = useApp();
  let details = $state.raw<ItemDetails | null>(null);
  let expanded = $state(false);

  $effect(() => {
    const source = plugin === LIBRARY ? undefined : app.sources.get(plugin);
    if (!source?.declares('Details')) return;
    let live = true;
    source
      .details(item)
      .then((answer) => live && (details = answer))
      .catch(() => {
        // What the page knows stays.
      });
    return () => {
      live = false;
    };
  });

  const song = $derived(item.kind === 'Track');
  const rows = $derived.by((): SongRows | null => {
    if (!song) return null;
    const format = formatOf(app, plugin, item);
    return JSON.parse(
      app.jf.song_rows(
        details ? JSON.stringify(details) : undefined,
        JSON.stringify(knownOf(app, plugin, item)),
        format ? JSON.stringify(format) : undefined,
      ),
    );
  });
  const facts = $derived(!song && details ? (JSON.parse(app.jf.item_rows(JSON.stringify(details))) as InfoRow[]) : []);
  const pictured = $derived(details?.image ? { ...item, artwork: details.image } : item);
</script>

<Sheet title={song ? t('About This Song') : item.title} {onclose}>
  <div class="info">
    {#if rows}
      {@render list(rows.about)}
      {#if rows.audio.length}
        <h3 class="section">{t('Audio')}</h3>
        {@render list(rows.audio)}
      {/if}
    {:else}
      <div class="head">
        <span class="picture" class:round={item.kind === 'Artist'}><Art {plugin} item={pictured} size={160} radius="grid" /></span>
        {#if item.artist && item.kind !== 'Artist'}<p class="by">{item.artist}</p>{/if}
      </div>
      {#if details?.description}
        <p class="notes" class:clamped={!expanded}>{details.description}</p>
        <button class="more" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
          {expanded ? t('Less') : t('More')}
        </button>
      {/if}
      {#if facts.length}{@render list(facts)}{/if}
    {/if}
  </div>
</Sheet>

{#snippet list(items: InfoRow[])}
  <dl>
    {#each items as row (row.key)}
      <dt>{rowLabel(row)}</dt>
      <dd>{rowValue(row)}</dd>
    {/each}
  </dl>
{/snippet}

<style>
  .info {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s3);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--jf-space-s2) var(--jf-space-s5);
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
  .section {
    margin: var(--jf-space-s3) 0 0;
  }
  .head {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
  }
  .picture {
    width: 160px;
    height: 160px;
    overflow: hidden;
    border-radius: var(--jf-radius-r-lg);
  }
  .picture.round {
    border-radius: 50%;
  }
  .by {
    margin: 0;
    color: var(--jf-ink-70);
  }
  .notes {
    margin: 0;
    white-space: pre-line;
    color: var(--jf-ink-90);
  }
  .notes.clamped {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .more {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--jf-accent-text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
</style>
