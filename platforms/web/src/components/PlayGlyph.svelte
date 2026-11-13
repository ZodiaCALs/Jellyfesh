<script lang="ts" module>
  // Play and pause as two four-point shapes each (the triangle's halves, the two bars), so the path
  // data can move from one to the other: the morph (dur-2 Spring) is a transition of the CSS `d`
  // property. A stroke in the same colour rounds the corners (ICO-03: filled because media plays).
  // Browsers that cannot transition `d` (Safari) swap the glyph at once, from the attribute.
  const PLAY = ['M8.5 6L13.5 9L13.5 15L8.5 18Z', 'M13.5 9L18.5 12L18.5 12L13.5 15Z'];
  const PAUSE = ['M7.25 5.75L10.25 5.75L10.25 18.25L7.25 18.25Z', 'M13.75 5.75L16.75 5.75L16.75 18.25L13.75 18.25Z'];
</script>

<script lang="ts">
  /** `playing`: the button pauses, so it shows pause. */
  let { playing, size = 24 }: { playing: boolean; size?: number } = $props();
  const shapes = $derived(playing ? PAUSE : PLAY);
</script>

<svg aria-hidden="true" focusable="false" width={size} height={size} viewBox="0 0 24 24">
  {#each shapes as d, index (index)}
    <path {d} style:d="path('{d}')" />
  {/each}
</svg>

<style>
  path {
    fill: currentColor;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linejoin: round;
    transition: d var(--jf-motion-dur-2) var(--jf-motion-ease-spring);
  }
</style>
