<script lang="ts">
  // "Remove Folder…", "Uninstall…" and the like ask first (docs/ui.md, "Steps that ask first"):
  // what happens, then Cancel and the action.
  import { t } from '../lib/i18n/index.svelte';
  import Sheet from './Sheet.svelte';

  let {
    title,
    message,
    action,
    destructive = true,
    onconfirm,
    oncancel,
  }: {
    title: string;
    message: string;
    action: string;
    destructive?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<Sheet {title} onclose={oncancel}>
  <p class="message">{message}</p>
  <div class="actions">
    <button class="cancel" onclick={oncancel}>{t('Cancel')}</button>
    <button class="confirm" class:destructive onclick={onconfirm}>{action}</button>
  </div>
</Sheet>

<style>
  .message {
    margin: 0 0 var(--jf-space-s5);
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--jf-space-s2);
  }
  button {
    min-height: 44px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .cancel {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .confirm {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .confirm.destructive {
    background: var(--jf-danger);
  }
</style>
