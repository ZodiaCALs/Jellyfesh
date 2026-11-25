<script lang="ts">
  // The player bar on windows 900 px wide and wider (v0.0.4, after Media Player and Apple Music;
  // Luminous v0.0.6): a floating Veil capsule, player.bar-height (80 px) high with r-2xl corners,
  // over every page, Settings included. In the order every app has (docs/ui.md, "The player bar
  // and the mini player"): the artwork (56 px) and two lines, which open Now Playing; the
  // transport above the seek bar and its times; the quality badge and its menu; Lyrics, Queue, the
  // device button (only while a remote source plays) and the volume; the mini player button (where
  // the browser has picture-in-picture windows) and More. A narrower bar drops the volume line,
  // then Lyrics and Queue (both stay in Now Playing), then the mute button. It bends CMP-02 (one
  // control) on purpose, for desktop use; play stays its only accent fill. The seek bar's clock
  // sleeps while inactive. With Show Lyrics Bar on and timed lyrics, the line being sung fills word
  // by word under the title, instead of the artist, with its translation when Show Translations is
  // on (ADR-0022 §6). While Desktop Lyrics waits for a click (the page opened with it on), Show
  // Desktop Lyrics opens it.
  import { untrack } from 'svelte';
  import { useApp } from '../lib/app';
  import { nowPlayingMenu } from '../lib/items';
  import { crossfade } from '../lib/motion';
  import { t } from '../lib/i18n/index.svelte';
  import { LyricsBarClock, showsLyricsBar } from '../lib/state/lyricsBar.svelte';
  import { toast } from '../lib/state/toast.svelte';
  import DeviceButton from './DeviceButton.svelte';
  import Icon from './Icon.svelte';
  import LyricsBarLine from './LyricsBarLine.svelte';
  import Menu from './Menu.svelte';
  import QualityMenu from './QualityMenu.svelte';
  import SeekBar from './SeekBar.svelte';
  import Transport from './Transport.svelte';
  import Volume from './Volume.svelte';

  let { active = true }: { active?: boolean } = $props();
  const app = useApp();
  const { player, nav, theme, settings, desktopLyrics, miniPlayer } = app;
  const row = $derived(player.current);
  const second = $derived(
    player.waitingForPlay
      ? t('Press play to resume')
      : [row?.artist ?? t('Unknown Artist'), row?.album].filter(Boolean).join(' · '),
  );

  const lyrics = $derived(player.lyrics.status === 'ready' ? player.lyrics.lyrics : null);
  const showsBar = $derived(showsLyricsBar(settings.value.show_lyrics_bar, lyrics));
  const clock = new LyricsBarClock();
  $effect(() => {
    const timeline = showsBar && lyrics ? lyrics.timeline : null;
    const anchor = player.anchor;
    untrack(() => clock.set(timeline, anchor));
  });
  $effect(() => clock.setActive(active));
  $effect(() => () => clock.destroy());
  const barLine = $derived(
    showsBar && lyrics && !player.waitingForPlay && clock.bar.line !== null ? (lyrics.doc.lines[clock.bar.line] ?? null) : null,
  );
  const translation = $derived.by(() => {
    if (!barLine || !lyrics || !settings.value.show_translation) return null;
    return lyrics.doc.tracks.find((track) => track.kind === 'translation')?.lines[String(barLine.id)]?.text ?? null;
  });
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
          {#if barLine && lyrics}
            <span class="lyric-line" lang={lyrics.doc.language ?? undefined}>
              <LyricsBarLine line={barLine} {clock} wordTimed={lyrics.doc.timing === 'word'} reduced={theme.reducedMotion} />
            </span>
            {#if translation}<span class="translation">{translation}</span>{/if}
          {:else}
            <span class="second">{second}</span>
          {/if}
        </span>
      </button>
      <div class="middle">
        <Transport compact />
        <SeekBar compact anchor={player.anchor} durationMs={player.durationMs} onseek={(ms) => player.seek(ms)} {active} />
      </div>
      <div class="end">
        <QualityMenu />
        {#if desktopLyrics.waiting}
          <button class="tool" aria-label={t('Show Desktop Lyrics')} title={t('Show Desktop Lyrics')} onclick={() => void desktopLyrics.show()}>
            <Icon name="external" />
          </button>
        {/if}
        <button class="tool pane" aria-label={t('Lyrics')} title={t('Lyrics')} onclick={() => nav.openNowPlaying('lyrics')}>
          <Icon name="lyrics" />
        </button>
        <button class="tool pane" aria-label={t('Queue')} title={t('Queue')} onclick={() => nav.openNowPlaying('queue')}>
          <Icon name="queue" />
        </button>
        {#if player.remote}<DeviceButton remote={player.remote} onchoose={() => {}} />{/if}
        <span class="volume"><Volume compact /></span>
        {#if miniPlayer.supported}
          <button
            class="tool"
            aria-label={miniPlayer.shown ? t('Close Mini Player') : t('Open Mini Player')}
            aria-pressed={miniPlayer.shown}
            title={miniPlayer.shown ? t('Close Mini Player') : t('Open Mini Player')}
            onclick={() => void miniPlayer.toggle()}
          >
            <Icon name="mini-player" />
          </button>
        {/if}
        <Menu items={() => nowPlayingMenu(app, row, toast)} label={t('More')} />
      </div>
    </div>
  </div>
{/if}

<style>
  .bar {
    width: 100%;
    max-width: 1280px;
    pointer-events: auto;
    container: bar / inline-size;
  }
  .inner {
    display: grid;
    grid-template-columns: minmax(112px, 1fr) minmax(232px, 340px) auto;
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
    font-size: calc(15px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .second {
    font-size: calc(12.5px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .lyric-line {
    min-width: 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
  }
  .translation {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: calc(11px * var(--jf-text-scale, 1));
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
    flex: none;
    width: 132px;
    min-width: 44px;
  }
  .tool[aria-pressed='true'] {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  /* A narrower bar keeps the mute button and drops the volume line (Ctrl+↑/↓ still change the
     volume), then Lyrics and Queue (Now Playing has both, and Ctrl+L and Ctrl+U), then the mute
     button; the title keeps room to be read. A 1280 px window's bar (968 px beside the sidebar)
     has everything. */
  @container bar (max-width: 899px) {
    .volume {
      width: auto;
    }
    .volume :global(.line) {
      display: none;
    }
  }
  @container bar (max-width: 759px) {
    .pane {
      display: none;
    }
  }
  @container bar (max-width: 639px) {
    .volume {
      display: none;
    }
  }
</style>
