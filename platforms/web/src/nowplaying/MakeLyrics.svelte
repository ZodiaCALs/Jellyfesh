<script lang="ts">
  // AI Lyrics in the Lyrics pane (docs/ui.md "AI Lyrics, Customize and Pro"): Create Synced Lyrics
  // (or Time These Lyrics under plain lyrics) when AI Lyrics can make them, or Pro is off (which
  // opens the Pro sheet); while the song has a job, its stage with Cancel; what is needed first;
  // and a failed job with Try Again. jf-web decides what is needed (src/lib/state/aiLyrics).
  import { useApp } from '../lib/app';
  import type { Row } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import { songKey } from '../lib/state/aiLyrics.svelte';

  let { row, text = null }: { row: Row; text?: string | null } = $props();
  const app = useApp();
  const ai = app.aiLyrics;
  const need = $derived(ai.need);
  const job = $derived(ai.job && ai.job.key === songKey(row) ? ai.job : null);
  const offered = $derived(ai.plugin() !== undefined && (need === 'Nothing' || need === 'Pro' || (typeof need === 'object' && 'Consent' in need)));
  const label = $derived(text ? t('Time These Lyrics') : t('Create Synced Lyrics'));
</script>

<div class="make">
  {#if job && job.state.stage !== 'failed'}
    <p class="stage" role="status">
      {job.state.stage === 'reading' ? t('Reading Audio…') : t('Sending to {origin}…', { origin: job.state.origin })}
    </p>
    <div class="bar" aria-hidden="true"><span></span></div>
    <button class="secondary" onclick={() => ai.cancel()}>{t('Cancel')}</button>
  {:else if job && job.state.stage === 'failed'}
    <p class="problem" role="alert">{t('Couldn’t make lyrics: {message}', { message: job.state.message })}</p>
    <button
      class="secondary"
      onclick={() => {
        ai.clearFailed();
        void ai.make(row, text);
      }}>{t('Try Again')}</button
    >
  {:else if need === 'SignIn' && ai.plugin()}
    <p>{t('Add your API key to make lyrics online.')}</p>
    <button class="secondary" onclick={() => app.nav.openSettings('extensions')}>{t('Open AI Lyrics Settings')}</button>
  {:else if offered}
    <button class="primary" onclick={() => void ai.make(row, text)}>{label}</button>
  {/if}
</div>

<style>
  .make {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--jf-space-s3);
    margin-top: var(--jf-space-s4);
  }
  p {
    margin: 0;
  }
  .stage {
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .problem {
    color: var(--jf-danger, var(--jf-ink-90));
  }
  /* An indeterminate bar: the online mode reports no fraction. */
  .bar {
    position: relative;
    width: min(280px, 100%);
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--jf-fill);
  }
  .bar span {
    position: absolute;
    inset: 0;
    width: 40%;
    border-radius: 2px;
    background: var(--jf-accent);
    animation: jf-indeterminate 1.4s var(--jf-motion-ease-fluid, ease-in-out) infinite;
  }
  :global(:root[data-motion='reduced']) .bar span {
    animation: none;
    width: 100%;
    opacity: 0.5;
  }
  @keyframes -global-jf-indeterminate {
    from {
      translate: -100% 0;
    }
    to {
      translate: 250% 0;
    }
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
  .primary:focus-visible,
  .secondary:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
</style>
