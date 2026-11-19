<script lang="ts">
  // The shell: navigation (a sidebar on wide screens, tabs on phones), the current page, the
  // player (the capsule under 900 px, the player bar from 900 px on; only the one shown exists, so
  // only its clock runs), Now Playing over everything, files dropped anywhere, and the keyboard
  // shortcuts of the Linux app. Luminous (v0.0.6): the sidebar is a floating panel of Clear glass,
  // the player and the phone tabs are Veil capsules floating over the content, which fades out
  // just above them.
  import { MediaQuery } from 'svelte/reactivity';
  import Icon from './components/Icon.svelte';
  import MiniPlayer from './components/MiniPlayer.svelte';
  import PlayerBar from './components/PlayerBar.svelte';
  import { type App as AppState, provideApp } from './lib/app';
  import { t } from './lib/i18n/index.svelte';
  import type { Tab } from './lib/state/nav.svelte';
  import { toast, toasts } from './lib/state/toast.svelte';
  import NowPlaying from './nowplaying/NowPlaying.svelte';
  import Albums from './views/Albums.svelte';
  import Artists from './views/Artists.svelte';
  import Browse from './views/Browse.svelte';
  import Home from './views/Home.svelte';
  import SearchView from './views/SearchView.svelte';
  import Settings from './views/Settings.svelte';
  import Songs from './views/Songs.svelte';
  import SourcePage from './views/SourcePage.svelte';
  import SourcesView from './views/SourcesView.svelte';

  let { app }: { app: AppState } = $props();
  // The app object is made once, before mounting, and never replaced.
  // svelte-ignore state_referenced_locally
  provideApp(app);
  // svelte-ignore state_referenced_locally
  const { nav, player, files } = app;

  const wide = new MediaQuery('min-width: 900px');
  let fileInput: HTMLInputElement | undefined = $state();
  let content: HTMLElement | undefined = $state();

  // Each page keeps where it was scrolled to: a page opened anew starts at its top, and going
  // back finds the one below where it was left (the content column is the one scroller).
  const offsets = new WeakMap<object, number>();
  let shown: object | null = null;
  $effect.pre(() => {
    const route = nav.route;
    if (shown && content) offsets.set(shown, content.scrollTop);
    shown = route;
  });
  // The aurora drifts while music plays (app.css).
  $effect(() => {
    document.documentElement.toggleAttribute('data-playing', player.playing);
  });
  $effect(() => {
    const route = nav.route;
    const element = content;
    if (!element) return;
    const top = offsets.get(route) ?? 0;
    element.scrollTop = top;
    // A page that loads its rows after the first paint is not tall enough yet: try once more.
    if (top > 0) requestAnimationFrame(() => (element.scrollTop = top));
  });
  let dragging = $state(false);
  let dragDepth = 0;
  let installEvent = $state<(Event & { prompt(): Promise<void> }) | null>(null);

  // The places in the order every app lists them (docs/ui.md); a null starts a new group.
  const PLACES: ([Tab, string, string] | null)[] = [
    ['home', 'Home', 'home'],
    ['albums', 'Albums', 'album'],
    ['artists', 'Artists', 'artist'],
    ['songs', 'Songs', 'note'],
    null,
    ['search', 'Search', 'search'],
    ['sources', 'Sources', 'sources'],
  ];
  const LIBRARY_TABS: Tab[] = ['albums', 'artists', 'songs'];
  // Phones: the library is one tab, as on Android.
  const PHONE_TABS: [Tab, string, string][] = [
    ['home', 'Home', 'home'],
    ['albums', 'Library', 'library'],
    ['search', 'Search', 'search'],
    ['sources', 'Sources', 'sources'],
    ['settings', 'Settings', 'settings'],
  ];
  function onTab(id: Tab): boolean {
    return id === 'albums' ? LIBRARY_TABS.includes(nav.tab) : nav.tab === id;
  }

  async function play(list: File[]) {
    if (list.length === 0) return;
    const entries = await files.add(list);
    if (entries.length === 0) {
      toast(t('Nothing to play in what was dropped.'));
      return;
    }
    player.playEntries(entries, 0);
  }

  function hasFiles(event: DragEvent): boolean {
    return [...(event.dataTransfer?.types ?? [])].includes('Files');
  }

  function typing(target: EventTarget | null): boolean {
    const element = target as HTMLElement | null;
    if (!element) return false;
    return element.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName);
  }

  function key(event: KeyboardEvent) {
    const mod = event.ctrlKey || event.metaKey;
    if (event.key === 'Escape') {
      if (nav.nowPlaying) {
        nav.closeNowPlaying();
        event.preventDefault();
      } else if (nav.stack.length > 1 && !document.querySelector('dialog[open]')) {
        nav.back();
        event.preventDefault();
      }
      return;
    }
    if (typing(event.target)) return;
    if (event.key === ' ' && !mod && !event.shiftKey) {
      const element = event.target as HTMLElement;
      if (element.closest('button, a, [role="slider"], [role="switch"], summary')) return;
      player.toggle();
      event.preventDefault();
      return;
    }
    if (event.shiftKey && !mod && (event.key === 'ArrowRight' || event.key === 'ArrowLeft')) {
      if ((event.target as HTMLElement).closest('[role="slider"], input')) return;
      player.seek(player.positionMs + (event.key === 'ArrowRight' ? 5000 : -5000));
      event.preventDefault();
      return;
    }
    if (!mod || event.altKey) return;
    const actions: Record<string, () => void> = {
      ArrowRight: () => player.next(),
      ArrowLeft: () => player.previous(),
      ArrowUp: () => player.setVolume(player.volume + 0.05),
      ArrowDown: () => player.setVolume(player.volume - 0.05),
      m: () => player.setMuted(!player.muted),
      s: () => player.setShuffle(!player.queue.shuffle),
      r: () => player.cycleRepeat(),
      p: () => (nav.nowPlaying ? nav.closeNowPlaying() : nav.openNowPlaying()),
      l: () => nav.openNowPlaying('lyrics'),
      u: () => nav.openNowPlaying('queue'),
      f: () => {
        nav.closeNowPlaying();
        nav.switchTab('search');
      },
    };
    const action = actions[event.key.length === 1 ? event.key.toLowerCase() : event.key];
    if (!action) return;
    action();
    event.preventDefault();
  }
</script>

<svelte:window
  onkeydown={key}
  onbeforeinstallprompt={(event) => {
    event.preventDefault();
    installEvent = event as unknown as Event & { prompt(): Promise<void> };
  }}
  ondragenter={(event) => {
    if (!hasFiles(event)) return;
    dragDepth += 1;
    dragging = true;
  }}
  ondragleave={() => {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) dragging = false;
  }}
  ondragover={(event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
  }}
  ondrop={(event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    dragDepth = 0;
    dragging = false;
    void play([...(event.dataTransfer?.files ?? [])]);
  }}
/>

<a class="skip" href="#main">{t('Skip to content')}</a>

<div class="shell" class:with-player={player.current !== null} inert={nav.nowPlaying}>
  <nav class="sidebar glass-clear" aria-label={t('Library sections')}>
    <div class="brand">
      <img src="./icons/icon.svg" alt="" width="32" height="32" />
      <span>Jellyfesh</span>
    </div>
    {#each PLACES as place, index (place?.[0] ?? `gap-${index}`)}
      {#if place}
        {@const [id, label, icon] = place}
        <button class="nav-item" class:on={nav.tab === id} aria-current={nav.tab === id ? 'page' : undefined} onclick={() => nav.switchTab(id)}>
          <Icon name={icon} />{t(label)}
        </button>
      {:else}
        <hr class="nav-gap" />
      {/if}
    {/each}
    <div class="nav-end">
      <button class="nav-item" onclick={() => fileInput?.click()}><Icon name="file" />{t('Open Files…')}</button>
      <button
        class="nav-item"
        class:on={nav.tab === 'settings'}
        aria-current={nav.tab === 'settings' ? 'page' : undefined}
        onclick={() => nav.switchTab('settings')}><Icon name="settings" />{t('Settings')}</button
      >
    </div>
  </nav>

  <main class="content" id="main" tabindex="-1" bind:this={content}>
    {#if nav.stack.length > 1}
      <button class="back" onclick={() => nav.back()}><Icon name="back" size={18} />{t('Back')}</button>
    {/if}
    {#key nav.route}
      {@const route = nav.route}
      <div class="route" class:fade={nav.motion === 'back'}>
        {#if route.view === 'browse'}
          <Browse plugin={route.plugin} item={route.item} />
        {:else if route.view === 'source'}
          <SourcePage plugin={route.plugin} />
        {:else if route.view === 'albums' || route.view === 'artists' || route.view === 'songs'}
          <nav class="library-tabs" aria-label={t('Library')}>
            {#each [['albums', t('Albums')], ['artists', t('Artists')], ['songs', t('Songs')]] as [id, label] (id)}
              <button class:on={route.view === id} aria-current={route.view === id ? 'page' : undefined} onclick={() => nav.switchTab(id as Tab)}>{label}</button>
            {/each}
          </nav>
          {#if route.view === 'albums'}<Albums />{:else if route.view === 'artists'}<Artists />{:else}<Songs />{/if}
        {:else if route.view === 'search'}
          <SearchView />
        {:else if route.view === 'sources'}
          <SourcesView />
        {:else if route.view === 'settings'}
          <Settings
            installPrompt={installEvent !== null}
            oninstall={async () => {
              await installEvent?.prompt();
              installEvent = null;
            }}
          />
        {:else}
          <Home onopenfiles={() => fileInput?.click()} />
        {/if}
      </div>
    {/key}
  </main>

  <div class="dock">
    {#if player.problem && !nav.nowPlaying}
      <p class="banner" role="alert">
        <Icon name="alert" size={18} /><span>{player.problem}</span>
        <button aria-label={t('Close')} onclick={() => (player.problem = null)}><Icon name="close" size={16} /></button>
      </p>
    {/if}
    <!-- Covered by Now Playing: not drawn at all, so its clock does not run. -->
    {#if !nav.nowPlaying}
      {#if wide.current}<PlayerBar />{:else}<MiniPlayer />{/if}
    {/if}
  </div>

  <nav class="tabs glass-veil" aria-label={t('Library sections')}>
    {#each PHONE_TABS as [id, label, icon] (id)}
      <button class:on={onTab(id)} aria-current={onTab(id) ? 'page' : undefined} onclick={() => nav.switchTab(id)}>
        <span class="pill"><Icon name={icon} /></span><span>{t(label)}</span>
      </button>
    {/each}
  </nav>
</div>

{#if nav.nowPlaying}
  <NowPlaying />
{/if}

{#if dragging}
  <div class="drop" aria-hidden="true">
    <div class="drop-card"><Icon name="file" size={32} />{t('Drop audio files to play them. They are not saved.')}</div>
  </div>
{/if}

<div class="toasts" aria-live="polite" aria-atomic="false">
  {#each toasts.items as item (item.id)}
    <p class="toast">{item.text}</p>
  {/each}
</div>

<input
  bind:this={fileInput}
  class="file-input"
  type="file"
  multiple
  accept="audio/*,.flac,.mp3,.m4a,.aac,.ogg,.oga,.opus,.wav,.lrc,.ttml,.vtt"
  aria-hidden="true"
  tabindex="-1"
  onchange={(event) => {
    const input = event.currentTarget as HTMLInputElement;
    void play([...(input.files ?? [])]);
    input.value = '';
  }}
/>

<style>
  .skip {
    position: absolute;
    left: -9999px;
    top: 8px;
    z-index: 50;
    padding: 8px 12px;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
  }
  .skip:focus {
    left: 8px;
  }
  .shell {
    /* How far the floating capsules reach up from the bottom of the window: the content fades
       out just above them (edge-blur.bottom-height), and its last row can scroll clear of them. */
    --floor: 0px;
    --fade: var(--jf-edge-blur-bottom-height);
    display: grid;
    grid-template-columns: 248px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100dvh;
  }
  .shell.with-player {
    /* The player bar, 16 px above the bottom. */
    --floor: calc(var(--jf-player-bar-height) + var(--jf-space-s4) + max(0px, env(safe-area-inset-bottom)));
  }
  /* A floating panel of Clear glass (Luminous), r-2xl, 12 px from the window's edges. */
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-height: 0;
    margin: max(var(--jf-space-s3), env(safe-area-inset-top)) 0 var(--jf-space-s3) max(var(--jf-space-s3), env(safe-area-inset-left));
    padding: var(--jf-space-s5) var(--jf-space-s3);
    overflow-y: auto;
    border-radius: var(--jf-radius-r-2xl);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 0 10px var(--jf-space-s6);
    font-weight: 700;
    font-size: 16px;
    letter-spacing: -0.01em;
  }
  .brand img {
    border-radius: 9px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-height: 42px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-size: 14.5px;
    font-weight: 500;
    text-align: start;
    cursor: pointer;
  }
  .nav-item:hover {
    background: var(--jf-fill-hover);
    color: var(--jf-ink-90);
  }
  .nav-item.on {
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
    font-weight: 600;
  }
  .nav-gap {
    margin: var(--jf-space-s2) var(--jf-space-s3);
    border: 0;
    border-top: 1px solid var(--jf-line-soft);
  }
  .nav-end {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: auto;
  }
  /* Albums, Artists and Songs as one Library tab on phones. */
  .library-tabs {
    display: none;
  }
  button:focus-visible,
  a:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .content {
    min-width: 0;
    overflow-y: auto;
    padding: max(var(--jf-space-s8), env(safe-area-inset-top)) var(--jf-layout-gutter-fluid)
      calc(var(--floor) + var(--fade) + var(--jf-space-s7));
    outline: none;
  }
  /* The content fades out just above the floating capsules, so nothing reads through them. */
  .with-player .content {
    mask-image: linear-gradient(to top, transparent var(--floor), black calc(var(--floor) + var(--fade)));
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 40px;
    margin: 0 0 var(--jf-space-s3) -8px;
    padding: 0 12px 0 6px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-accent-text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .back:hover {
    background: var(--jf-fill-hover);
  }
  /* Pages fade in and rise 8 px (dur-2 Fluid); going back fades (dur-1); reduced motion fades. */
  .route {
    animation: jf-page-in var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  .route.fade {
    animation: jf-fade-in var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) .route {
    animation: jf-fade-in var(--jf-motion-dur-1) linear;
  }
  /* The player bar floats over the content column, its ends on the content's own gutters. */
  .dock {
    position: fixed;
    left: 248px;
    right: 0;
    bottom: max(var(--jf-space-s4), env(safe-area-inset-bottom));
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
    padding: 0 var(--jf-layout-gutter-fluid);
    pointer-events: none;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    width: min(560px, 100%);
    margin: 0;
    padding: var(--jf-space-s2) var(--jf-space-s2) var(--jf-space-s2) var(--jf-space-s4);
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    font-size: 14px;
    line-height: 1.4;
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-2);
    pointer-events: auto;
  }
  .banner :global(svg) {
    flex: none;
    color: var(--jf-warning);
  }
  .banner span {
    flex: 1;
  }
  .banner button {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .tabs {
    display: none;
  }
  .drop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: var(--jf-scrim);
    pointer-events: none;
  }
  .drop-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s3);
    max-width: 320px;
    padding: var(--jf-space-s7);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    font-weight: 600;
    text-align: center;
    box-shadow:
      inset 0 0 0 2px var(--jf-accent),
      var(--jf-elevation-shadow-3);
  }
  .drop-card :global(svg) {
    color: var(--jf-accent-text);
  }
  .toasts {
    position: fixed;
    top: max(var(--jf-space-s4), env(safe-area-inset-top));
    left: 50%;
    z-index: 45;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
    transform: translateX(-50%);
    pointer-events: none;
  }
  .toast {
    margin: 0;
    padding: 10px 16px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-ink-90);
    color: var(--jf-sky);
    font-size: 14px;
    font-weight: 600;
    box-shadow: var(--jf-elevation-shadow-2);
  }
  .file-input {
    display: none;
  }

  /* Phones: the tabs a floating Veil capsule at the bottom, the mini-player a capsule above them. */
  @media (max-width: 760px) {
    .shell {
      /* The tab bar: 68 px, 24 px above the bottom. */
      --tabs-top: calc(68px + max(var(--jf-space-s6), env(safe-area-inset-bottom)));
      --floor: var(--tabs-top);
      grid-template-columns: minmax(0, 1fr);
    }
    .shell.with-player {
      --floor: calc(var(--tabs-top) + var(--jf-space-s3) + var(--jf-player-mini-height));
    }
    .sidebar {
      display: none;
    }
    .content {
      padding: max(var(--jf-space-s5), env(safe-area-inset-top)) var(--jf-space-s4)
        calc(var(--floor) + var(--fade) + var(--jf-space-s4));
    }
    .content,
    .with-player .content {
      mask-image: linear-gradient(to top, transparent var(--floor), black calc(var(--floor) + var(--fade)));
    }
    .dock {
      left: 0;
      bottom: calc(var(--tabs-top) + var(--jf-space-s3));
      padding: 0 14px;
    }
    .tabs {
      position: fixed;
      left: 14px;
      right: 14px;
      bottom: max(var(--jf-space-s6), env(safe-area-inset-bottom));
      z-index: 10;
      display: flex;
      justify-content: space-around;
      align-items: center;
      height: 68px;
      padding: 0 6px;
      border-radius: var(--jf-radius-r-full);
    }
    .tabs button {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 3px;
      min-width: 0;
      min-height: 56px;
      padding: 4px 0 0;
      border: 0;
      border-radius: var(--jf-radius-r-lg);
      background: transparent;
      color: var(--jf-ink-70);
      font: inherit;
      font-size: 11px;
      font-weight: 600;
      cursor: pointer;
    }
    .tabs .pill {
      display: grid;
      place-items: center;
      width: 54px;
      height: 30px;
      border-radius: 15px;
      transition: background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
    }
    .tabs button.on {
      color: var(--jf-accent-text);
    }
    .tabs button.on .pill {
      background: var(--jf-accent-soft);
    }
    .library-tabs {
      display: flex;
      gap: 4px;
      margin-bottom: var(--jf-space-s4);
      padding: 4px;
      border-radius: var(--jf-radius-r-full);
      background: var(--jf-fill);
    }
    .library-tabs button {
      flex: 1;
      min-height: 36px;
      border: 0;
      border-radius: var(--jf-radius-r-full);
      background: transparent;
      color: var(--jf-ink-70);
      font: inherit;
      font-weight: 600;
      cursor: pointer;
    }
    .library-tabs button.on {
      background: var(--jf-cloud);
      color: var(--jf-ink-90);
      box-shadow: var(--jf-elevation-shadow-1);
    }
  }
  @media (max-width: 760px) and (hover: hover) {
    .tabs button:hover:not(.on) .pill {
      background: var(--jf-fill-hover);
    }
  }
</style>
