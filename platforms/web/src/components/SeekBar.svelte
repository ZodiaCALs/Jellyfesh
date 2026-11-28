<script lang="ts">
  // The seek bar (v0.0.4, after Apple Music and Media Player): a straight line with round caps, the
  // played part ink-90 and the rest ink-30 (line-strong on plain surfaces), elapsed and remaining
  // times at its ends. No thumb at rest. Pointer screens: hovering or focusing thickens the line to
  // 6 px and shows a 14 px ringed thumb (keyboard focus shows no focus ring, v0.0.9); dragging
  // thickens it to 8 px and the times follow the thumb. Touch screens (pointer: coarse): a 6 px
  // line, no thumb (but for keyboard focus), 12 px while dragging. The fill
  // redraws only when it moves by a device pixel or a time label's second changes (the
  // DisplayClock), never per frame, never while paused, hidden or inactive.
  import { untrack } from 'svelte';
  import type { PlaybackAnchor } from '../lib/drawing/displayClock';
  import { nextDrawUs } from '../lib/drawing/steps';
  import { Ticker } from '../lib/drawing/ticker';
  import { formatTime, t } from '../lib/i18n/index.svelte';

  let {
    anchor,
    durationMs,
    onseek,
    disabled = false,
    compact = false,
    plain = false,
    active = true,
  }: {
    anchor: PlaybackAnchor;
    durationMs: number | null;
    onseek: (ms: number) => void;
    disabled?: boolean;
    /** Times beside the line instead of under it (the player bar). */
    compact?: boolean;
    /** On a plain surface rather than glass or a backdrop: the rest of the line is line-strong. */
    plain?: boolean;
    /** Shown: an inactive bar's clock sleeps. */
    active?: boolean;
  } = $props();

  let positionMs = $state(0);
  let dragging = $state(false);
  let dragMs = $state(0);
  let track: HTMLDivElement | undefined = $state();
  /** The line's length in CSS pixels. */
  let width = $state(0);
  let rtl = $state(false);

  const duration = $derived(durationMs && durationMs > 0 ? durationMs : null);
  const off = $derived(disabled || !duration);

  const ticker = new Ticker((us) => {
    positionMs = Number(us / 1000n);
    const length = duration;
    return nextDrawUs(us, length ? BigInt(Math.round(length * 1000)) : null, width, devicePixelRatio);
  });
  $effect(() => {
    const current = anchor;
    untrack(() => ticker.set(current));
  });
  $effect(() => {
    void duration;
    void width;
    untrack(() => ticker.tick());
  });
  $effect(() => {
    const shown = active;
    untrack(() => ticker.setActive(shown));
  });
  $effect(() => () => ticker.destroy());

  $effect(() => {
    const element = track;
    if (!element) return;
    rtl = getComputedStyle(element).direction === 'rtl';
    const observer = new ResizeObserver((entries) => {
      width = entries[entries.length - 1].contentRect.width;
    });
    observer.observe(element);
    return () => observer.disconnect();
  });

  const shown = $derived(dragging ? dragMs : positionMs);
  const fraction = $derived(duration ? Math.min(1, Math.max(0, shown / duration)) : 0);
  const sign = $derived(rtl ? -1 : 1);
  const elapsed = $derived(formatTime(shown));
  const remaining = $derived(duration ? `−${formatTime(Math.max(0, duration - shown))}` : formatTime(null));

  function msAt(clientX: number): number {
    if (!track || !duration) return 0;
    const box = track.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (clientX - box.left) / box.width));
    return (rtl ? 1 - x : x) * duration;
  }

  function down(event: PointerEvent) {
    if (off || event.button !== 0) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragging = true;
    dragMs = msAt(event.clientX);
  }
  function move(event: PointerEvent) {
    if (dragging) dragMs = msAt(event.clientX);
  }
  function up(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    onseek(msAt(event.clientX));
  }
  function key(event: KeyboardEvent) {
    if (off || !duration) return;
    const step = { ArrowLeft: -5000, ArrowDown: -5000, ArrowRight: 5000, ArrowUp: 5000, PageDown: -30000, PageUp: 30000 }[
      event.key
    ];
    if (step !== undefined) onseek(Math.min(duration, Math.max(0, Number(ticker.position() / 1000n) + step)));
    else if (event.key === 'Home') onseek(0);
    else if (event.key === 'End') onseek(duration);
    else return;
    event.preventDefault();
  }
</script>

<div class="seek" class:compact class:plain class:dragging class:off>
  {#if compact}<span class="time start" aria-hidden="true">{elapsed}</span>{/if}
  <div
    class="track"
    bind:this={track}
    role="slider"
    tabindex={off ? -1 : 0}
    aria-label={t('Position')}
    aria-valuemin={0}
    aria-valuemax={Math.round((duration ?? 0) / 1000)}
    aria-valuenow={Math.round(shown / 1000)}
    aria-valuetext={t('{position} of {duration}', { position: formatTime(shown), duration: formatTime(duration) })}
    aria-disabled={off}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={() => (dragging = false)}
    onkeydown={key}
  >
    <span class="line"><span class="played" style:translate="{(fraction - 1) * 100 * sign}% 0"></span></span>
    <span class="thumb" style:translate="{fraction * width * sign}px 0"></span>
  </div>
  {#if compact}
    <span class="time end" aria-hidden="true">{remaining}</span>
  {:else}
    <div class="times" aria-hidden="true">
      <span>{elapsed}</span>
      <span>{remaining}</span>
    </div>
  {/if}
</div>

<style>
  .seek {
    /* The line's thickness and the colour of its unplayed part. */
    --line: var(--jf-player-scrubber-track);
    --rest: var(--jf-ink-30);
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .plain {
    --rest: var(--jf-line-strong);
  }
  .compact {
    flex-direction: row;
    align-items: center;
    gap: var(--jf-space-s2);
  }
  .track {
    position: relative;
    display: flex;
    align-items: center;
    height: 24px;
    flex: 1;
    min-width: 0;
    cursor: pointer;
    touch-action: none;
    border-radius: var(--jf-radius-r-full);
  }
  /* Keyboard focus thickens the line and shows the thumb (below), never a focus ring (v0.0.9). */
  .track:focus-visible {
    outline: none;
  }
  .off .track {
    cursor: default;
  }
  .line {
    position: relative;
    flex: 1;
    height: var(--line);
    overflow: hidden;
    border-radius: var(--jf-radius-r-full);
    background: var(--rest);
    transition: height var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  /* Progress is Linear (MOT-06): the fill steps by a device pixel at a time, without tweening. */
  .played {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--jf-ink-90);
  }
  .thumb {
    position: absolute;
    top: 50%;
    inset-inline-start: calc(var(--jf-player-scrubber-thumb) / -2);
    width: var(--jf-player-scrubber-thumb);
    height: var(--jf-player-scrubber-thumb);
    margin-top: calc(var(--jf-player-scrubber-thumb) / -2);
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-ink-90);
    box-shadow:
      0 0 0 2px var(--jf-glass-edge),
      var(--jf-elevation-shadow-1);
    opacity: 0;
    scale: 0.5;
    pointer-events: none;
    transition:
      opacity var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      scale var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  /* Pointer screens: hover or focus thickens the line and shows the thumb; dragging thickens more. */
  .seek:not(.off) .track:hover,
  .seek:not(.off) .track:focus-visible {
    --line: var(--jf-player-scrubber-track-hover);
  }
  .seek:not(.off) .track:hover .thumb,
  .seek:not(.off) .track:focus-visible .thumb,
  .dragging .thumb {
    opacity: 1;
    scale: 1;
  }
  .seek.dragging:not(.off) .track {
    --line: var(--jf-player-scrubber-track-drag);
  }
  .times {
    display: flex;
    justify-content: space-between;
  }
  .times,
  .time {
    font-size: calc(12px * var(--jf-text-scale, 1));
    line-height: 1.45;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-70);
  }
  .time {
    flex: none;
    min-width: 4.5ch;
  }
  .time.start {
    text-align: end;
  }
  /* Touch screens: a thicker line and no thumb; a taller target than the line shows. */
  @media (pointer: coarse) {
    .seek {
      --line: var(--jf-player-scrubber-track-touch);
    }
    .seek:not(.off) .track:hover,
    .seek:not(.off) .track:focus-visible {
      --line: var(--jf-player-scrubber-track-touch);
    }
    .seek.dragging:not(.off) .track {
      --line: var(--jf-player-scrubber-track-touch-drag);
    }
    .thumb {
      display: none;
    }
    /* With a keyboard, the thumb still shows where the focus is. */
    .seek:not(.off) .track:focus-visible .thumb {
      display: block;
    }
    .track::before {
      content: '';
      position: absolute;
      inset: -10px 0;
    }
  }
</style>
