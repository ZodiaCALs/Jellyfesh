<script lang="ts">
  // What the library pages show above their lists: folders to pick again this visit before their
  // songs play, and, with nothing in the library, the empty page (docs/ui.md: "No Albums Yet", the
  // text, Add Music Folder…).
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { toast } from '../lib/state/toast.svelte';
  import Icon from './Icon.svelte';

  let { title, text }: { title: string; text: string } = $props();
  const app = useApp();
  const waiting = $derived(app.library.folders.filter((f) => f.access !== 'ready'));

  async function add() {
    try {
      await app.library.addFolder();
    } catch (error) {
      toast(error instanceof Error ? error.message : String(error));
    }
  }
</script>

{#each waiting as folder (folder.id)}
  <p class="notice" role="note">
    <Icon name="folder" size={18} />
    <span>{t('Pick “{name}” again to play its songs on this visit.', { name: folder.name })}</span>
    <button onclick={() => app.library.pickAgain(folder.id)}>{t('Pick Folder…')}</button>
  </p>
{/each}

{#if app.library.isEmpty && !app.library.progress}
  <div class="empty">
    <h2>{title}</h2>
    <p>{text}</p>
    <button class="primary" onclick={add}><Icon name="folder-plus" size={18} />{t('Add Music Folder…')}</button>
  </div>
{:else if app.library.progress && app.library.isEmpty}
  <p class="scanning" aria-live="polite">{t('Reading “{name}”… {count} songs so far', { name: app.library.progress.folder, count: app.library.progress.seen })}</p>
{/if}

<style>
  .notice {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    margin: 0 0 var(--jf-space-s4);
    padding: var(--jf-space-s3) var(--jf-space-s4);
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-tint-mist);
    box-shadow: inset 0 0 0 1px var(--jf-glass-edge-soft);
    font-size: 14px;
  }
  .notice span {
    flex: 1;
  }
  .notice :global(svg) {
    flex: none;
    color: var(--jf-accent-text);
  }
  .notice button {
    flex: none;
    min-height: 36px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .empty {
    max-width: var(--jf-layout-reading);
    padding: var(--jf-space-s7) 0;
  }
  .empty h2 {
    margin: 0 0 var(--jf-space-s2);
    font-size: 20px;
  }
  .empty p {
    margin: 0 0 var(--jf-space-s5);
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .primary {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .scanning {
    color: var(--jf-ink-70);
  }
</style>
