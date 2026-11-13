<script lang="ts">
  // About This Song: what plays, from where, in what format, with what lyrics.
  import { useApp } from '../lib/app';
  import { formatTime, t } from '../lib/i18n/index.svelte';

  const app = useApp();
  const player = app.player;
  const row = $derived(player.current);
  const source = $derived(row?.plugin ? (app.sources.get(row.plugin)?.name ?? row.plugin) : null);
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
    color: var(--jf-ink-50);
    font-weight: 600;
  }
  dd {
    margin: 0;
    color: var(--jf-ink-90);
    overflow-wrap: anywhere;
  }
  .muted {
    color: var(--jf-ink-50);
  }
</style>
