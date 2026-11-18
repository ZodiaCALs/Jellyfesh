<script lang="ts">
  // The mini-player on phones and narrow windows (under 900 px): a floating Veil capsule with the
  // artwork, two lines of type and one control, the play button (CMP-02), never accent-filled
  // (COL-04). Its progress is a straight 2 px line along the capsule's bottom edge, inset by the
  // capsule's radius so it lies on the straight part (v0.0.4; it replaces the ring). The line
  // redraws only when it moves by a device pixel, never per frame, never while paused or inactive.
  import { untrack } from 'svelte';
  import { useApp } from '../lib/app';
  import { nextDrawUs } from '../lib/drawing/steps';
  import { Ticker } from '../lib/drawing/ticker';
  import { t } from '../lib/i18n/index.svelte';
  import PlayGlyph from './PlayGlyph.svelte';

  let { active = true }: { active?: boolean } = $props();
  const app = useApp();
  const { player, nav, theme } = app;
  const row = $derived(player.current);
  let fraction = $state(0);
  let line: HTMLSpanElement | undefined = $state();
  let width = $state(0);

  const ticker = new Ticker((us) => {
    const duration = player.durationMs;
    if (!duration) {
      fraction = 0;
      return null;
    }
    const durationUs = BigInt(Math.round(duration * 1000));
    fraction = Math.min(1, Number(us) / Number(durationUs));
    return nextDrawUs(us, durationUs, width, devicePixelRatio, false);
  });
  $effect(() => {
    const anchor = player.anchor;
    untrack(() => ticker.set(anchor));
  });
  $effect(() => {
    void player.durationMs;
    void width;
    untrack(() => ticker.tick());
  });
  $effect(() => {
    const shown = active;
    untrack(() => ticker.setActive(shown));
  });
  $effect(() => () => ticker.destroy());

  $effect(() => {
    const element = line;
    if (!element) return;
    const observer = new ResizeObserver((entries) => {
      width = entries[entries.length - 1].contentRect.width;
    });
    observer.observe(element);
    return () => observer.disconnect();
  });

  const status = $derived(
    player.waitingForPlay
      ? t('Press play to resume')
      : player.playing
        ? (row?.artist ?? t('Unknown Artist'))
        : t('Pause, {percent} % played', { percent: Math.round(fraction * 100) }),
  );
</script>

{#if row}
  <div class="mini glass-veil" role="region" aria-label={t('Now Playing')}>
    <button class="open" onclick={() => nav.openNowPlaying()} aria-label={t('Open Now Playing')}>
      <span class="art">
        {#if theme.artwork}<img src={theme.artwork} alt="" referrerpolicy="no-referrer" />{/if}
      </span>
      <span class="text">
        <span class="title">{row.title}</span>
        <span class="second">{player.playing ? (row.artist ?? t('Unknown Artist')) : status}</span>
      </span>
    </button>
    <button
      class="play"
      aria-label={player.playing ? t('Pause') : t('Play')}
      title={t('Play or pause')}
      onclick={() => player.toggle()}
    >
      <PlayGlyph playing={player.playing} size={22} />
    </button>
    <span class="progress" bind:this={line} aria-hidden="true">
      <span class="played" style:translate="{(fraction - 1) * 100}% 0"></span>
    </span>
  </div>
{/if}

<style>
  .mini {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    height: var(--jf-player-mini-height);
    width: min(560px, 100%);
    padding: 0 var(--jf-space-s2) 0 var(--jf-space-s2);
    overflow: hidden;
    border-radius: var(--jf-radius-r-full);
    pointer-events: auto;
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    height: 52px;
    padding: 0 var(--jf-space-s2) 0 4px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .art {
    flex: none;
    width: var(--jf-artwork-size-mini);
    height: var(--jf-artwork-size-mini);
    border-radius: var(--jf-radius-r-full);
    overflow: hidden;
    background: linear-gradient(135deg, var(--jf-aurora-1), var(--jf-aurora-2));
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Grows into Now Playing's artwork (src/lib/motion.ts, viewTransition). */
  :global(:root[data-view-transition='now-playing']) .art {
    view-transition-name: np-artwork;
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title,
  .second {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-size: 14.5px;
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    font-size: 12px;
    color: var(--jf-ink-70);
  }
  .play {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill-hover);
    color: var(--jf-ink-90);
    cursor: pointer;
    transition: scale var(--jf-motion-dur-0) var(--jf-motion-ease-spring);
  }
  .play:active {
    scale: 0.92;
  }
  :global(:root[data-motion='reduced']) .play:active {
    scale: none;
  }
  .play:hover {
    background: var(--jf-fill-press);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  /* Along the bottom edge, between the ends' curves (the capsule's radius is half its height). */
  .progress {
    position: absolute;
    bottom: 0;
    inset-inline: calc(var(--jf-player-mini-height) / 2);
    height: var(--jf-player-mini-progress);
    overflow: hidden;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-ink-10);
  }
  .played {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--jf-ink-70);
  }
</style>
