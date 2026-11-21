<script lang="ts" generics="S extends string">
  // A list's sort order and direction (docs/ui.md: the Albums and Songs sort menus).
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

  let {
    options,
    sort = $bindable(),
    descending = $bindable(),
  }: { options: [S, string][]; sort: S; descending: boolean } = $props();
  const id = `sort-${Math.random().toString(36).slice(2, 8)}`;
</script>

<div class="sort">
  <label for={id}>{t('Sort')}</label>
  <select {id} bind:value={sort}>
    {#each options as [value, label] (value)}
      <option {value}>{label}</option>
    {/each}
  </select>
  <button class="direction" aria-pressed={descending} title={t('Descending')} onclick={() => (descending = !descending)}>
    <Icon name="sort" size={16} />{t('Descending')}
  </button>
</div>

<style>
  .sort {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    font-size: calc(14px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  select {
    min-height: 36px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
  }
  .direction {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    cursor: pointer;
  }
  .direction[aria-pressed='true'] {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
  }
  select:focus-visible,
  .direction:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
