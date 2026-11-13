<script lang="ts">
  // Style D's transport: shuffle and repeat as accent-soft circles when on; previous, play and
  // next grouped in one glass capsule, play an accent circle (the view's one accent fill, COL-04)
  // whose play and pause glyphs morph into each other (dur-2 Spring), with a soft halo on hover and
  // a 92 % press. Compact (the player bar): 44 px targets, and the capsule a quiet fill rather than
  // glass, since the bar is glass already (MAT-06).
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';
  import PlayGlyph from './PlayGlyph.svelte';

  let { compact = false }: { compact?: boolean } = $props();
  const { player } = useApp();
  const repeat = $derived(player.queue.repeat);
  const repeatLabel = $derived(repeat === 'one' ? t('Repeat one') : repeat === 'all' ? t('Repeat all') : t('Repeat, off'));
</script>

<div class="transport" class:compact>
  <button
    class="toggle"
    class:on={player.queue.shuffle}
    aria-pressed={player.queue.shuffle}
    aria-label={player.queue.shuffle ? t('Shuffle, on') : t('Shuffle, off')}
    title={t('Shuffle')}
    onclick={() => player.setShuffle(!player.queue.shuffle)}
  >
    <Icon name="shuffle" size={compact ? 18 : 20} />
  </button>
  <div class="capsule">
    <button class="skip" aria-label={t('Previous')} title={t('Previous')} onclick={() => player.previous()}>
      <Icon name="previous" size={compact ? 20 : 24} />
    </button>
    <button
      class="play"
      class:busy={player.loading}
      aria-label={player.playing ? t('Pause') : t('Play')}
      title={t('Play or pause')}
      disabled={!player.current}
      onclick={() => player.toggle()}
    >
      <PlayGlyph playing={player.playing} size={compact ? 22 : 28} />
    </button>
    <button class="skip" aria-label={t('Next')} title={t('Next')} onclick={() => player.next()}>
      <Icon name="next" size={compact ? 20 : 24} />
    </button>
  </div>
  <button
    class="toggle"
    class:on={repeat !== 'off'}
    aria-pressed={repeat !== 'off'}
    aria-label={repeatLabel}
    title={t('Repeat')}
    onclick={() => player.cycleRepeat()}
  >
    <Icon name={repeat === 'one' ? 'repeat-one' : 'repeat'} size={compact ? 18 : 20} />
  </button>
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
  }
  .compact {
    justify-content: center;
    gap: var(--jf-space-s2);
  }
  button {
    display: grid;
    place-items: center;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: var(--jf-ink-90);
    background: transparent;
    border-radius: var(--jf-radius-r-full);
    transition:
      scale var(--jf-motion-dur-0) var(--jf-motion-ease-spring),
      background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      box-shadow var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  button:active:not(:disabled) {
    scale: 0.92;
  }
  :global(:root[data-motion='reduced']) button:active:not(:disabled) {
    scale: none;
  }
  .toggle {
    width: 44px;
    height: 44px;
    color: var(--jf-ink-70);
  }
  .toggle:hover {
    background: var(--jf-fill-hover);
  }
  .toggle.on {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  .capsule {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    padding: 6px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-tint-veil);
    backdrop-filter: blur(var(--jf-glass-veil-blur)) saturate(var(--jf-glass-sat));
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  :global([data-effects='reduced']) .capsule {
    background: var(--jf-cloud);
    backdrop-filter: none;
  }
  .compact .capsule,
  :global([data-effects='reduced']) .compact .capsule {
    gap: 2px;
    padding: 0;
    background: var(--jf-fill);
    backdrop-filter: none;
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  .skip {
    width: 52px;
    height: 52px;
  }
  .compact .skip {
    width: 44px;
    height: 44px;
  }
  .skip:hover {
    background: var(--jf-fill-hover);
  }
  .play {
    width: 64px;
    height: 64px;
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    box-shadow: 0 10px 24px -12px var(--jf-shadow-2-1);
  }
  .compact .play {
    width: 44px;
    height: 44px;
    box-shadow: none;
  }
  .play:hover:not(:disabled) {
    box-shadow:
      0 0 0 6px var(--jf-accent-soft),
      0 10px 24px -12px var(--jf-shadow-2-1);
  }
  .compact .play:hover:not(:disabled) {
    box-shadow: 0 0 0 4px var(--jf-accent-soft);
  }
  .play:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .play.busy {
    opacity: 0.85;
  }
</style>
