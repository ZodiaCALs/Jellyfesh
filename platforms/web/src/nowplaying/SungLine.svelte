<script lang="ts">
  // The line being sung under the cover in the look's Artwork layout (ADR-0020), filling word by
  // word; nothing without timed lyrics.
  import { untrack } from 'svelte';
  import LyricsBarLine from '../components/LyricsBarLine.svelte';
  import { useApp } from '../lib/app';
  import { LyricsBarClock, showsLyricsBar } from '../lib/state/lyricsBar.svelte';

  const app = useApp();
  const player = app.player;
  const lyrics = $derived(player.lyrics.status === 'ready' ? player.lyrics.lyrics : null);
  const timed = $derived(showsLyricsBar(true, lyrics));
  const clock = new LyricsBarClock();
  $effect(() => {
    const timeline = timed && lyrics ? lyrics.timeline : null;
    const anchor = player.anchor;
    untrack(() => clock.set(timeline, anchor));
  });
  $effect(() => () => clock.destroy());
  const line = $derived(timed && lyrics && clock.bar.line !== null ? (lyrics.doc.lines[clock.bar.line] ?? null) : null);
</script>

{#if line && lyrics}
  <p class="sung" lang={lyrics.doc.language ?? undefined}>
    <LyricsBarLine {line} {clock} wordTimed={lyrics.doc.timing === 'word'} reduced={app.theme.reducedMotion} />
  </p>
{/if}

<style>
  .sung {
    margin: 0;
    max-width: 100%;
    text-align: center;
    font-size: calc(24px * var(--jf-text-scale, 1));
    font-weight: var(--jf-lyrics-line-weight, 700);
    letter-spacing: -0.01em;
  }
</style>
