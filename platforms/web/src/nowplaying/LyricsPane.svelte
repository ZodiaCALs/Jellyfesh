<script lang="ts">
  // Lyrics that follow the song: the active line sits at lyrics.anchor-position (38 %) of the pane,
  // its words light up as they are sung, with the translation and romanization under each line
  // when the lyrics have them and the person wants them. Redrawn only when a line or word changes
  // (the core's next_change); a sweeping word is a CSS animation, so nothing runs per frame
  // otherwise.
  //
  // After Apple Music (v0.0.4; Luminous v0.0.6): when the line changes, the lines move up in a
  // cascade. The pane jumps to its new scroll position, every line on screen is put back where it
  // was drawn with a translate, and CSS transitions (dur-3 Fluid) bring them home, each line
  // lyrics.stagger (40 ms) after the one above (a per-line transition-delay). Lines are
  // lyrics.line (40 px bold where the pane is wide, line height 1.18); the active line is ink-90
  // at full size, the others fade to lyrics.ink-inactive (sung ones ink-30) through opacity and
  // scale to lyrics.scale-inactive (92 %) from their start edge: transform and opacity only, which
  // the compositor runs, and no blur. Size and alignment follow the settings; tapping a line seeks
  // to it. Reduced motion: lines jump, words light up whole.
  import { tick, untrack } from 'svelte';
  import { useApp } from '../lib/app';
  import type { Line, Lyrics } from '../lib/core/types';
  import { cascadeStep } from '../lib/drawing/cascade';
  import { cursor, nextChange } from '../lib/drawing/lyricCursor';
  import type { PlaybackAnchor } from '../lib/drawing/displayClock';
  import { Ticker } from '../lib/drawing/ticker';
  import { t } from '../lib/i18n/index.svelte';

  let { lyrics, anchor }: { lyrics: Lyrics; anchor: PlaybackAnchor } = $props();
  const app = useApp();

  let activeLine = $state<number | null>(null);
  let activeWord = $state<number | null>(null);
  let progress = $state(0);
  let remainingMs = $state(0);
  let running = $state(false);
  let parity = $state(0);
  let pane: HTMLDivElement | undefined = $state();
  let userScrolledAt = 0;
  /** The first follow after the lyrics appear jumps; later ones cascade. */
  let settled = false;

  const reduced = $derived(app.theme.reducedMotion);
  const synced = $derived(lyrics.synced && lyrics.doc.timing !== 'unsynced');
  const translation = $derived(
    app.settings.value.show_translation ? lyrics.doc.tracks.find((tr) => tr.kind === 'translation') : undefined,
  );
  const romanization = $derived(
    app.settings.value.show_romanization ? lyrics.doc.tracks.find((tr) => tr.kind === 'romanization') : undefined,
  );
  const size = $derived(app.settings.value.lyrics_size);
  const align = $derived(app.settings.value.lyrics_align);
  const scale = $derived(
    size === 'small' ? 'var(--jf-lyrics-scale-small)' : size === 'large' ? 'var(--jf-lyrics-scale-large)' : '1',
  );

  const ticker = new Ticker((us, isRunning) => {
    const tl = lyrics.timeline;
    const ms = Number(us / 1000n);
    const at = cursor(tl, ms);
    const lineChanged = at.line !== activeLine;
    activeLine = at.line;
    activeWord = at.word;
    progress = at.wordProgress;
    running = isRunning;
    // How long the current word still sweeps.
    if (at.line !== null && at.word !== null) {
      const index = tl.word_offsets[at.line] + at.word;
      remainingMs = Math.max(0, (tl.word_ends[index] ?? ms) - ms) / (anchor.rate > 0 ? anchor.rate : 1);
    } else {
      remainingMs = 0;
    }
    parity += 1;
    if (lineChanged) queueMicrotask(() => follow());
    const next = nextChange(tl, ms);
    return next === null ? null : BigInt(next) * 1000n;
  });

  // Redraw for a new anchor or new lyrics; the drawing itself reads state, so it runs untracked.
  $effect(() => {
    const current = anchor;
    const on = synced;
    void lyrics;
    untrack(() => {
      if (on) ticker.set(current);
      else activeLine = null;
    });
  });
  $effect(() => () => ticker.destroy());
  // New lyrics start with a jump to their line.
  $effect(() => {
    void lyrics;
    settled = false;
  });
  // Another size or alignment moves every line: put the active one back in place at once.
  $effect(() => {
    void size;
    void align;
    untrack(() => void tick().then(() => follow(false)));
  });
  // So does another pane height.
  $effect(() => {
    const element = pane;
    if (!element) return;
    let height = element.clientHeight;
    const observer = new ResizeObserver(() => {
      if (element.clientHeight === height) return;
      height = element.clientHeight;
      follow(false);
    });
    observer.observe(element);
    return () => observer.disconnect();
  });

  /** The lyrics.anchor-position token: where the active line's middle sits in the pane. */
  function anchorAt(element: HTMLElement): number {
    const value = Number.parseFloat(getComputedStyle(element).getPropertyValue('--jf-lyrics-anchor-position'));
    return Number.isFinite(value) ? value : 0.38;
  }

  /** How far a line is drawn from its place now (still moving in a cascade), px. */
  function offsetOf(element: HTMLElement): number {
    const translate = getComputedStyle(element).translate;
    if (!translate || translate === 'none') return 0;
    const parts = translate.split(' ');
    return parts.length > 1 ? Number.parseFloat(parts[1]) || 0 : 0;
  }

  /**
   * Keeps the active line at the anchor position, unless the person scrolled a moment ago: a
   * cascade when `animate`, motion is allowed and the move is shorter than the pane; else a jump.
   */
  function follow(animate = true) {
    if (!pane || activeLine === null) return;
    if (performance.now() - userScrolledAt < 4000) return;
    const line = pane.querySelector<HTMLElement>(`[data-line="${activeLine}"]`);
    if (!line) return;
    const height = pane.clientHeight;
    const most = Math.max(0, pane.scrollHeight - height);
    const target = Math.min(most, Math.max(0, line.offsetTop + line.offsetHeight / 2 - height * anchorAt(pane)));
    const from = pane.scrollTop;
    const lines = [...pane.querySelectorAll<HTMLElement>('.line')];
    if (!animate || !settled || reduced || Math.abs(target - from) > height) {
      settled = true;
      for (const element of lines) element.style.removeProperty('--dy');
      if (Math.abs(target - from) >= 1) pane.scrollTop = target;
      return;
    }
    if (Math.abs(target - from) < 1) return;
    // The lines on screen before or after the move.
    const top = Math.min(from, target) - 64;
    const bottom = Math.max(from, target) + height + 64;
    const moving = lines.filter((element) => element.offsetTop + element.offsetHeight >= top && element.offsetTop <= bottom);
    const drawn = moving.map(offsetOf);
    pane.scrollTop = target;
    const moved = pane.scrollTop - from;
    // First each line goes back to where it was drawn, without a transition...
    moving.forEach((element, index) => {
      element.classList.add('jump');
      element.style.setProperty('--dy', `${(drawn[index] + moved).toFixed(2)}px`);
    });
    void pane.offsetHeight;
    // ...then home, each a stagger after the line above it.
    const active = activeLine;
    for (const element of moving) {
      element.style.setProperty('--k', String(cascadeStep(Number(element.dataset.line), active)));
      element.classList.remove('jump');
      element.style.setProperty('--dy', '0px');
    }
  }

  function seekTo(line: Line) {
    if (line.start_ms === null) return;
    userScrolledAt = 0;
    app.player.seek(line.start_ms);
    if (!app.player.playing) app.player.play();
  }

  function aux(track: typeof translation, line: Line): string | null {
    return track?.lines[String(line.id)]?.text ?? null;
  }
</script>

<!-- Scrolling by hand pauses the follow for a moment (wheel, touch or keys on the region). -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="lyrics"
  class:synced
  class:reduced
  class:center={align === 'center'}
  style:--size={scale}
  bind:this={pane}
  onwheel={() => (userScrolledAt = performance.now())}
  ontouchmove={() => (userScrolledAt = performance.now())}
  onkeydown={(event) => {
    if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown'].includes(event.key)) userScrolledAt = performance.now();
  }}
>
  <div class="spacer top" aria-hidden="true"></div>
  {#each lyrics.doc.lines as line, index (line.id)}
    {@const active = index === activeLine}
    {@const translated = aux(translation, line)}
    {@const romanized = aux(romanization, line)}
    <div
      class="line"
      class:active
      class:sung={activeLine !== null && index < activeLine}
      class:end={line.agent === 'v2'}
      class:rtl={line.direction === 'rtl'}
      data-line={index}
    >
      <button
        class="text"
        dir={line.direction}
        disabled={!synced || line.start_ms === null}
        aria-current={active ? 'true' : undefined}
        onclick={() => seekTo(line)}
      >
        {#if active && line.words.length > 0 && lyrics.doc.timing === 'word'}
          {#each line.words as word, w (w)}
            {@const sung = activeWord !== null && w < activeWord}
            {@const current = w === activeWord}
            <span
              class="word"
              class:sung
              class:current
              class:sweeping={current && running && !reduced && remainingMs > 0}
              style:--from="{Math.round((current ? progress : sung ? 1 : 0) * 100)}%"
              style:--ms="{Math.round(remainingMs)}ms"
              style:animation-name={current && running && !reduced && remainingMs > 0 ? `sweep-${parity % 2}` : 'none'}
              >{word.text}{word.joins_next ? '' : ' '}</span
            >
          {/each}
        {:else}
          {line.text}
        {/if}
      </button>
      {#if line.background.length}
        <p class="background" dir={line.direction}>{line.background.map((w) => w.text + (w.joins_next ? '' : ' ')).join('')}</p>
      {/if}
      {#if romanized}<p class="aux romanization" lang="{lyrics.doc.language ?? 'und'}-Latn">{romanized}</p>{/if}
      {#if translated}<p class="aux translation" lang={translation?.language ?? undefined}>{translated}</p>{/if}
    </div>
  {/each}
  <div class="spacer bottom" aria-hidden="true"></div>
  {#if lyrics.doc.source.origin || lyrics.doc.metadata.by}
    <p class="credit">{lyrics.doc.source.origin ?? lyrics.doc.metadata.by}</p>
  {/if}
</div>

<style>
  .lyrics {
    position: relative;
    height: 100%;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: none;
    padding: 0 var(--jf-space-s2);
    mask-image: linear-gradient(to bottom, transparent 0, black 72px, black calc(100% - 96px), transparent 100%);
  }
  .lyrics::-webkit-scrollbar {
    display: none;
  }
  .spacer.top {
    height: 30%;
  }
  .spacer.bottom {
    height: 55%;
  }
  .line {
    --dy: 0px;
    --k: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: var(--jf-lyrics-line-gap);
    content-visibility: auto;
    contain-intrinsic-size: auto 60px;
    translate: 0 var(--dy);
    scale: var(--jf-lyrics-scale-inactive);
    opacity: var(--jf-lyrics-dim, 0.67);
    transform-origin: left center;
    transition:
      translate var(--jf-motion-dur-3) var(--jf-motion-ease-fluid) calc(var(--k) * var(--jf-lyrics-stagger)),
      scale var(--jf-motion-dur-3) var(--jf-motion-ease-fluid),
      opacity var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  /* Put back where it was drawn, before the cascade brings it home. */
  .line:global(.jump) {
    transition:
      scale var(--jf-motion-dur-3) var(--jf-motion-ease-fluid),
      opacity var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  .line.sung {
    opacity: var(--jf-lyrics-dim-sung, 0.38);
  }
  .line.active,
  .lyrics:not(.synced) .line {
    scale: 1;
    opacity: 1;
  }
  .line.rtl {
    transform-origin: right center;
  }
  .line.end {
    align-items: flex-end;
    text-align: end;
    transform-origin: right center;
  }
  .line.end.rtl {
    transform-origin: left center;
  }
  .center .line,
  .center .line.end {
    align-items: center;
    text-align: center;
    transform-origin: center;
  }
  .reduced .line {
    scale: 1;
  }
  .text {
    display: block;
    width: 100%;
    margin: 0;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: none;
    font: inherit;
    /* lyrics.line between its fluid ends, by the pane's width (Now Playing's pane is a container). */
    font-size: calc(clamp(var(--jf-lyrics-line-fluid-min), 6.4cqi, var(--jf-lyrics-line-fluid-max)) * var(--size, 1));
    line-height: var(--jf-lyrics-line-line-height);
    font-weight: var(--jf-lyrics-line-weight);
    letter-spacing: -0.02em;
    text-align: inherit;
    color: var(--jf-ink-90);
    overflow-wrap: anywhere;
    cursor: pointer;
  }
  :global(:lang(zh)) .text,
  :global(:lang(ja)) .text {
    line-height: var(--jf-lyrics-line-cjk-line-height);
  }
  .text:disabled {
    cursor: default;
  }
  .synced .text:hover:not(:disabled) {
    background: var(--jf-fill);
  }
  .text:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .word {
    color: transparent;
    background-image: linear-gradient(var(--jf-ink-90), var(--jf-ink-90)), linear-gradient(var(--jf-ink-70), var(--jf-ink-70));
    background-repeat: no-repeat;
    background-size:
      var(--from) 100%,
      100% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  .word.sung {
    background-size:
      100% 100%,
      100% 100%;
  }
  .reduced .word.current {
    background-size:
      100% 100%,
      100% 100%;
  }
  .word.sweeping {
    animation-duration: var(--ms);
    animation-timing-function: linear;
    animation-fill-mode: forwards;
  }
  @keyframes sweep-0 {
    from {
      background-size:
        var(--from) 100%,
        100% 100%;
    }
    to {
      background-size:
        100% 100%,
        100% 100%;
    }
  }
  @keyframes sweep-1 {
    from {
      background-size:
        var(--from) 100%,
        100% 100%;
    }
    to {
      background-size:
        100% 100%,
        100% 100%;
    }
  }
  .background {
    margin: 0;
    padding: 0 8px;
    font-size: calc(var(--jf-lyrics-translation-size) * var(--size, 1));
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  .aux {
    margin: 0;
    padding: 0 8px;
    color: var(--jf-ink-70);
  }
  .translation {
    font-size: calc(var(--jf-lyrics-translation-size) * var(--size, 1));
    line-height: var(--jf-lyrics-translation-line-height);
    font-weight: var(--jf-lyrics-translation-weight);
  }
  .romanization {
    font-size: calc(var(--jf-lyrics-romanization-size) * var(--size, 1));
    line-height: var(--jf-lyrics-romanization-line-height);
    font-weight: var(--jf-lyrics-romanization-weight);
  }
  .credit {
    margin: 0 8px var(--jf-space-s8);
    font-size: 12px;
    color: var(--jf-ink-70);
  }
</style>
