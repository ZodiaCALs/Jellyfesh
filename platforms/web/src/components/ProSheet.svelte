<script lang="ts">
  // The Pro sheet (docs/ui.md "AI Lyrics, Customize and Pro"): opens from any Pro action while
  // Pro is inactive. Jellyfesh Web is sold in no store, so it offers Enter Licence Key… and Not
  // Now, then the Licence Agreement and the Privacy Policy where the build knows the website.
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import Sheet from './Sheet.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const app = useApp();
  const site = app.jf.website();
</script>

<Sheet title={t('Jellyfesh Pro')} {onclose}>
  <p class="lede">{t('AI Lyrics makes synced lyrics for any song. Customize makes Jellyfesh yours: corners, glass, type, lyrics and layout.')}</p>
  <div class="actions">
    <button
      class="primary"
      onclick={() => {
        app.pro.sheet = false;
        app.pro.entering = true;
      }}>{t('Enter Licence Key…')}</button
    >
    <button class="secondary" onclick={onclose}>{t('Not Now')}</button>
  </div>
  {#if site}
    <p class="links">
      <a href="{site}legal/eula.html" target="_blank" rel="noopener">{t('Licence Agreement')}</a>
      <a href="{site}legal/privacy.html" target="_blank" rel="noopener">{t('Privacy Policy')}</a>
    </p>
  {/if}
</Sheet>

<style>
  .lede {
    margin: 0 0 var(--jf-space-s5);
    color: var(--jf-ink-90);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s2);
  }
  .primary,
  .secondary {
    min-height: 44px;
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
  .secondary {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .primary:focus-visible,
  .secondary:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .links {
    display: flex;
    gap: var(--jf-space-s4);
    margin: var(--jf-space-s5) 0 0;
    font-size: calc(14px * var(--jf-text-scale, 1));
  }
  .links a {
    color: var(--jf-accent-text);
  }
</style>
