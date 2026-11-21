<script lang="ts">
  // Settings › Extensions (docs/ui.md; ADR-0019, ADR-0020, ADR-0021): Jellyfesh Pro, then the
  // extensions installed (AI Lyrics, Customize and others), each as the sources are, with Install
  // from File… in the heading. Jellyfesh Web is sold in no store: Pro comes from a licence key (or
  // a Pro Preview build).
  import ConfirmSheet from '../components/ConfirmSheet.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { licenceErrorText } from '../lib/state/pro.svelte';
  import SourcesSettings from './SourcesSettings.svelte';

  const app = useApp();
  const pro = app.pro;
  const site = app.jf.website();
  let removing = $state(false);
  const licence = $derived(typeof pro.snapshot.source === 'object' ? pro.snapshot.source.Licence : null);
</script>

<section class="group" aria-labelledby="g-pro">
  <h2 id="g-pro" class="section">{t('Jellyfesh Pro')}</h2>
  <div class="card">
    {#if pro.snapshot.preview}
      <p class="status">{t('Pro features are free during the public beta.')}</p>
    {:else if licence}
      <p class="status">{t('Jellyfesh Pro · Licence …{suffix}', { suffix: licence.id_suffix })}</p>
      <div class="actions"><button class="secondary danger" onclick={() => (removing = true)}>{t('Remove Licence…')}</button></div>
    {:else}
      <p class="status">{t('AI Lyrics and Customize are part of Jellyfesh Pro.')}</p>
      {#if pro.snapshot.licence_error}
        <p class="problem" role="alert">{licenceErrorText(pro.snapshot.licence_error, t)}</p>
      {/if}
      <div class="actions"><button class="primary" onclick={() => (pro.entering = true)}>{t('Enter Licence Key…')}</button></div>
    {/if}
    {#if site}
      <p class="links">
        <a href="{site}legal/eula.html" target="_blank" rel="noopener">{t('Licence Agreement')}</a>
        <a href="{site}legal/privacy.html" target="_blank" rel="noopener">{t('Privacy Policy')}</a>
        <a href="{site}legal/terms-of-sale.html" target="_blank" rel="noopener">{t('Terms of Sale')}</a>
      </p>
    {/if}
  </div>
</section>

<SourcesSettings extensions />

{#if app.aiLyrics.consents.length > 0}
  <section class="group" aria-labelledby="g-sending">
    <h2 id="g-sending" class="section">{t('Sending Audio')}</h2>
    <ul class="consents">
      {#each app.aiLyrics.consents as consent (consent.plugin + consent.origin)}
        <li>
          <span>{consent.origin}</span>
          <button class="secondary" onclick={() => app.aiLyrics.revoke(consent)}>{t('Stop Allowing')}</button>
        </li>
      {/each}
    </ul>
  </section>
{/if}

{#if removing}
  <ConfirmSheet
    title={t('Remove the Licence Key?')}
    message={t('Pro features stop until you enter a key or buy Jellyfesh Pro.')}
    action={t('Remove Licence')}
    onconfirm={() => {
      pro.removeLicence();
      removing = false;
    }}
    oncancel={() => (removing = false)}
  />
{/if}

<style>
  .group {
    margin-bottom: var(--jf-space-s5);
    padding: var(--jf-space-s5) var(--jf-space-s6) var(--jf-space-s4);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s3);
  }
  .status,
  .problem,
  .links {
    margin: 0;
  }
  .problem {
    color: var(--jf-danger, var(--jf-ink-90));
  }
  .actions {
    display: flex;
    gap: var(--jf-space-s2);
  }
  .primary,
  .secondary {
    min-height: 40px;
    padding: 0 var(--jf-space-s4);
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
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .secondary.danger {
    color: var(--jf-danger, var(--jf-ink-90));
  }
  .primary:focus-visible,
  .secondary:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s4);
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .links a {
    color: var(--jf-accent-text);
  }
  .consents {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .consents li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    overflow-wrap: anywhere;
  }
</style>
