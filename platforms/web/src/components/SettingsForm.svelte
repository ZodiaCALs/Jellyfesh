<script lang="ts">
  // A plugin's settings, built from its JSON Schema (as the apps do): the required ones first,
  // the others behind "More Settings". Schema profile 1.2 (docs/plugins/authoring.md) adds choices
  // with titles (`oneOf`), colours (`format: color`, a colour well), sliders (`minimum` and
  // `maximum`, `multipleOf` the step), ordered checklists (an array of known items: the value lists
  // the shown items in order) and group headings (`x-jf-group`). The core checks the values
  // (validate_settings).
  import type { SettingSpec, SettingsSchema } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

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
  const grouped = $derived(properties.some(([, spec]) => spec['x-jf-group']));
  // With groups (profile 1.2), every setting shows under its heading; else the optional ones wait
  // behind More Settings.
  const main = $derived(grouped ? properties : properties.filter(([name]) => required.has(name)));
  const extra = $derived(grouped ? [] : properties.filter(([name]) => !required.has(name)));
  const prefix = `f-${Math.random().toString(36).slice(2, 8)}`;

  /** Ordered checklists' values as they are being edited: the checked items, in the shown order. */
  let lists = $state<Record<string, { order: string[]; on: Set<string> }>>({});
  $effect.pre(() => {
    const next: Record<string, { order: string[]; on: Set<string> }> = {};
    for (const [name, spec] of properties) {
      if (spec.type !== 'array') continue;
      const known = choicesOf(spec.items ?? {}).map(([value]) => String(value));
      const current = (values[name] ?? spec.default ?? []) as string[];
      const order = [...current.filter((v) => known.includes(v)), ...known.filter((v) => !current.includes(v))];
      next[name] = { order, on: new Set(current) };
    }
    lists = next;
  });

  function choicesOf(spec: { enum?: string[]; oneOf?: { const: string | number; title?: string }[] }): [string | number, string][] {
    if (spec.oneOf) return spec.oneOf.map((c) => [c.const, c.title ?? String(c.const)]);
    return (spec.enum ?? []).map((v) => [v, v]);
  }

  function slider(spec: SettingSpec): boolean {
    return (spec.type === 'number' || spec.type === 'integer') && spec.minimum !== undefined && spec.maximum !== undefined;
  }

  function move(name: string, index: number, by: number) {
    const list = lists[name];
    const to = index + by;
    if (!list || to < 0 || to >= list.order.length) return;
    const order = [...list.order];
    [order[index], order[to]] = [order[to], order[index]];
    lists[name] = { ...list, order };
  }

  function toggle(name: string, value: string, on: boolean) {
    const list = lists[name];
    if (!list) return;
    const set = new Set(list.on);
    if (on) set.add(value);
    else set.delete(value);
    lists[name] = { ...list, on: set };
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const form = new FormData(event.currentTarget as HTMLFormElement);
    const out: Record<string, unknown> = {};
    for (const [name, spec] of properties) {
      if (spec.type === 'boolean') {
        out[name] = form.get(name) === 'on';
        continue;
      }
      if (spec.type === 'array') {
        const list = lists[name];
        if (list) out[name] = list.order.filter((v) => list.on.has(v));
        continue;
      }
      const raw = form.get(name);
      if (raw === null || raw === '') continue;
      const text = String(raw).trim();
      const numeric = spec.type === 'integer' ? Number.parseInt(text, 10) : spec.type === 'number' ? Number(text) : null;
      out[name] = numeric ?? (spec.oneOf && typeof spec.oneOf[0]?.const === 'number' ? Number(text) : text);
    }
    onsubmit(out);
  }
</script>

{#snippet field(name: string, spec: SettingSpec)}
  {@const id = `${prefix}-${name}`}
  {@const value = values[name] ?? spec.default}
  <div class="field" class:check={spec.type === 'boolean'}>
    {#if spec.type === 'boolean'}
      <input {id} {name} type="checkbox" checked={value === true} />
      <label for={id}>{spec.title ?? name}</label>
    {:else if spec.type === 'array'}
      <span class="label" id="{id}-label">{spec.title ?? name}</span>
      <ul class="checklist" aria-labelledby="{id}-label">
        {#each lists[name]?.order ?? [] as item, index (item)}
          {@const title = choicesOf(spec.items ?? {}).find(([v]) => String(v) === item)?.[1] ?? item}
          <li>
            <label class="item">
              <input type="checkbox" checked={lists[name]?.on.has(item)} onchange={(e) => toggle(name, item, (e.currentTarget as HTMLInputElement).checked)} />
              {title}
            </label>
            <button type="button" class="step" aria-label={t('Move {item} Up', { item: title })} disabled={index === 0} onclick={() => move(name, index, -1)}>
              <Icon name="back" size={14} />
            </button>
            <button
              type="button"
              class="step down"
              aria-label={t('Move {item} Down', { item: title })}
              disabled={index === (lists[name]?.order.length ?? 0) - 1}
              onclick={() => move(name, index, 1)}
            >
              <Icon name="back" size={14} />
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <label for={id}>{spec.title ?? name}</label>
      {#if spec.enum || spec.oneOf}
        <select {id} {name} value={String(value ?? '')}>
          {#each choicesOf(spec) as [option, title] (option)}
            <option value={String(option)}>{title}</option>
          {/each}
        </select>
      {:else if spec.format === 'color'}
        <input {id} {name} type="color" class="colour" value={typeof value === 'string' ? value : '#000000'} />
      {:else if slider(spec)}
        <div class="slider">
          <input
            {id}
            {name}
            type="range"
            min={spec.minimum}
            max={spec.maximum}
            step={spec.multipleOf ?? (spec.type === 'integer' ? 1 : 0.01)}
            value={Number(value ?? spec.minimum)}
            oninput={(e) => {
              const output = (e.currentTarget as HTMLInputElement).nextElementSibling;
              if (output) output.textContent = (e.currentTarget as HTMLInputElement).value;
            }}
          />
          <output for={id}>{String(value ?? spec.minimum)}</output>
        </div>
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
  {#each main as [name, spec], index (name)}
    {#if spec['x-jf-group'] && spec['x-jf-group'] !== main[index - 1]?.[1]['x-jf-group']}
      <h3 class="group">{spec['x-jf-group']}</h3>
    {/if}
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
  .group {
    margin: var(--jf-space-s2) 0 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 700;
    color: var(--jf-ink-70);
    text-transform: uppercase;
    letter-spacing: 0.04em;
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
  label,
  .label {
    font-weight: 600;
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  input:not([type='checkbox']):not([type='range']):not([type='color']),
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
  .colour {
    width: 64px;
    height: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: none;
  }
  .slider {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
  }
  .slider input {
    flex: 1;
    accent-color: var(--jf-accent);
  }
  .slider output {
    min-width: 3em;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-70);
  }
  .checklist {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .checklist li {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
  }
  .checklist .item {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    font-weight: 500;
  }
  .step {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .step :global(svg) {
    rotate: 90deg;
  }
  .step.down :global(svg) {
    rotate: -90deg;
  }
  .step:disabled {
    opacity: 0.35;
    cursor: default;
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
    font-size: calc(12px * var(--jf-text-scale, 1));
    line-height: 1.45;
    color: var(--jf-ink-70);
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
