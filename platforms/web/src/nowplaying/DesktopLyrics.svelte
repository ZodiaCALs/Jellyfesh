<script lang="ts">
  // Desktop Lyrics' window (docs/ui.md "Lyrics, word by word"; drawn into a Document
  // Picture-in-Picture window by DesktopLyricsHost): the line being sung, filling word by word in
  // the highlight colour, and the next line under it. Before the first line and without timed
  // lyrics, the song's title and artist; with nothing playing, "Nothing Playing". Pointing at it
  // shows Close. Browsers let no page pass clicks through a window, so there is no Lock here.
  import type { App } from '../lib/app';
  import LyricsBarLine from '../components/LyricsBarLine.svelte';
  import Icon from '../components/Icon.svelte';
  import { t } from '../lib/i18n/index.svelte';
  import { LyricsBarClock } from '../lib/state/lyricsBar.svelte';
  import { untrack } from 'svelte';

  let { app, win, onclose }: { app: App; win: Window; onclose: () => void } = $props();

  const clock = new LyricsBarClock(() => win.document.visibilityState === 'hidden');
  const row = $derived(app.player.current);
  const lyrics = $derived(app.player.lyrics.status === 'ready' ? app.player.lyrics.lyrics : null);
  const timed = $derived(lyrics !== null && lyrics.synced && lyrics.doc.timing !== 'unsynced');
  $effect(() => {
    const timeline = timed && lyrics ? lyrics.timeline : null;
    const anchor = app.player.anchor;
    untrack(() => clock.set(timeline, anchor));
  });
  $effect(() => {
    const wake = () => clock.wake();
    win.document.addEventListener('visibilitychange', wake);
    return () => {
      win.document.removeEventListener('visibilitychange', wake);
      clock.destroy();
    };
  });
  const line = $derived(timed && lyrics && clock.bar.line !== null ? (lyrics.doc.lines[clock.bar.line] ?? null) : null);
  const next = $derived(timed && lyrics && clock.bar.next !== null ? (lyrics.doc.lines[clock.bar.next] ?? null) : null);
  const fallback = $derived(row ? [row.title, row.artist].filter(Boolean).join(' · ') : t('Nothing Playing'));
  $effect(() => {
    win.document.title = t('Desktop Lyrics');
  });
</script>

<div class="desktop" class:center={app.settings.value.lyrics_align === 'center'}>
  {#if line && lyrics}
    <p class="line-now" lang={lyrics.doc.language ?? undefined}>
      <LyricsBarLine {line} {clock} wordTimed={lyrics.doc.timing === 'word'} reduced={app.theme.reducedMotion} />
    </p>
    {#if next}
      <p class="line-next" lang={lyrics.doc.language ?? undefined}>
        <LyricsBarLine line={next} {clock} wordTimed={false} reduced={app.theme.reducedMotion} sung={false} />
      </p>
    {/if}
  {:else}
    <p class="fallback">{fallback}</p>
  {/if}
  <button class="close" aria-label={t('Close Desktop Lyrics')} title={t('Close Desktop Lyrics')} onclick={onclose}>
    <Icon name="close" size={16} />
  </button>
</div>

<style>
  .desktop {
    position: relative;
    box-sizing: border-box;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
    padding: 12px 20px;
  }
  .center {
    text-align: center;
  }
  p {
    margin: 0;
    min-width: 0;
  }
  .line-now {
    font-size: calc(26px * var(--jf-text-scale, 1));
    font-weight: var(--jf-lyrics-line-weight, 700);
    letter-spacing: -0.01em;
    line-height: 1.25;
  }
  .line-next {
    font-size: calc(17px * var(--jf-text-scale, 1));
    font-weight: 600;
    line-height: 1.3;
  }
  .fallback {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: calc(18px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  .close {
    position: absolute;
    top: 8px;
    inset-inline-end: 8px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--jf-radius-r-full, 50%);
    background: var(--jf-fill, transparent);
    color: var(--jf-ink-70);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--jf-motion-dur-1, 120ms) linear;
  }
  .desktop:hover .close,
  .close:focus-visible {
    opacity: 1;
  }
  .close:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
