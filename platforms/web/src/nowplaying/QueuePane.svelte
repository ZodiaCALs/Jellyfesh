<script lang="ts">
  // Up Next: the rest of the queue in playback order, lazily; play, move or remove an entry.
  import Art from '../components/Art.svelte';
  import Menu from '../components/Menu.svelte';
  import Sheet from '../components/Sheet.svelte';
  import VirtualList from '../components/VirtualList.svelte';
  import { useApp } from '../lib/app';
  import type { Row } from '../lib/core/types';
  import { formatTime, t } from '../lib/i18n/index.svelte';

  const app = useApp();
  const player = app.player;
  const offset = $derived((player.queue.position ?? -1) + 1);
  const count = $derived(Math.max(0, player.queue.length - offset));
  let confirming = $state(false);

  function art(row: Row) {
    if (row.kind === 'file') return { plugin: null, item: null };
    const found = player.sourceItem(row);
    return { plugin: found?.plugin.id ?? null, item: found?.item ?? null };
  }
</script>

<div class="queue">
  <header>
    <h2 class="section">{t('Up Next')}</h2>
    {#if player.queue.length > 0}
      <button class="clear" onclick={() => (confirming = true)}>{t('Clear Queue')}</button>
    {/if}
  </header>
  {#if count === 0}
    <div class="empty">
      <p class="title">{t('Nothing Up Next')}</p>
      <p>{t('Use Play Next or Play Last on any song, album or playlist.')}</p>
    </div>
  {:else}
    <div class="list">
      <VirtualList
        {count}
        rowHeight={56}
        revision={player.queue.revision}
        rows={(start, end) => player.window(offset + start, end - start)}
        key={(row) => row.uid}
        label={t('Up Next')}
      >
        {#snippet row(entry: Row, index: number)}
          {@const shown = art(entry)}
          <div class="entry">
            <button class="main" onclick={() => player.jumpTo(entry.uid)} aria-label={t('Play {title}', { title: entry.title })}>
              <Art plugin={shown.plugin} item={shown.item} src={entry.artwork_url} size={40} />
              <span class="text">
                <span class="name">{entry.title}</span>
                <span class="second">{entry.artist ?? t('Unknown Artist')}</span>
              </span>
              <span class="length">{entry.duration_ms ? formatTime(entry.duration_ms) : ''}</span>
            </button>
            <Menu
              label={t('More for {title}', { title: entry.title })}
              items={[
                { label: t('Play'), action: () => player.jumpTo(entry.uid) },
                { label: t('Move Up'), disabled: index === 0, action: () => player.move(entry.uid, offset + index - 1) },
                { label: t('Move Down'), disabled: index === count - 1, action: () => player.move(entry.uid, offset + index + 1) },
                { label: t('Remove'), action: () => player.remove(entry.uid) },
              ]}
            />
          </div>
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

{#if confirming}
  <Sheet title={t('Clear the Queue?')} onclose={() => (confirming = false)}>
    <p class="confirm">{t('Playback stops and every song is removed from the queue.')}</p>
    <div class="actions">
      <button class="secondary" onclick={() => (confirming = false)}>{t('Cancel')}</button>
      <button
        class="danger"
        onclick={() => {
          confirming = false;
          player.clear();
        }}>{t('Clear Queue')}</button
      >
    </div>
  </Sheet>
{/if}

<style>
  .queue {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    padding: 0 var(--jf-space-s2);
  }
  .clear {
    min-height: 36px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-accent-text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .clear:hover {
    background: var(--jf-fill-hover);
  }
  .list {
    flex: 1;
    min-height: 0;
  }
  .entry {
    display: flex;
    align-items: center;
    height: 100%;
    border-radius: var(--jf-radius-r-md);
  }
  .entry:hover {
    background: var(--jf-fill);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    height: 100%;
    padding: 0 var(--jf-space-s2);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .main:focus-visible,
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name,
  .second {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-size: 14px;
    font-weight: 500;
    color: var(--jf-ink-90);
  }
  .second {
    font-size: 12px;
    color: var(--jf-ink-50);
  }
  .length {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-50);
  }
  .empty {
    padding: var(--jf-space-s6) var(--jf-space-s2);
    color: var(--jf-ink-70);
  }
  .empty .title {
    font-weight: 700;
    color: var(--jf-ink-90);
  }
  .empty p {
    margin: 0 0 var(--jf-space-s1);
  }
  .confirm {
    margin: 0 0 var(--jf-space-s5);
    color: var(--jf-ink-70);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--jf-space-s2);
  }
  .actions button {
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .danger {
    background: var(--jf-danger);
    color: var(--jf-on-accent);
  }
</style>
