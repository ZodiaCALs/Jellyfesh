<script lang="ts">
  // Now Playing as a stage (Luminous, v0.0.6), in the person's backdrop style: Artwork (the core's
  // baked backdrop of the cover, tone-mapped for the appearance) or Misty (the Misty field in the
  // theme's hue) behind everything. Library (⌄ on phones) and the segmented control (Lyrics, Queue,
  // Info) float at the top; the artwork (380 px, shadow-3) and the titles on one side and the
  // chosen pane on the other, or stacked on a phone; every control in one floating Veil dock
  // (radius 40, 80 px high; a rounded card on phones): shuffle, previous, play, next, repeat, the
  // seek bar with its times, the volume and More. There is no device button: a browser decides
  // where sound goes, and Jellyfesh Web has no remote sessions (ADR-0018). After Apple Music: the
  // artwork scales to 0.88 while paused and springs back when playing, the backdrop crossfades
  // between songs, and the whole view opens with Deeper and closes with Back (src/lib/motion.ts).
  // The dock is the view's one live blur layer; the top controls are Veil without blur.
  import { MediaQuery } from 'svelte/reactivity';
  import Art from '../components/Art.svelte';
  import Icon from '../components/Icon.svelte';
  import Menu from '../components/Menu.svelte';
  import SeekBar from '../components/SeekBar.svelte';
  import Transport from '../components/Transport.svelte';
  import Volume from '../components/Volume.svelte';
  import { useApp } from '../lib/app';
  import { t } from '../lib/i18n/index.svelte';
  import { nowPlayingMenu } from '../lib/items';
  import { back, crossfade, deeper } from '../lib/motion';
  import type { Pane } from '../lib/state/nav.svelte';
  import { arrangement } from '../lib/look';
  import CommentsPane from './CommentsPane.svelte';
  import MakeLyrics from './MakeLyrics.svelte';
  import SungLine from './SungLine.svelte';
  import InfoPane from './InfoPane.svelte';
  import LyricsPane from './LyricsPane.svelte';
  import QueuePane from './QueuePane.svelte';

  const app = useApp();
  const { player, theme, nav } = app;
  const style = $derived(app.settings.value.style);
  const row = $derived(player.current);
  /** On a phone the pane replaces the artwork while one is chosen. */
  let phonePane = $state(false);
  /** Wider than a phone: the dock is one row, with the seek bar's times beside it. */
  const wide = new MediaQuery('min-width: 761px');
  const more = $derived((void app.sources.revision, nowPlayingMenu(app, row)));

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

  // The look's layout (ADR-0020): Stage (v0.0.6), Artwork (the cover and the sung line in place of
  // the Lyrics pane) or Lyrics (any pane across the window, a small cover in the corner).
  const arranged = $derived(arrangement(app.look.value.now_playing, nav.pane));

  // A fourth pane, Comments, for songs of a source whose plugin has comments (API 1.2).
  const commented = $derived.by(() => {
    void app.sources.revision;
    const found = row ? player.sourceItem(row) : null;
    return found && found.plugin.declares('Comments') ? found : null;
  });
  const PANES = $derived<[Pane, string, string][]>([
    ['lyrics', 'Lyrics', 'lyrics'],
    ['queue', 'Queue', 'queue'],
    ['info', 'Info', 'info'],
    ...(commented ? ([['comments', 'Comments', 'comments']] as [Pane, string, string][]) : []),
  ]);
  // A song without comments leaves the Comments pane for its lyrics.
  $effect(() => {
    if (nav.pane === 'comments' && !commented) nav.pane = 'lyrics';
  });

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

  <header class="top">
    <button class="back glass-veil-flat" onclick={() => nav.closeNowPlaying()} aria-label={t('Back to Library')}>
      <Icon name="down" /><span class="back-label">{t('Library')}</span>
    </button>
    <div class="segmented glass-veil-flat" role="toolbar" aria-label={t('Now Playing views')}>
      {#each PANES as [id, label, icon] (id)}
        <button
          aria-pressed={nav.pane === id}
          class:on={nav.pane === id}
          class:phone-on={nav.pane === id && phonePane}
          aria-label={t(label)}
          title={t(label)}
          onclick={() => choose(id)}
        >
          <span class="seg-icon"><Icon name={icon} /></span><span class="seg-label">{t(label)}</span>
        </button>
      {/each}
    </div>
  </header>

  <main class="stage" class:cover-layout={arranged === 'cover'} class:wide-pane={arranged === 'wide-pane'}>
    <section class="player" aria-label={t('Player')}>
      <div class="hero" class:paused={!player.playing}>
        {#each theme.artwork ? [theme.artwork] : [] as url (url)}
          <img
            class="cover"
            src={url}
            alt={t('Artwork of {title}', { title: row?.title ?? '' })}
            referrerpolicy="no-referrer"
            transition:crossfade
          />
        {:else}
          <Art kind="Album" size={380} radius="hero" />
        {/each}
      </div>
      <div class="titles">
        <h1 class="title">{row?.title ?? t('Nothing Playing')}</h1>
        <p class="artist">
          {#if row}{[row.artist ?? t('Unknown Artist'), row.album].filter(Boolean).join(' · ')}{:else}{t(
              'Pick a song, an album or a playlist.',
            )}{/if}
        </p>
      </div>
      {#if arranged === 'cover' && wide.current}<SungLine />{/if}
      {#if player.problem}
        <p class="problem" role="alert"><Icon name="alert" size={18} />{player.problem}</p>
      {/if}
    </section>

    <section class="pane" aria-label={t(PANES.find((p) => p[0] === nav.pane)?.[1] ?? 'Lyrics')}>
      {#if nav.pane === 'lyrics'}
        {#if player.lyrics.status === 'ready'}
          <LyricsPane lyrics={player.lyrics.lyrics} anchor={player.anchor} made={player.lyrics.made ?? null} />
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
            {#if row}<MakeLyrics {row} />{/if}
          </div>
        {/if}
      {:else if nav.pane === 'queue'}
        <QueuePane />
      {:else}
        {#if nav.pane === 'comments' && commented}
          <CommentsPane plugin={commented.plugin} item={commented.item} />
        {:else}
          <InfoPane />
        {/if}
      {/if}
    </section>
  </main>

  <div class="dock glass-veil" class:has-more={more.length > 0} role="group" aria-label={t('Playback')}>
    <div class="transport"><Transport dock /></div>
    <div class="seek">
      <SeekBar anchor={player.anchor} durationMs={player.durationMs} onseek={(ms) => player.seek(ms)} disabled={!row} compact={wide.current} />
    </div>
    <div class="vol"><Volume /></div>
    {#if more.length > 0}
      <div class="more"><Menu items={more} label={t('More')} /></div>
    {/if}
  </div>
</div>

<style>
  .np {
    /* The artwork: 380 px, smaller in short windows. */
    --art: clamp(200px, calc(100dvh - 400px), 380px);
    --side: clamp(var(--jf-space-s6), 7.5vw, 96px);
    --top: calc(78px + env(safe-area-inset-top));
    --dock-h: var(--jf-player-bar-height);
    --dock-bottom: max(28px, env(safe-area-inset-bottom));
    position: fixed;
    inset: 0;
    z-index: 20;
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

  /* Library and the segmented control, floating at the top. */
  .top {
    position: absolute;
    inset: 0 0 auto;
    z-index: 3;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    height: var(--top);
    padding: env(safe-area-inset-top) max(28px, env(safe-area-inset-right)) 0 max(28px, env(safe-area-inset-left));
  }
  .back {
    justify-self: start;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 44px;
    padding: 0 18px 0 12px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    color: var(--jf-ink-90);
    font: inherit;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    cursor: pointer;
  }
  .segmented {
    display: flex;
    gap: 2px;
    padding: 4px;
    border-radius: var(--jf-radius-r-full);
  }
  .segmented button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 44px;
    height: 36px;
    padding: 0 16px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .segmented button:hover:not(.on) {
    background: var(--jf-fill-hover);
    color: var(--jf-ink-90);
  }
  .segmented button.on {
    background: var(--jf-ink-90);
    color: var(--jf-sky);
  }
  .seg-icon {
    display: none;
  }
  button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }

  /* The stage: the artwork and titles, and the pane, between the top controls and the dock. */
  .stage {
    position: absolute;
    top: var(--top);
    bottom: calc(var(--dock-bottom) + var(--dock-h) + var(--jf-space-s5));
    left: var(--side);
    right: var(--side);
    max-width: 1320px;
    margin-inline: auto;
    display: grid;
    grid-template-columns: var(--art) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    column-gap: clamp(var(--jf-space-s7), 5vw, var(--jf-space-s10));
    align-items: start;
  }
  .player {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s7);
    min-width: 0;
    padding-top: 22px;
  }
  .hero {
    width: var(--art);
    height: var(--art);
    display: grid;
    place-items: center;
    container-type: size;
  }
  /* Two covers share the place while a new song's crossfades in over the last. */
  .hero > :global(*) {
    grid-area: 1 / 1;
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
    gap: 6px;
    min-width: 0;
  }
  .title {
    margin: 0;
    font-size: calc(var(--jf-type-roles-now-playing-title-size) * var(--jf-text-scale, 1));
    line-height: var(--jf-type-roles-now-playing-title-line-height);
    font-weight: var(--jf-type-roles-now-playing-title-weight);
    letter-spacing: -0.018em;
    overflow-wrap: anywhere;
  }
  .artist {
    margin: 0;
    font-size: calc(17px * var(--jf-text-scale, 1));
    line-height: var(--jf-type-roles-now-playing-artist-line-height);
    font-weight: 500;
    color: var(--jf-ink-70);
    overflow-wrap: anywhere;
  }
  .problem {
    display: flex;
    gap: var(--jf-space-s2);
    margin: 0;
    padding: var(--jf-space-s3);
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-tint-veil);
    color: var(--jf-ink-90);
    font-size: calc(14px * var(--jf-text-scale, 1));
    line-height: 1.4;
  }
  .problem :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  /* The look's layouts, where the stage is wide (phones keep one view at a time). */
  @media (min-width: 761px) {
    .stage.cover-layout {
      grid-template-columns: minmax(0, 1fr);
      justify-items: center;
    }
    .stage.cover-layout .pane {
      display: none;
    }
    .stage.cover-layout .player {
      align-items: center;
      text-align: center;
    }
    .stage.wide-pane {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
      row-gap: var(--jf-space-s4);
    }
    .stage.wide-pane .player {
      flex-direction: row;
      align-items: center;
      gap: var(--jf-space-s4);
      padding-top: 0;
    }
    .stage.wide-pane .hero {
      width: 72px;
      height: 72px;
    }
  }
  .pane {
    height: 100%;
    min-height: 0;
    min-width: 0;
    /* Lyrics size themselves to the pane (lyrics.line: 40 px, the fluid maximum, where it is wide). */
    container-type: inline-size;
  }
  .muted {
    color: var(--jf-ink-70);
  }
  .no-lyrics {
    max-width: 420px;
    padding-top: 22px;
    color: var(--jf-ink-70);
  }
  .title-2 {
    font-size: calc(var(--jf-type-text-h3) * var(--jf-text-scale, 1));
    font-weight: 700;
    color: var(--jf-ink-90);
  }

  /* Every control in one floating Veil dock. */
  .dock {
    position: absolute;
    left: var(--side);
    right: var(--side);
    bottom: var(--dock-bottom);
    z-index: 3;
    max-width: 1320px;
    height: var(--dock-h);
    margin-inline: auto;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(120px, 200px);
    grid-template-areas: 'transport seek vol';
    align-items: center;
    column-gap: var(--jf-space-s6);
    padding: 0 var(--jf-space-s6) 0 var(--jf-space-s4);
    border-radius: 40px;
  }
  .dock.has-more {
    grid-template-columns: auto minmax(0, 1fr) minmax(120px, 200px) auto;
    grid-template-areas: 'transport seek vol more';
    padding-inline-end: var(--jf-space-s3);
    column-gap: var(--jf-space-s5);
  }
  .transport {
    grid-area: transport;
  }
  .seek {
    grid-area: seek;
    min-width: 0;
  }
  .vol {
    grid-area: vol;
    min-width: 0;
  }
  .more {
    grid-area: more;
  }
  /* A narrower dock keeps the mute button and drops the volume line; Ctrl+↑/↓ still change it. */
  @media (min-width: 761px) and (max-width: 1040px) {
    .dock,
    .dock.has-more {
      grid-template-columns: auto minmax(0, 1fr) auto auto;
      column-gap: var(--jf-space-s3);
    }
    .vol :global(.line),
    .vol :global(.glyph) {
      display: none;
    }
  }

  /* Phones: one column under the top controls; the chosen pane replaces the artwork; the dock is
     a rounded card with the seek bar, the transport, and the volume with More. */
  @media (max-width: 760px) {
    .np {
      --top: calc(64px + env(safe-area-inset-top));
      --side: var(--jf-space-s4);
      --dock-bottom: max(var(--jf-space-s4), env(safe-area-inset-bottom));
      --dock-h: 216px;
    }
    .top {
      grid-template-columns: auto minmax(0, 1fr) auto;
      padding-inline: max(18px, env(safe-area-inset-left)) max(18px, env(safe-area-inset-right));
    }
    .back {
      width: 44px;
      padding: 0;
      justify-content: center;
    }
    .back-label,
    .seg-label {
      display: none;
    }
    .segmented {
      grid-column: 3;
    }
    .segmented button {
      width: 44px;
      padding: 0;
    }
    .seg-icon {
      display: grid;
    }
    .segmented button.on:not(.phone-on) {
      background: transparent;
      color: var(--jf-ink-70);
    }
    .stage {
      left: var(--jf-space-s6);
      right: var(--jf-space-s6);
      display: flex;
      flex-direction: column;
      align-items: stretch;
      gap: var(--jf-space-s5);
      padding-top: var(--jf-space-s3);
    }
    .player {
      flex: 1;
      min-height: 0;
      gap: var(--jf-space-s5);
      padding-top: 0;
    }
    .hero {
      flex: 1;
      min-height: 0;
      width: 100%;
      height: auto;
    }
    .title {
      font-size: calc(25px * var(--jf-text-scale, 1));
    }
    .artist {
      font-size: calc(15.5px * var(--jf-text-scale, 1));
    }
    .pane {
      display: none;
    }
    .pane-open .pane {
      display: block;
      flex: 1;
      height: auto;
      min-height: 0;
    }
    .pane-open .player {
      display: none;
    }
    .dock,
    .dock.has-more {
      height: auto;
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        'seek seek'
        'transport transport'
        'vol more';
      row-gap: var(--jf-space-s2);
      padding: var(--jf-space-s5) 22px var(--jf-space-s3);
      border-radius: 32px;
    }
  }
</style>
