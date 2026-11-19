<script lang="ts">
  // The player bar on windows 900 px wide and wider (v0.0.4, after Media Player and Apple Music;
  // Luminous v0.0.6): a floating Veil capsule, player.bar-height (80 px) high with r-2xl corners.
  // The artwork (56 px) and two lines open Now Playing; in the middle the transport sits above the
  // seek bar and its times; Lyrics, Queue and the volume are at the end. It bends CMP-02 (one
  // control) on purpose, for desktop use; play stays its only accent fill. The seek bar's clock
  // sleeps while inactive.
  import { useApp } from '../lib/app';
  import { crossfade } from '../lib/motion';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';
  import SeekBar from './SeekBar.svelte';
  import Transport from './Transport.svelte';
  import Volume from './Volume.svelte';

  let { active = true }: { active?: boolean } = $props();
  const { player, nav, theme } = useApp();
  const row = $derived(player.current);
  const second = $derived(
    player.waitingForPlay
      ? t('Press play to resume')
      : [row?.artist ?? t('Unknown Artist'), row?.album].filter(Boolean).join(' · '),
  );
</script>

{#if row}
  <div class="bar" role="region" aria-label={t('Now Playing')}>
    <div class="inner glass-veil">
      <button class="open" onclick={() => nav.openNowPlaying()} aria-label={t('Open Now Playing')}>
        <span class="art">
          {#each theme.artwork ? [theme.artwork] : [] as url (url)}
            <img src={url} alt="" referrerpolicy="no-referrer" transition:crossfade />
          {/each}
        </span>
        <span class="text">
          <span class="title">{row.title}</span>
          <span class="second">{second}</span>
        </span>
      </button>
      <div class="middle">
        <Transport compact />
        <SeekBar compact anchor={player.anchor} durationMs={player.durationMs} onseek={(ms) => player.seek(ms)} {active} />
      </div>
      <div class="end">
        <button class="tool" aria-label={t('Lyrics')} title={t('Lyrics')} onclick={() => nav.openNowPlaying('lyrics')}>
          <Icon name="lyrics" />
        </button>
        <button class="tool" aria-label={t('Queue')} title={t('Queue')} onclick={() => nav.openNowPlaying('queue')}>
          <Icon name="queue" />
        </button>
        <span class="volume"><Volume compact /></span>
      </div>
    </div>
  </div>
{/if}

<style>
  .bar {
    width: 100%;
    max-width: 1280px;
    pointer-events: auto;
  }
  .inner {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 1.4fr) minmax(0, 1fr);
    align-items: center;
    gap: var(--jf-space-s4);
    height: var(--jf-player-bar-height);
    padding: 0 var(--jf-space-s4) 0 var(--jf-space-s2);
    border-radius: var(--jf-radius-r-2xl);
  }
  .open {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    height: 68px;
    padding: 0 var(--jf-space-s2) 0 6px;
    border: 0;
    border-radius: var(--jf-radius-r-xl);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .open:hover {
    background: var(--jf-fill-hover);
  }
  .art {
    flex: none;
    width: 56px;
    height: 56px;
    overflow: hidden;
    border-radius: var(--jf-radius-r-lg);
    background: linear-gradient(135deg, var(--jf-aurora-1), var(--jf-aurora-2));
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-1);
  }
  .art {
    position: relative;
  }
  /* A new song's artwork fades in over the last (dur-3 Fluid). */
  .art img {
    position: absolute;
    inset: 0;
    display: block;
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
    gap: 2px;
  }
  .title,
  .second {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    font-size: 12.5px;
    color: var(--jf-ink-70);
  }
  .middle {
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
  }
  .end {
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
  }
  .tool {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .tool:hover {
    background: var(--jf-fill-hover);
    color: var(--jf-ink-90);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .volume {
    flex: 0 1 160px;
    min-width: 44px;
  }
  /* A narrow bar (under about 760 px, beside the 232 px sidebar) keeps the mute button and drops
     the volume line; Ctrl+↑/↓ still change the volume. */
  @media (max-width: 1040px) {
    .volume {
      flex: none;
    }
    .volume :global(.line) {
      display: none;
    }
  }
</style>
