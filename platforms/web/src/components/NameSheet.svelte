<script lang="ts">
  // New Playlist…, Rename… and Save as Playlist… ask for the name first (docs/ui.md): one field,
  // then Cancel and the action. The core checks the name (trimmed, not empty) and says why not.
  import { refusalText } from '../lib/state/collections.svelte';
  import type { Naming } from '../lib/state/sheets.svelte';
  import { t } from '../lib/i18n/index.svelte';
  import Sheet from './Sheet.svelte';

  let { naming, onclose }: { naming: Naming; onclose: () => void } = $props();
  // The sheet is made for one naming and closed after it.
  // svelte-ignore state_referenced_locally
  let name = $state(naming.initial);
  let problem = $state<string | null>(null);
  let field: HTMLInputElement | undefined = $state();

  $effect(() => {
    field?.focus();
    field?.select();
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    try {
      naming.submit(name);
    } catch (error) {
      problem = refusalText(error);
      field?.focus();
      return;
    }
    onclose();
  }
</script>

<Sheet title={naming.title} {onclose}>
  <form onsubmit={submit}>
    <label class="field">
      <span>{t('Name')}</span>
      <input
        bind:this={field}
        bind:value={name}
        type="text"
        autocomplete="off"
        spellcheck="false"
        aria-invalid={problem !== null}
        aria-describedby={problem ? 'name-problem' : undefined}
        oninput={() => (problem = null)}
      />
    </label>
    {#if problem}<p class="problem" id="name-problem" role="alert">{problem}</p>{/if}
    <div class="actions">
      <button type="button" class="cancel" onclick={onclose}>{t('Cancel')}</button>
      <button type="submit" class="confirm">{naming.action}</button>
    </div>
  </form>
</Sheet>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s2);
    font-weight: 600;
  }
  input {
    min-height: 44px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 400;
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  input:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .problem {
    margin: var(--jf-space-s2) 0 0;
    color: var(--jf-ink-70);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--jf-space-s2);
    margin-top: var(--jf-space-s5);
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
</style>
