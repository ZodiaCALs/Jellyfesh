<script lang="ts">
  // Sending audio asks once per plugin and origin before the first online job (docs/ui.md "AI
  // Lyrics, Customize and Pro"): "Send This Song to {origin}?", Cancel, Allow.
  import { useApp } from '../lib/app';
  import type { Row } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import ConfirmSheet from './ConfirmSheet.svelte';

  let { row, origin }: { row: Row; origin: string } = $props();
  const app = useApp();
</script>

<ConfirmSheet
  title={t('Send This Song to {origin}?', { origin })}
  message={t('AI Lyrics will send the song’s audio to {origin} to make its lyrics. The provider’s terms and privacy policy apply, and it may charge your account.', { origin })}
  action={t('Allow')}
  destructive={false}
  onconfirm={() => {
    app.aiLyrics.asking = null;
    app.aiLyrics.allow(origin);
    void app.aiLyrics.make(row);
  }}
  oncancel={() => (app.aiLyrics.asking = null)}
/>
