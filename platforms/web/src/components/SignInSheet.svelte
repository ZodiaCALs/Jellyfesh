<script lang="ts">
  // Signing in to a source: the server address, then the flow the plugin offers (a form, a code
  // to approve elsewhere such as Jellyfin's Quick Connect, a QR code, or a page to open), each
  // step drawn from what the core's sign-in machine says. Passwords go to the plugin once and are
  // never stored.
  import { useApp } from '../lib/app';
  import type { AuthKind, AuthStep, FieldSpec } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import { explain, type PresetView, refusal, SignInFlow } from '../lib/state/sources.svelte';
  import Icon from './Icon.svelte';
  import SettingsForm from './SettingsForm.svelte';
  import Sheet from './Sheet.svelte';

  let {
    plugin,
    preset = null,
    onclose,
  }: {
    plugin: Plugin;
    /** The service chosen in Add a Source, for the title and the example address. */
    preset?: PresetView | null;
    onclose: () => void;
  } = $props();
  const app = useApp();
  let flow = $state<SignInFlow | null>(null);
  let loadError = $state<string | null>(null);

  $effect(() => {
    let live = true;
    plugin
      .verify()
      .then(() => {
        if (live) flow = new SignInFlow(app.jf, plugin, app.sources);
      })
      .catch(async (error) => {
        if (live) loadError = await explain(error, plugin);
      });
    return () => {
      live = false;
      flow?.stop();
    };
  });

  const KIND_LABELS: Record<AuthKind, string> = {
    Qr: 'Sign In with QR Code',
    Oauth: 'Sign In in Browser',
    CookieImport: 'Paste Cookies',
    PasswordExchange: 'Sign In',
    ApiKey: 'Enter API Key',
  };

  function qr(data: string): { path: string; size: number } | null {
    return JSON.parse(app.jf.qr_svg(data));
  }

  function submitFields(event: SubmitEvent, fields: FieldSpec[]) {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    const values = fields.map((f) => ({ name: f.name, value: String(data.get(f.name) ?? '') }));
    // The typed secrets leave the form at once.
    for (const input of form.querySelectorAll<HTMLInputElement>('input[type="password"]')) input.value = '';
    flow?.submit(values);
  }

  function inputType(field: FieldSpec): string {
    if (field.secret) return 'password';
    return { Email: 'email', Url: 'url', Number: 'text', Text: 'text', CookieHeader: 'text' }[field.kind] ?? 'text';
  }

  function autocomplete(field: FieldSpec): AutoFill {
    if (field.secret) return field.name.includes('key') ? 'off' : 'current-password';
    if (/user|login|name/.test(field.name)) return 'username';
    if (field.kind === 'Email') return 'email';
    return 'off';
  }

  function waiting(step: AuthStep): string | null {
    return 'Waiting' in step ? step.Waiting.message : null;
  }
</script>

<Sheet title={preset ? t('Connect to {name}', { name: preset.name }) : t('Sign In to {name}', { name: plugin.name })} {onclose}>
  {#if loadError}
    <p class="error" role="alert">{loadError}</p>
  {:else if !flow}
    <p class="muted">{t('Loading…')}</p>
  {:else}
    {@const view = flow.view}
    {#if view.stage === 'refused'}
      <div class="step">
        <p class="refused" role="alert"><Icon name="alert" size={20} /><span>{refusal(view.service)}</span></p>
        <button class="primary" onclick={onclose}>{t('Close')}</button>
      </div>
    {:else if view.stage === 'settings'}
      <SettingsForm
        schema={plugin.info?.settings_schema ?? {}}
        values={plugin.state.settings ?? {}}
        submitLabel={t('Continue')}
        busy={view.busy}
        error={view.error}
        placeholders={preset?.example ? { server_url: preset.example } : {}}
        onsubmit={(values) => flow?.saveSettings(values)}
      />
    {:else if view.stage === 'choose'}
      <p class="server">
        <Icon name="globe" size={18} /><span>{app.sources.list.find((s) => s.id === plugin.id)?.server ?? ''}</span>
        <button class="link" onclick={() => flow?.editSettings()}>{t('Change')}</button>
      </p>
      <div class="kinds">
        {#each flow.kinds as kind (kind)}
          <button class="kind" onclick={() => flow?.start(kind)}>
            <Icon name={kind === 'Qr' ? 'qr' : kind === 'ApiKey' ? 'key' : 'artist'} />{t(KIND_LABELS[kind])}
          </button>
        {/each}
      </div>
    {:else if view.stage === 'step'}
      {@const step = view.step}
      <div class="step" aria-live="polite">
        {#if 'AskFields' in step}
          <form onsubmit={(event) => submitFields(event, step.AskFields.fields)}>
            {#each step.AskFields.fields as field, index (field.name)}
              <div class="field">
                <label for="signin-{field.name}">{field.label}</label>
                <input
                  id="signin-{field.name}"
                  name={field.name}
                  type={inputType(field)}
                  inputmode={field.kind === 'Number' ? 'numeric' : undefined}
                  required={field.required}
                  placeholder={field.placeholder ?? undefined}
                  autocomplete={autocomplete(field)}
                  autocapitalize="off"
                  spellcheck="false"
                  aria-describedby={field.help ? `signin-${field.name}-help` : undefined}
                  {...{ autofocus: index === 0 }}
                />
                {#if field.help}<p class="help" id="signin-{field.name}-help">{field.help}</p>{/if}
              </div>
            {/each}
            {#if view.error}<p class="error" role="alert">{view.error}</p>{/if}
            <button type="submit" class="primary" disabled={view.busy}>{view.busy ? t('Signing in…') : t('Sign In')}</button>
          </form>
        {:else if 'ShowCode' in step}
          <p class="code" aria-label={t('Code {code}', { code: step.ShowCode.code.replace(/\s/g, '').split('').join(' ') })}>{step.ShowCode.code}</p>
          <p class="hint">{step.ShowCode.hint}</p>
          <p class="muted">{t('Waiting for approval…')}</p>
        {:else if 'ShowQr' in step}
          {@const code = qr(step.ShowQr.data)}
          {#if code}
            <svg class="qr" viewBox="-4 -4 {code.size + 8} {code.size + 8}" role="img" aria-label={t('Sign-in QR code')}>
              <rect x="-4" y="-4" width={code.size + 8} height={code.size + 8} fill="#fff" />
              <path d={code.path} fill="#000" />
            </svg>
          {/if}
          <p class="hint">{step.ShowQr.hint ?? t('Scan the code with the {name} app on your phone.', { name: plugin.name })}</p>
        {:else if 'OpenUrl' in step}
          <p class="hint">{t('Continue in your browser. Come back here when you are done.')}</p>
          <a class="primary as-button" href={step.OpenUrl.url} target="_blank" rel="noopener noreferrer">{t('Open Browser Again')}</a>
        {:else if 'Failed' in step}
          <p class="error" role="alert">{step.Failed.message}</p>
        {:else}
          <p class="muted">{waiting(step) || t('Signing in…')}</p>
        {/if}
      </div>
    {:else if view.stage === 'done'}
      <div class="step">
        <p class="done"><Icon name="check" size={28} />{t('Signed in as {account}.', { account: view.account })}</p>
        <button class="primary" onclick={onclose}>{t('Done')}</button>
      </div>
    {:else if view.stage === 'failed'}
      <div class="step">
        <p class="error" role="alert">{view.message}</p>
        <button class="primary" onclick={() => (flow?.kind ? flow.start(flow.kind) : flow?.editSettings())}>{t('Try Again')}</button>
      </div>
    {/if}
  {/if}
</Sheet>

<style>
  .muted {
    color: var(--jf-ink-70);
  }
  .server {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    margin: 0 0 var(--jf-space-s4);
    color: var(--jf-ink-70);
    overflow-wrap: anywhere;
  }
  .server span {
    flex: 1;
  }
  .link {
    border: 0;
    background: none;
    color: var(--jf-accent-text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .kinds {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s2);
  }
  .kind {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-height: 52px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .kind:hover {
    background: var(--jf-fill-hover);
  }
  .step,
  form {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s4);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label {
    font-weight: 600;
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  input {
    min-height: 44px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    box-shadow: inset 0 0 0 1px var(--jf-line);
  }
  input:focus-visible,
  button:focus-visible,
  a:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .help {
    margin: 0;
    font-size: calc(12px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .code {
    margin: var(--jf-space-s4) 0 0;
    font-size: calc(40px * var(--jf-text-scale, 1));
    font-weight: 700;
    letter-spacing: 0.18em;
    text-align: center;
    font-variant-numeric: tabular-nums;
    user-select: all;
  }
  .hint {
    margin: 0;
    text-align: center;
    color: var(--jf-ink-70);
    line-height: 1.5;
  }
  .qr {
    width: min(240px, 70vw);
    align-self: center;
    border-radius: var(--jf-radius-r-md);
  }
  .error {
    margin: 0;
    padding: var(--jf-space-s3);
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-danger);
    line-height: 1.45;
  }
  .refused {
    display: flex;
    gap: var(--jf-space-s3);
    align-items: flex-start;
    margin: 0;
    padding: var(--jf-space-s4);
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    line-height: 1.5;
  }
  .refused :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--jf-warning);
  }
  .done {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    margin: 0;
    font-size: calc(18px * var(--jf-text-scale, 1));
    font-weight: 600;
  }
  .done :global(svg) {
    color: var(--jf-success);
  }
  .primary {
    align-self: flex-end;
    display: inline-grid;
    place-items: center;
    min-height: 44px;
    padding: 0 var(--jf-space-s6);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    font: inherit;
    font-weight: 600;
    text-decoration: none;
    cursor: pointer;
  }
  .as-button {
    align-self: center;
  }
  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
