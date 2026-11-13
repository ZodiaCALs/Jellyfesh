<script lang="ts">
  // A slider in the seek bar's style (v0.0.4): a straight line with round caps, ink-90 up to the
  // value and ink-30 after it (line-strong on plain surfaces), a 14 px ringed thumb while hovered,
  // focused or dragged (none on touch screens, where the line is thicker). A native range input
  // carries the value, the keyboard and what screen readers hear; it lies invisibly over the line.
  // `gradient` draws the whole track in a gradient (the hue slider) and always shows the thumb.
  let {
    value,
    min = 0,
    max = 1,
    step = 0.01,
    label,
    valuetext,
    oninput,
    onchange,
    plain = false,
    gradient = null,
    thumb = null,
    disabled = false,
    id,
  }: {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    valuetext?: string;
    oninput?: (value: number) => void;
    onchange?: (value: number) => void;
    plain?: boolean;
    /** A CSS gradient for the whole track, instead of the played part and the rest. */
    gradient?: string | null;
    /** The thumb's colour (the gradient slider's current colour). */
    thumb?: string | null;
    disabled?: boolean;
    id?: string;
  } = $props();

  /** The input's own value while it is being changed, before the owner has taken it. */
  let local = $state<number | null>(null);
  const shown = $derived(local ?? value);
  const fraction = $derived(max > min ? Math.min(1, Math.max(0, (shown - min) / (max - min))) : 0);

  function read(event: Event): number {
    return Number((event.currentTarget as HTMLInputElement).value);
  }
</script>

<span
  class="slider"
  class:plain
  class:gradient={gradient !== null}
  class:disabled
  style:--fraction={fraction}
  style:--track={gradient}
  style:--thumb={thumb}
>
  <span class="line"><span class="played"></span></span>
  <span class="thumb"></span>
  <input
    type="range"
    {id}
    {min}
    {max}
    {step}
    {value}
    {disabled}
    aria-label={label}
    aria-valuetext={valuetext}
    oninput={(event) => {
      local = read(event);
      oninput?.(local);
    }}
    onchange={(event) => {
      local = null;
      onchange?.(read(event));
    }}
  />
</span>

<style>
  .slider {
    --line: var(--jf-player-scrubber-track);
    --rest: var(--jf-ink-30);
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    height: 24px;
    border-radius: var(--jf-radius-r-full);
  }
  .plain {
    --rest: var(--jf-line-strong);
  }
  .line {
    position: relative;
    flex: 1;
    height: var(--fixed-line, var(--line));
    overflow: hidden;
    border-radius: var(--jf-radius-r-full);
    background: var(--rest);
    transition: height var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .played {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--jf-ink-90);
    translate: calc((var(--fraction) - 1) * 100%) 0;
  }
  /* A gradient track is thicker, so its colours show, and keeps its thickness. */
  .slider.gradient {
    --fixed-line: var(--jf-player-scrubber-track-drag);
  }
  .gradient .line {
    background: var(--track);
  }
  .gradient .played {
    display: none;
  }
  .thumb {
    position: absolute;
    top: 50%;
    left: calc(var(--fraction) * 100%);
    width: var(--jf-player-scrubber-thumb);
    height: var(--jf-player-scrubber-thumb);
    translate: -50% -50%;
    border-radius: var(--jf-radius-r-full);
    background: var(--thumb, var(--jf-ink-90));
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
  .slider:not(.disabled):hover,
  .slider:has(input:focus-visible) {
    --line: var(--jf-player-scrubber-track-hover);
  }
  .slider:not(.disabled):hover .thumb,
  .slider:has(input:focus-visible) .thumb,
  .slider:not(.disabled):has(input:active) .thumb,
  .gradient .thumb {
    opacity: 1;
    scale: 1;
  }
  .slider:not(.disabled):has(input:active) {
    --line: var(--jf-player-scrubber-track-drag);
  }
  .slider:has(input:focus-visible) {
    box-shadow: var(--jf-elevation-focus);
  }
  .disabled {
    opacity: 0.5;
  }
  input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
    background: none;
    touch-action: pan-y;
  }
  input:disabled {
    cursor: default;
  }
  /* A hairline native thumb, so a press anywhere maps to the value the line shows there. */
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 1px;
    height: 24px;
  }
  input::-moz-range-thumb {
    width: 1px;
    height: 24px;
    border: 0;
  }
  @media (pointer: coarse) {
    .slider,
    .slider:not(.disabled):hover,
    .slider:has(input:focus-visible) {
      --line: var(--jf-player-scrubber-track-touch);
    }
    .slider:not(.disabled):has(input:active) {
      --line: var(--jf-player-scrubber-track-touch-drag);
    }
    .slider:not(.gradient) .thumb {
      display: none;
    }
  }
</style>
