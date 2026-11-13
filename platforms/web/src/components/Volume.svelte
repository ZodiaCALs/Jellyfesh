<script lang="ts">
  // The volume: the seek bar's straight line between two speaker glyphs, with the same hover thumb;
  // the quiet glyph mutes. Compact (the player bar): the mute button and the line.
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';
  import LineSlider from './LineSlider.svelte';

  let { compact = false }: { compact?: boolean } = $props();
  const { player } = useApp();
  const level = $derived(Math.round((player.muted ? 0 : player.volume) * 100));
</script>

<div class="volume" class:compact>
  <button
    aria-label={player.muted ? t('Unmute') : t('Mute')}
    aria-pressed={player.muted}
    title={player.muted ? t('Unmute') : t('Mute')}
    onclick={() => player.setMuted(!player.muted)}
  >
    <Icon name={player.muted ? 'mute' : 'volume-low'} />
  </button>
  <span class="line">
    <LineSlider
      value={level}
      min={0}
      max={100}
      step={1}
      label={t('Volume')}
      valuetext="{level} %"
      oninput={(value) => player.setVolume(value / 100)}
    />
  </span>
  {#if !compact}<span class="glyph" aria-hidden="true"><Icon name="volume-high" /></span>{/if}
</div>

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-width: 0;
    color: var(--jf-ink-70);
  }
  .line {
    flex: 1;
    min-width: 0;
    display: flex;
  }
  button {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  button:hover {
    background: var(--jf-fill-hover);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .glyph {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
  }
  .compact {
    gap: 2px;
  }
</style>
