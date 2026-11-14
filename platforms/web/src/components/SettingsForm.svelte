<script lang="ts">
  // A plugin's settings, built from its JSON Schema (as the apps do): the required ones first,
  // the others behind "More Settings". The core checks the values (validate_settings).
  import type { SettingsSchema } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';

  let {
    schema,
    values,
    submitLabel,
    busy = false,
    error = null,
    placeholders = {},
    onsubmit,
  }: {
    schema: SettingsSchema;
    values: Record<string, unknown>;
    submitLabel: string;
    busy?: boolean;
    error?: string | null;
    /** Examples to show in empty text fields, by setting name (a service's example address). */
    placeholders?: Record<string, string>;
    onsubmit: (values: Record<string, unknown>) => void;
  } = $props();

  const properties = $derived(Object.entries(schema.properties ?? {}));
  const required = $derived(new Set(schema.required ?? []));
  const main = $derived(properties.filter(([name]) => required.has(name)));
  const extra = $derived(properties.filter(([name]) => !required.has(name)));
  const prefix = `f-${Math.random().toString(36).slice(2, 8)}`;

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const form = new FormData(event.currentTarget as HTMLFormElement);
    const out: Record<string, unknown> = {};
    for (const [name, spec] of properties) {
      if (spec.type === 'boolean') {
        out[name] = form.get(name) === 'on';
        continue;
      }
      const raw = form.get(name);
      if (raw === null || raw === '') continue;
      const text = String(raw).trim();
      out[name] = spec.type === 'integer' ? Number.parseInt(text, 10) : spec.type === 'number' ? Number(text) : text;
    }
    onsubmit(out);
  }
</script>

{#snippet field(name: string, spec: NonNullable<SettingsSchema['properties']>[string])}
  {@const id = `${prefix}-${name}`}
  {@const value = values[name] ?? spec.default}
  <div class="field" class:check={spec.type === 'boolean'}>
    {#if spec.type === 'boolean'}
      <input {id} {name} type="checkbox" checked={value === true} />
      <label for={id}>{spec.title ?? name}</label>
    {:else}
      <label for={id}>{spec.title ?? name}</label>
      {#if spec.enum}
        <select {id} {name} value={String(value ?? '')}>
          {#each spec.enum as option (option)}
            <option value={option}>{option}</option>
          {/each}
        </select>
      {:else}
        <input
          {id}
          {name}
          type={spec.format === 'uri' ? 'url' : spec.type === 'string' ? 'text' : 'number'}
          inputmode={spec.format === 'uri' ? 'url' : undefined}
          value={value === undefined || value === null ? '' : String(value)}
          required={required.has(name)}
          minlength={spec.minLength}
          maxlength={spec.maxLength}
          min={spec.minimum}
          max={spec.maximum}
          placeholder={placeholders[name] ?? (spec.format === 'uri' ? 'https://' : undefined)}
          autocomplete={spec.format === 'uri' ? 'url' : 'off'}
          autocapitalize="off"
          spellcheck="false"
          aria-describedby={spec.description ? `${id}-help` : undefined}
        />
      {/if}
    {/if}
    {#if spec.description}<p class="help" id="{id}-help">{spec.description}</p>{/if}
  </div>
{/snippet}

<form onsubmit={submit} novalidate={false}>
  {#each main as [name, spec] (name)}
    {@render field(name, spec)}
  {/each}
  {#if extra.length}
    <details>
      <summary>{t('More Settings')}</summary>
      {#each extra as [name, spec] (name)}
        {@render field(name, spec)}
      {/each}
    </details>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <button type="submit" class="primary" disabled={busy}>{busy ? t('Checking…') : submitLabel}</button>
</form>

<style>
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
  .field.check {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
  }
  label {
    font-weight: 600;
    font-size: 14px;
  }
  input:not([type='checkbox']),
  select {
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
  select:focus-visible,
  button:focus-visible,
  summary:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .help {
    flex-basis: 100%;
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--jf-ink-50);
  }
  details {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s4);
  }
  details[open] summary {
    margin-bottom: var(--jf-space-s4);
  }
  summary {
    cursor: pointer;
    font-weight: 600;
    color: var(--jf-accent-text);
    border-radius: var(--jf-radius-r-sm);
  }
  .error {
    margin: 0;
    padding: var(--jf-space-s3);
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-danger);
    line-height: 1.45;
  }
  .primary {
    align-self: flex-end;
    min-height: 44px;
    padding: 0 var(--jf-space-s6);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
