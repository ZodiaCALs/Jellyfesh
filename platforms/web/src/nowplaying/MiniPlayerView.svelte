<script lang="ts">
  // The mini player's window (ADR-0026 §4; drawn into a Document Picture-in-Picture window by
  // MiniPlayerWindow): the cover, the title and the artist, previous, play and next, the line being
  // sung (word by word, with timed lyrics; Settings › Appearance › Mini Player › Show Lyrics Line),
  // and the progress as a straight line along the bottom.
  // Nothing runs per frame: the line moves by device pixels and the lyric line at the next word,
  // and both sleep while the window is hidden. Its own button closes it.
  import { untrack } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import LyricsBarLine from '../components/LyricsBarLine.svelte';
  import PlayGlyph from '../components/PlayGlyph.svelte';
  import type { App } from '../lib/app';
  import { nextDrawUs } from '../lib/drawing/steps';
  import { Ticker } from '../lib/drawing/ticker';
  import { t } from '../lib/i18n/index.svelte';
  import { LyricsBarClock } from '../lib/state/lyricsBar.svelte';

  let { app, win, onclose }: { app: App; win: Window; onclose: () => void } = $props();
  // The window is made for one app and closed with it.
  // svelte-ignore state_referenced_locally
  const { player, theme, settings } = app;
  const hidden = () => win.document.visibilityState === 'hidden';
  const row = $derived(player.current);

  // The progress line.
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
    return nextDrawUs(us, durationUs, width, win.devicePixelRatio, false);
  }, hidden);
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
    const element = line;
    if (!element) return;
    const observer = new ResizeObserver((entries) => {
      width = entries[entries.length - 1].contentRect.width;
    });
    observer.observe(element);
    return () => observer.disconnect();
  });

  // The line being sung, with timed lyrics, unless Show Lyrics Line is off.
  const clock = new LyricsBarClock(hidden);
  const lyrics = $derived(
    settings.value.mini_player.show_lyrics_line && player.lyrics.status === 'ready' ? player.lyrics.lyrics : null,
  );
  const timed = $derived(lyrics !== null && lyrics.synced && lyrics.doc.timing !== 'unsynced');
  $effect(() => {
    const timeline = timed && lyrics ? lyrics.timeline : null;
    const anchor = player.anchor;
    untrack(() => clock.set(timeline, anchor));
  });
  const sung = $derived(timed && lyrics && clock.bar.line !== null ? (lyrics.doc.lines[clock.bar.line] ?? null) : null);

  $effect(() => {
    const wake = () => {
      ticker.tick();
      clock.wake();
    };
    win.document.addEventListener('visibilitychange', wake);
    return () => {
      win.document.removeEventListener('visibilitychange', wake);
      ticker.destroy();
      clock.destroy();
    };
  });
  $effect(() => {
    win.document.title = row ? row.title : t('Mini Player');
  });
</script>

<div class="mini" role="region" aria-label={t('Mini Player')}>
  <span class="cover">
    {#if theme.artwork}
      <img src={theme.artwork} alt="" referrerpolicy="no-referrer" />
    {:else}
      <span class="placeholder" aria-hidden="true"><Icon name="note" size={36} /></span>
    {/if}
  </span>
  <div class="side">
    <div class="titles">
      <p class="title">{row?.title ?? t('Nothing Playing')}</p>
      <p class="artist">{row ? (row.artist ?? t('Unknown Artist')) : t('Pick a song, an album or a playlist.')}</p>
    </div>
    <div class="controls">
      <button class="skip" aria-label={t('Previous')} title={t('Previous')} disabled={!row} onclick={() => player.previous()}>
        <Icon name="previous" size={20} />
      </button>
      <button
        class="play"
        aria-label={player.playing ? t('Pause') : t('Play')}
        title={t('Play or pause')}
        disabled={!row}
        onclick={() => player.toggle()}
      >
        <PlayGlyph playing={player.playing} size={22} />
      </button>
      <button class="skip" aria-label={t('Next')} title={t('Next')} disabled={!row} onclick={() => player.next()}>
        <Icon name="next" size={20} />
      </button>
    </div>
    {#if sung && lyrics}
      <p class="lyric" lang={lyrics.doc.language ?? undefined}>
        <LyricsBarLine line={sung} {clock} wordTimed={lyrics.doc.timing === 'word'} reduced={theme.reducedMotion} />
      </p>
    {/if}
  </div>
  <button class="close" aria-label={t('Close Mini Player')} title={t('Close Mini Player')} onclick={onclose}>
    <Icon name="close" size={16} />
  </button>
  <span class="progress" bind:this={line} aria-hidden="true">
    <span class="played" style:translate="{(fraction - 1) * 100}% 0"></span>
  </span>
</div>

<style>
  .mini {
    position: relative;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 100%;
    padding: 12px 14px 14px 12px;
    overflow: hidden;
    background: var(--jf-cloud);
  }
  .cover {
    position: relative;
    flex: none;
    width: 104px;
    height: 104px;
    max-height: 100%;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--jf-radius-r-lg, 12px);
    background: linear-gradient(135deg, var(--jf-aurora-1), var(--jf-aurora-2));
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-1);
  }
  .cover img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--jf-ink-30);
  }
  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .titles {
    min-width: 0;
    padding-inline-end: 28px;
  }
  p {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-size: calc(15px * var(--jf-text-scale, 1));
    font-weight: 700;
    color: var(--jf-ink-90);
  }
  .artist {
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  button {
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--jf-radius-r-full, 999px);
    background: transparent;
    color: var(--jf-ink-90);
    cursor: pointer;
  }
  button:disabled {
    color: var(--jf-ink-30);
    cursor: default;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .skip {
    width: 40px;
    height: 40px;
  }
  .skip:hover:not(:disabled) {
    background: var(--jf-fill-hover);
  }
  /* Play is the window's one accent fill, as in the bar. */
  .play {
    width: 44px;
    height: 44px;
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .lyric {
    font-size: calc(14px * var(--jf-text-scale, 1));
    font-weight: 600;
  }
  .close {
    position: absolute;
    top: 8px;
    inset-inline-end: 8px;
    width: 28px;
    height: 28px;
    color: var(--jf-ink-70);
    background: var(--jf-fill, transparent);
  }
  .close:hover {
    color: var(--jf-ink-90);
  }
  /* The progress: a straight 2 px line along the bottom edge, inset from its corners. */
  .progress {
    position: absolute;
    bottom: 5px;
    inset-inline: 12px;
    height: 2px;
    overflow: hidden;
    border-radius: 1px;
    background: var(--jf-ink-10);
  }
  .played {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--jf-ink-70);
  }
</style>
