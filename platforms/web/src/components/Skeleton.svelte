<script lang="ts">
  // Placeholders the shape of what is loading (docs/ui.md: "While loading: placeholders the
  // shape of the shelves"): rows like a list's, cards like a grid's, or a row of shelf tiles. A
  // soft light sweeps across them (a translated gradient, so the compositor does it); with reduced
  // motion they stand still. Screen readers hear "Loading…" once.
  import { t } from '../lib/i18n/index.svelte';

  let { kind = 'rows', count = 6 }: { kind?: 'rows' | 'cards' | 'tiles'; count?: number } = $props();
</script>

<div class="skeleton {kind}" role="status" aria-busy="true">
  <span class="sr-only">{t('Loading…')}</span>
  {#each { length: count } as _, index (index)}
    {#if kind === 'rows'}
      <div class="sk-row" aria-hidden="true">
        <span class="block art"></span>
        <span class="lines"><span class="block line"></span><span class="block line short"></span></span>
      </div>
    {:else if kind === 'cards'}
      <div class="sk-card" aria-hidden="true">
        <span class="block cover"></span>
        <span class="block line"></span>
        <span class="block line short"></span>
      </div>
    {:else}
      <div class="block tile" aria-hidden="true"></div>
    {/if}
  {/each}
</div>

<style>
  .skeleton {
    position: relative;
    overflow: hidden;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    border-radius: var(--jf-radius-r-xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 168px), 1fr));
    gap: var(--jf-space-s4);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--jf-space-s2);
  }
  .block {
    display: block;
    background: var(--jf-fill);
  }
  .sk-row {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    height: var(--jf-player-row-height);
    padding: 0 var(--jf-space-s2);
  }
  .art {
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: var(--jf-artwork-radius-list);
  }
  .lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .line {
    height: 12px;
    width: min(60%, 320px);
    border-radius: var(--jf-radius-r-full);
  }
  .line.short {
    width: min(36%, 200px);
  }
  .sk-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
  }
  .cover {
    aspect-ratio: 1;
    border-radius: var(--jf-artwork-radius-grid);
  }
  .sk-card .line {
    width: 70%;
  }
  .sk-card .line.short {
    width: 45%;
  }
  .tile {
    height: 56px;
    border-radius: var(--jf-radius-r-lg);
  }
  /* The sweep: a band of light crossing every block at once, 1.6 s, then a pause. */
  .skeleton::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, var(--jf-fill-hover) 50%, transparent 70%);
    opacity: 0.9;
    translate: -100% 0;
    animation: shimmer 2.2s var(--jf-motion-ease-fluid) infinite;
    pointer-events: none;
  }
  @keyframes shimmer {
    70%,
    to {
      translate: 100% 0;
    }
  }
  :global(:root[data-motion='reduced']) .skeleton::after {
    animation: none;
    opacity: 0;
  }
</style>
