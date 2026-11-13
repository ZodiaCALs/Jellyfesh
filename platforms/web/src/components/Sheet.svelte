<script lang="ts">
  // A modal sheet on <dialog>: the browser keeps focus inside, makes the page behind inert, and
  // closes it with Esc. It scales from 96 % and fades in.
  import type { Snippet } from 'svelte';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
  let dialog: HTMLDialogElement | undefined = $state();
  const id = `sheet-${Math.random().toString(36).slice(2, 9)}`;

  $effect(() => {
    dialog?.showModal();
    return () => dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby={id}
  onclose={onclose}
  onclick={(event) => event.target === dialog && dialog?.close()}
>
  <div class="sheet">
    <header>
      <h2 {id}>{title}</h2>
      <button class="close" aria-label={t('Close')} title={t('Close')} onclick={() => dialog?.close()}><Icon name="close" /></button>
    </header>
    <div class="body">
      {@render children()}
    </div>
  </div>
</dialog>

<style>
  dialog {
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    padding: 0;
    border: 0;
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-3);
  }
  dialog::backdrop {
    background: var(--jf-scrim);
  }
  /* Sheets scale from 96 % and fade in (dur-2 Fluid); with reduced motion they only fade. */
  dialog[open] {
    animation: jf-pop-in var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  dialog[open]::backdrop {
    animation: jf-fade-in var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) dialog[open],
  :global(:root[data-motion='reduced']) dialog[open]::backdrop {
    animation: jf-fade-in var(--jf-motion-dur-1) linear;
  }
  .sheet {
    display: flex;
    flex-direction: column;
    max-height: calc(100dvh - 32px);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    padding: var(--jf-space-s4) var(--jf-space-s4) 0 var(--jf-space-s6);
  }
  h2 {
    margin: 0;
    font-size: var(--jf-type-roles-section-size);
    line-height: var(--jf-type-roles-section-line-height);
  }
  .close {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .close:hover {
    background: var(--jf-fill-hover);
  }
  .close:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .body {
    overflow: auto;
    padding: var(--jf-space-s4) var(--jf-space-s6) var(--jf-space-s6);
  }
</style>
