<script lang="ts">
  // An empty page (docs/ui.md: "No Playlists Yet", "No Favourites Yet"): a title, a sentence, and
  // what to do about it when there is something to do.
  import type { Snippet } from 'svelte';

  let { title, text, children }: { title: string; text: string; children?: Snippet } = $props();
</script>

<div class="empty">
  <h2>{title}</h2>
  <p>{text}</p>
  {#if children}<div class="actions">{@render children()}</div>{/if}
</div>

<style>
  .empty {
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s7) 0;
  }
  h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: calc(20px * var(--jf-text-scale, 1));
  }
  p {
    margin: 0;
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s5);
  }
</style>
