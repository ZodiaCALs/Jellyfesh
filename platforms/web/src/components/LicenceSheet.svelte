<script lang="ts">
  // Enter Licence Key (docs/ui.md "The Pro sheet"): asks for the key ("JFP1-…"; Cancel, Activate)
  // and says why one is refused, in the apps' words. jf-web checks it offline.
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { licenceErrorText } from '../lib/state/pro.svelte';
  import { toast } from '../lib/state/toast.svelte';
  import Sheet from './Sheet.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const app = useApp();
  let key = $state('');
  let error = $state<string | null>(null);

  function activate(event: SubmitEvent) {
    event.preventDefault();
    const refused = app.pro.activate(key);
    if (refused) {
      error = licenceErrorText(refused, t);
      return;
    }
    toast(t('Jellyfesh Pro is on.'));
    onclose();
  }
</script>

<Sheet title={t('Enter Licence Key')} {onclose}>
  <form onsubmit={activate}>
    <label class="field">
      <span>{t('Licence Key')}</span>
      <input
        bind:value={key}
        placeholder="JFP1-…"
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        aria-invalid={error !== null}
        aria-describedby={error ? 'licence-error' : undefined}
      />
    </label>
    {#if error}<p class="error" id="licence-error" role="alert">{error}</p>{/if}
    <div class="actions">
      <button type="button" class="secondary" onclick={onclose}>{t('Cancel')}</button>
      <button type="submit" class="primary" disabled={key.trim() === ''}>{t('Activate')}</button>
    </div>
  </form>
</Sheet>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s3);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-weight: 600;
  }
  input {
    min-height: 44px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-family: var(--jf-type-mono, ui-monospace, monospace);
    letter-spacing: 0.02em;
  }
  input:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .error {
    margin: 0;
    color: var(--jf-danger, var(--jf-ink-90));
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--jf-space-s2);
  }
  .primary,
  .secondary {
    min-height: 40px;
    padding: 0 var(--jf-space-s5);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .primary {
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
</style>
