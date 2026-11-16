<script lang="ts">
  // About This Song: what plays, from where, in what format, with what lyrics; then Open in
  // {Service} when the song has a link (API 1.1, docs/ui.md).
  import Icon from '../components/Icon.svelte';
  import { useApp } from '../lib/app';
  import { formatTime, t } from '../lib/i18n/index.svelte';
  import { linkOf, openLink } from '../lib/items';

  const app = useApp();
  const player = app.player;
  const row = $derived(player.current);
  const source = $derived(row?.plugin ? (app.sources.get(row.plugin)?.name ?? row.plugin) : null);
  const link = $derived.by(() => {
    void app.sources.revision;
    const found = row ? player.sourceItem(row) : null;
    return found ? linkOf(app, found.plugin.id, found.item) : null;
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
  {#if row}
    <dl>
      <dt>{t('Title')}</dt>
      <dd>{row.title}</dd>
      <dt>{t('Artist')}</dt>
      <dd>{row.artist ?? t('Unknown Artist')}</dd>
      <dt>{t('Album')}</dt>
      <dd>{row.album ?? t('Unknown Album')}</dd>
      <dt>{t('Length')}</dt>
      <dd>{formatTime(player.durationMs ?? row.duration_ms)}</dd>
      {#if player.format}
        <dt>{t('Format')}</dt>
        <dd>{player.format}</dd>
      {/if}
      <dt>{row.kind === 'file' ? t('File') : t('Source')}</dt>
      <dd>{row.kind === 'file' ? t('Dropped on this page; not saved') : source}</dd>
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
