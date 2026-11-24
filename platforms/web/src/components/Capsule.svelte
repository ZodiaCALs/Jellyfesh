<script lang="ts">
  // The capsule on phones and narrow windows (under 900 px; docs/ui.md calls it the capsule, the
  // look the mini-player style): a floating Veil capsule with the artwork, the title (and the
  // artist under it), play and next (docs/ui.md, "The player bar and the mini player"), never
  // accent-filled (COL-04). Its progress is a straight 2 px line along the capsule's bottom edge,
  // inset by the capsule's radius so it lies on the straight part (v0.0.4; it replaces the ring).
  // The line redraws only when it moves by a device pixel, never per frame, never while paused or
  // inactive.
  import { untrack } from 'svelte';
  import { useApp } from '../lib/app';
  import { crossfade } from '../lib/motion';
  import { nextDrawUs } from '../lib/drawing/steps';
  import { Ticker } from '../lib/drawing/ticker';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';
  import PlayGlyph from './PlayGlyph.svelte';

  // Floating (the look's mini-player style, ADR-0020): a smaller capsule at the end corner.
  let { active = true, floating = false }: { active?: boolean; floating?: boolean } = $props();
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
  <div class="mini glass-veil" class:floating role="region" aria-label={t('Now Playing')}>
    <button class="open" onclick={() => nav.openNowPlaying()} aria-label={t('Open Now Playing')}>
      <span class="art">
        {#each theme.artwork ? [theme.artwork] : [] as url (url)}
          <img src={url} alt="" referrerpolicy="no-referrer" transition:crossfade />
        {/each}
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
    <button class="next" aria-label={t('Next')} title={t('Next')} onclick={() => player.next()}>
      <Icon name="next" size={20} />
    </button>
    <span class="progress" bind:this={line} aria-hidden="true">
      <span class="played" style:translate="{(fraction - 1) * 100}% 0"></span>
    </span>
  </div>
{/if}

<style>
  .mini.floating {
    align-self: flex-end;
    width: min(360px, 100%);
    box-shadow: var(--jf-elevation-shadow-3);
  }
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
  .art {
    position: relative;
  }
  /* A new song's artwork fades in over the last (dur-3 Fluid). */
  .art img {
    position: absolute;
    inset: 0;
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
    font-size: calc(14.5px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    font-size: calc(12px * var(--jf-text-scale, 1));
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
  .next {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    height: 44px;
    margin-inline-end: 2px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-90);
    cursor: pointer;
  }
  .next:hover {
    background: var(--jf-fill-hover);
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
