<script lang="ts">
  // One lyric line in one row (the lyrics bar and Desktop Lyrics): its words fill from the start
  // edge in the highlight colour as they are sung, over the unsung ink; the word being sung sweeps
  // as a CSS animation (jf-sweep-*, app.css). A line without word timing, a right-to-left line, and
  // any line under Reduce Motion show whole in the sung colour, as in the apps. Too long for the
  // row, it ends with an ellipsis.
  import type { Line } from '../lib/core/types';
  import { type LyricsBarClock, sweepOf } from '../lib/state/lyricsBar.svelte';

  let {
    line,
    clock,
    wordTimed,
    reduced,
    sung = true,
  }: {
    line: Line;
    clock: LyricsBarClock;
    /** The lyrics are timed word by word. */
    wordTimed: boolean;
    reduced: boolean;
    /** The line the bar is on (false: the next line, shown unsung). */
    sung?: boolean;
  } = $props();

  const words = $derived(
    sung && wordTimed && !reduced && line.direction !== 'rtl' && line.words.length > 0 && clock.bar.words.length === line.words.length
      ? clock.bar.words
      : null,
  );
  const sweep = $derived(words ? sweepOf(words) : null);
</script>

<span class="bar-line" class:sung={sung && !words} class:next={!sung} dir={line.direction}>
  {#if words && sweep}
    {#each line.words as word, w (w)}
      {@const current = w === sweep.word}
      {@const lit = w < sweep.word || (current && sweep.progress >= 1)}
      {@const moving = current && !lit && clock.running && clock.remainingMs > 0}
      <span
        class="word"
        class:lit
        style:--from="{Math.round((current ? sweep.progress : lit ? 1 : 0) * 100)}%"
        style:--ms="{Math.round(clock.remainingMs)}ms"
        style:animation-name={moving ? `jf-sweep-${clock.parity % 2}` : 'none'}
        >{word.text}{word.joins_next ? '' : ' '}</span
      >
    {/each}
  {:else}
    {line.text}
  {/if}
</span>

<style>
  .bar-line {
    display: block;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--jf-ink-70);
  }
  .bar-line.sung {
    color: var(--jf-lyrics-highlight, var(--jf-ink-90));
  }
  .bar-line.next {
    color: var(--jf-ink-50, var(--jf-ink-70));
  }
  /* As the Lyrics pane's words: the sung layer is the highlight colour, feathered over 0.3 em. */
  .word {
    color: transparent;
    background-image:
      linear-gradient(90deg, var(--jf-lyrics-highlight, var(--jf-ink-90)) calc(100% - 0.3em), transparent),
      linear-gradient(var(--jf-ink-70), var(--jf-ink-70));
    background-repeat: no-repeat;
    background-size:
      calc(var(--from) + 0.3em) 100%,
      100% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    animation-duration: var(--ms);
    animation-timing-function: linear;
    animation-fill-mode: forwards;
  }
  .word.lit {
    background-size:
      calc(100% + 0.3em) 100%,
      100% 100%;
  }
</style>
