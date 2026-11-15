<script lang="ts">
  // Settings › Library (docs/ui.md): Music Folders (each with its access and Remove Folder…, which
  // asks first), Add Folder…, then Rescan Library with the scan's progress and the last scan.
  // Browsers do not tell pages when files change, so there is no Watch Folders here.
  import ConfirmSheet from '../components/ConfirmSheet.svelte';
  import Icon from '../components/Icon.svelte';
  import { useApp } from '../lib/app';
  import { t, tn } from '../lib/i18n/index.svelte';
  import type { FolderView } from '../lib/state/library.svelte';
  import { toast } from '../lib/state/toast.svelte';

  const app = useApp();
  const library = app.library;
  let removing = $state<FolderView | null>(null);
  let busy = $state(false);

  async function run(job: () => Promise<unknown>) {
    busy = true;
    try {
      await job();
    } catch (error) {
      toast(error instanceof Error ? error.message : String(error));
    } finally {
      busy = false;
    }
  }

  function lastScan(folder: FolderView): string {
    const scan = folder.lastScan;
    if (!scan) return t('Not read yet');
    const parts = [
      t('Added {n}', { n: scan.added }),
      t('updated {n}', { n: scan.updated }),
      t('removed {n}', { n: scan.removed }),
    ];
    if (scan.errors > 0) parts.push(tn('{n} file could not be read', '{n} files could not be read', scan.errors));
    return parts.join(', ');
  }
</script>

<section class="group" aria-labelledby="g-folders">
  <h2 id="g-folders" class="section">{t('Music Folders')}</h2>
  <p class="note">
    {t('Jellyfesh reads every audio file in these folders. What it read is kept; pick a folder again on each visit before its songs play.')}
  </p>
  <div class="card">
    {#if library.folders.length === 0}
      <p class="row muted">{t('No folders yet')}</p>
    {/if}
    {#each library.folders as folder (folder.id)}
      <div class="row folder">
        <Icon name="folder" />
        <span class="text">
          <span class="label">{folder.name}</span>
          <span class="help">{lastScan(folder)}</span>
        </span>
        {#if folder.access === 'pick'}
          <button class="button" disabled={busy} onclick={() => run(() => library.pickAgain(folder.id))}>{t('Pick Folder…')}</button>
        {/if}
        <button class="button quiet" disabled={busy} onclick={() => (removing = folder)}>{t('Remove Folder…')}</button>
      </div>
    {/each}
    <div class="row end">
      <button class="button" disabled={busy} onclick={() => run(() => library.addFolder())}><Icon name="folder-plus" size={18} />{t('Add Folder…')}</button>
    </div>
  </div>
</section>

<section class="group" aria-labelledby="g-rescan">
  <h2 id="g-rescan" class="section">{t('Rescan Library')}</h2>
  <div class="card">
    <div class="row">
      <span class="text">
        <span class="label">{t('Rescan Library')}</span>
        <span class="help">
          {#if library.progress}
            {t('Reading “{name}”: {seen} files checked, {changed} changed', {
              name: library.progress.folder,
              seen: library.progress.seen,
              changed: library.progress.changed,
            })}
          {:else}
            {t('Look for new, changed and removed files.')}
          {/if}
        </span>
      </span>
      <button class="button" disabled={busy || library.folders.length === 0} onclick={() => run(() => library.rescan())}>{t('Rescan Library')}</button>
    </div>
    <div class="row">
      <span class="text">
        <span class="label">{t('Size')}</span>
        <span class="help">
          {t('{songs} · {albums} · {artists}', {
            songs: tn('{n} song', '{n} songs', library.stats.tracks),
            albums: tn('{n} album', '{n} albums', library.stats.albums),
            artists: tn('{n} artist', '{n} artists', library.stats.artists),
          })}
        </span>
      </span>
    </div>
    {#if library.problem}
      <p class="row problem" role="alert"><Icon name="alert" size={18} />{library.problem}</p>
    {/if}
  </div>
</section>

{#if removing}
  {@const folder = removing}
  <ConfirmSheet
    title={t('Remove “{name}” from the library?', { name: folder.name })}
    message={t('Its songs leave the library. The files stay where they are.')}
    action={t('Remove Folder')}
    onconfirm={() => {
      const id = folder.id;
      removing = null;
      void run(() => library.removeFolder(id));
    }}
    oncancel={() => (removing = null)}
  />
{/if}

<style>
  .group {
    margin-bottom: var(--jf-space-s8);
  }
  .card {
    padding: 0 var(--jf-space-s4);
    border-radius: var(--jf-radius-r-xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--jf-space-s3);
    min-height: 60px;
    margin: 0;
    padding: var(--jf-space-s3) 0;
  }
  .row + .row {
    border-top: 1px solid var(--jf-line-soft);
  }
  .row.end {
    justify-content: flex-end;
  }
  .folder :global(svg) {
    flex: none;
    color: var(--jf-accent-text);
  }
  .text {
    flex: 1 1 200px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .label {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .help,
  .note,
  .muted {
    font-size: 13px;
    line-height: 1.45;
    color: var(--jf-ink-50);
  }
  .note {
    margin: calc(-1 * var(--jf-space-s2)) 0 var(--jf-space-s3);
  }
  .button {
    display: inline-flex;
    align-items: center;
    gap: var(--jf-space-s2);
    min-height: 36px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .button.quiet {
    background: var(--jf-fill);
    color: var(--jf-ink-90);
  }
  .button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .problem {
    color: var(--jf-ink-70);
  }
  .problem :global(svg) {
    color: var(--jf-warning);
  }
</style>
