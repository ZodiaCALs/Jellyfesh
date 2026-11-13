<script lang="ts">
  // The playing row's level glyph: three bars that move only while playing. A CSS animation of
  // `scale` (the compositor runs it), paused with playback, and still with reduced motion.
  let { moving }: { moving: boolean } = $props();
</script>

<span class="level" class:moving aria-hidden="true"><span></span><span></span><span></span></span>

<style>
  .level {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    width: 14px;
    height: 14px;
    color: var(--jf-accent-text);
  }
  .level span {
    flex: 1;
    height: 100%;
    border-radius: 1px;
    background: currentColor;
    transform-origin: bottom;
    scale: 1 0.5;
    animation: level 0.9s ease-in-out infinite alternate paused;
  }
  /* Negative delays: paused, the bars stand at three different heights. */
  .level span:nth-child(1) {
    animation-duration: 0.8s;
    animation-delay: -0.5s;
  }
  .level span:nth-child(2) {
    animation-duration: 1.05s;
    animation-delay: -0.2s;
  }
  .level span:nth-child(3) {
    animation-duration: 0.7s;
    animation-delay: -0.6s;
  }
  .moving span {
    animation-play-state: running;
  }
  :global(:root[data-motion='reduced']) .level span {
    animation: none;
  }
  :global(:root[data-motion='reduced']) .level span:nth-child(1) {
    scale: 1 0.6;
  }
  :global(:root[data-motion='reduced']) .level span:nth-child(2) {
    scale: 1 1;
  }
  :global(:root[data-motion='reduced']) .level span:nth-child(3) {
    scale: 1 0.4;
  }
  @keyframes level {
    from {
      scale: 1 0.25;
    }
    to {
      scale: 1 1;
    }
  }
</style>
