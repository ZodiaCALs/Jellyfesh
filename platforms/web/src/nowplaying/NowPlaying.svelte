<script lang="ts">
  // Now Playing, in the person's backdrop style: Artwork (the core's baked backdrop of the cover,
  // tone-mapped for the appearance) or Misty (the Misty field in the theme's hue). A veil bar with
  // the segmented control (Lyrics, Queue, Info); the player on one side and the chosen pane on the
  // other, or stacked on a phone. After Apple Music: the artwork scales to 0.88 while paused and
  // springs back when playing, the backdrop crossfades between songs, and the whole view opens
  // with Deeper and closes with Back (src/lib/motion.ts).
  import Art from '../components/Art.svelte';
  import Icon from '../components/Icon.svelte';
  import SeekBar from '../components/SeekBar.svelte';
  import Transport from '../components/Transport.svelte';
  import Volume from '../components/Volume.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { back, deeper } from '../lib/motion';
  import type { Pane } from '../lib/state/nav.svelte';
  import InfoPane from './InfoPane.svelte';
  import LyricsPane from './LyricsPane.svelte';
  import QueuePane from './QueuePane.svelte';

  const app = useApp();
  const { player, theme, nav } = app;
  const style = $derived(app.settings.value.style);
  const row = $derived(player.current);
  /** On a phone the pane replaces the artwork while one is chosen. */
  let phonePane = $state(false);

  // The backdrop crossfades over dur-4 when the artwork changes (instant with reduced motion).
  let layers = $state<{ id: number; url: string }[]>([]);
  let layerId = 0;
  $effect(() => {
    const url = style === 'artwork' ? theme.backdrop : null;
    const top = layers[layers.length - 1];
    if ((top?.url ?? null) === url) return;
    if (!url) {
      layers = [];
      return;
    }
    const id = ++layerId;
    layers = [...layers.slice(-1), { id, url }];
    const timer = setTimeout(() => (layers = layers.filter((l) => l.id >= id)), 700);
    return () => clearTimeout(timer);
  });

  const PANES: [Pane, string, string][] = [
    ['lyrics', 'Lyrics', 'lyrics'],
    ['queue', 'Queue', 'queue'],
    ['info', 'Info', 'info'],
  ];

  function choose(pane: Pane) {
    if (nav.pane === pane && phonePane) phonePane = false;
    else {
      nav.pane = pane;
      phonePane = true;
    }
  }

  let dialog: HTMLDivElement | undefined = $state();
  $effect(() => {
    dialog?.focus();
  });
</script>

<div
  class="np {style}"
  class:pane-open={phonePane}
  in:deeper
  out:back
  role="dialog"
  aria-modal="true"
  aria-label={t('Now Playing')}
  tabindex="-1"
  bind:this={dialog}
>
  <div class="backdrop" aria-hidden="true">
    <div class="field"></div>
    {#each layers as layer (layer.id)}
      <img class="layer" src={layer.url} alt="" />
    {/each}
  </div>

  <header class="bar">
    <button class="back" onclick={() => nav.closeNowPlaying()} aria-label={t('Back to Library')}>
      <Icon name="down" /><span class="back-label">{t('Library')}</span>
    </button>
    <p class="heading">{t('Now Playing')}</p>
    <div class="segmented" role="toolbar" aria-label={t('Now Playing views')}>
      {#each PANES as [id, label, icon] (id)}
        <button
          aria-pressed={nav.pane === id}
          class:on={nav.pane === id}
          class:phone-on={nav.pane === id && phonePane}
          aria-label={t(label)}
          title={t(label)}
          onclick={() => choose(id)}
        >
          <Icon name={icon} />
        </button>
      {/each}
    </div>
  </header>

  <main class="body">
    <section class="player" aria-label={t('Player')}>
      <div class="hero" class:paused={!player.playing}>
        {#if theme.artwork}
          <img class="cover" src={theme.artwork} alt={t('Artwork of {title}', { title: row?.title ?? '' })} />
        {:else}
          <Art kind="Album" size={400} radius="hero" />
        {/if}
      </div>
      <div class="titles">
        <h1 class="title">{row?.title ?? t('Nothing Playing')}</h1>
        <p class="artist">
          {#if row}{[row.artist ?? t('Unknown Artist'), row.album].filter(Boolean).join(' · ')}{:else}{t(
              'Pick a song, an album or a playlist.',
            )}{/if}
        </p>
      </div>
      {#if player.problem}
        <p class="problem" role="alert"><Icon name="alert" size={18} />{player.problem}</p>
      {/if}
      <SeekBar anchor={player.anchor} durationMs={player.durationMs} onseek={(ms) => player.seek(ms)} disabled={!row} />
      <Transport />
      <Volume />
    </section>

    <section class="pane" aria-label={t(PANES.find((p) => p[0] === nav.pane)?.[1] ?? 'Lyrics')}>
      {#if nav.pane === 'lyrics'}
        {#if player.lyrics.status === 'ready'}
          <LyricsPane lyrics={player.lyrics.lyrics} anchor={player.anchor} />
        {:else if player.lyrics.status === 'loading'}
          <p class="muted">{t('Loading…')}</p>
        {:else}
          <div class="no-lyrics">
            <p class="title-2">{t('No Lyrics')}</p>
            <p>
              {row?.kind === 'file'
                ? t('Jellyfesh shows lyrics from a .lrc, .ttml or .vtt file next to the song, or from its tags.')
                : t('The server has no lyrics for this song.')}
            </p>
          </div>
        {/if}
      {:else if nav.pane === 'queue'}
        <QueuePane />
      {:else}
        <InfoPane />
      {/if}
    </section>
  </main>
</div>

<style>
  .np {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    color: var(--jf-ink-90);
    background: var(--jf-sky);
    outline: none;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }
  .field {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(60% 55% at 10% 8%, var(--jf-aurora-1) 0%, transparent 70%),
      radial-gradient(55% 55% at 92% 92%, var(--jf-aurora-2) 0%, transparent 70%),
      radial-gradient(45% 50% at 4% 98%, var(--jf-aurora-3) 0%, transparent 70%),
      radial-gradient(40% 40% at 94% 6%, var(--jf-aurora-1) 0%, transparent 70%),
      var(--jf-sky);
  }
  .layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* The next song's backdrop crossfades in over the last (backdrop.crossfade: dur-4 Fluid). */
    animation: jf-fade-in var(--jf-motion-dur-4) var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) .layer {
    animation-duration: var(--jf-motion-dur-1);
  }
  .bar {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--jf-space-s3);
    min-height: 56px;
    padding: 6px max(var(--jf-space-s5), env(safe-area-inset-right)) 6px max(var(--jf-space-s5), env(safe-area-inset-left));
    padding-top: max(6px, env(safe-area-inset-top));
    background: var(--jf-tint-veil);
    backdrop-filter: blur(var(--jf-glass-veil-blur)) saturate(var(--jf-glass-sat));
    box-shadow: inset 0 -1px 0 var(--jf-glass-edge-soft);
  }
  :global([data-effects='reduced']) .bar {
    background: var(--jf-cloud);
    backdrop-filter: none;
  }
  .back {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .heading {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  .segmented {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
  }
  .segmented button {
    display: grid;
    place-items: center;
    width: 44px;
    height: 38px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .segmented button.on {
    background: var(--jf-cloud-soft);
    color: var(--jf-ink-90);
    box-shadow: var(--jf-elevation-shadow-1);
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(300px, 400px) minmax(0, 640px);
    justify-content: center;
    align-items: center;
    gap: 56px;
    padding: var(--jf-space-s7) var(--jf-space-s9) var(--jf-space-s9);
  }
  .player {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s5);
    min-width: 0;
  }
  .hero {
    width: 100%;
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    container-type: size;
  }
  /* Square, as large as the space allows (on a phone the space is taller than wide or the
     other way round). */
  .cover,
  .hero :global(.art) {
    display: block;
    width: min(100cqw, 100cqh);
    height: min(100cqw, 100cqh);
    object-fit: cover;
    border-radius: var(--jf-artwork-radius-hero);
    box-shadow: var(--jf-elevation-shadow-3);
    /* Springs back to full size when playing (dur-3 Spring)... */
    transition: scale var(--jf-motion-dur-3) var(--jf-motion-ease-spring);
  }
  /* ...and settles at 0.88 when paused (dur-3 Fluid). Reduced motion: no scaling. */
  .paused .cover,
  .paused :global(.art) {
    scale: var(--jf-player-artwork-paused-scale);
    transition-timing-function: var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) .hero .cover,
  :global(:root[data-motion='reduced']) .hero :global(.art) {
    scale: none;
  }
  /* Grows out of the player's artwork as Now Playing opens (src/lib/motion.ts, viewTransition). */
  :global(:root[data-view-transition='now-playing']) .cover,
  :global(:root[data-view-transition='now-playing']) .hero :global(.art) {
    view-transition-name: np-artwork;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .title {
    margin: 0;
    font-size: var(--jf-type-roles-now-playing-title-size);
    line-height: var(--jf-type-roles-now-playing-title-line-height);
    font-weight: var(--jf-type-roles-now-playing-title-weight);
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .artist {
    margin: 0;
    font-size: var(--jf-type-roles-now-playing-artist-size);
    line-height: var(--jf-type-roles-now-playing-artist-line-height);
    font-weight: var(--jf-type-roles-now-playing-artist-weight);
    color: var(--jf-ink-70);
    overflow-wrap: anywhere;
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    margin: 0;
    padding: var(--jf-space-s3);
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-tint-veil);
    color: var(--jf-ink-90);
    font-size: 14px;
    line-height: 1.4;
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  .pane {
    height: min(100%, 720px);
    min-height: 0;
    min-width: 0;
  }
  .muted {
    color: var(--jf-ink-50);
  }
  .no-lyrics {
    max-width: 420px;
    color: var(--jf-ink-70);
  }
  .title-2 {
    font-size: var(--jf-type-text-h3);
    font-weight: 700;
    color: var(--jf-ink-90);
  }

  /* Phones: one column; the chosen pane replaces the artwork. */
  @media (max-width: 760px) {
    .heading,
    .back-label {
      display: none;
    }
    .body {
      display: flex;
      flex-direction: column;
      align-items: stretch;
      gap: var(--jf-space-s4);
      padding: var(--jf-space-s4) var(--jf-space-s4) max(var(--jf-space-s5), env(safe-area-inset-bottom));
    }
    .player {
      flex: 1;
      min-height: 0;
      justify-content: flex-end;
      gap: var(--jf-space-s4);
    }
    .hero {
      flex: 1;
      min-height: 0;
      width: 100%;
      aspect-ratio: auto;
    }
    .pane {
      display: none;
    }
    .pane-open .pane {
      display: block;
      order: -1;
      flex: 1;
      height: auto;
    }
    .pane-open .hero {
      display: none;
    }
    .pane-open .player {
      flex: none;
    }
    .segmented button.on:not(.phone-on) {
      background: transparent;
      box-shadow: none;
      color: var(--jf-ink-70);
    }
    .title {
      font-size: 22px;
    }
  }
</style>
