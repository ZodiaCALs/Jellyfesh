<script lang="ts">
  // The shell: navigation (a sidebar on wide screens, tabs on phones), the current page, the
  // player (the capsule under 900 px, the player bar from 900 px on; only the one shown exists, so
  // only its clock runs), Now Playing over everything, files dropped anywhere, and the keyboard
  // shortcuts of the Linux app.
  import { MediaQuery } from 'svelte/reactivity';
  import Icon from './components/Icon.svelte';
  import MiniPlayer from './components/MiniPlayer.svelte';
  import PlayerBar from './components/PlayerBar.svelte';
  import { type App as AppState, provideApp } from './lib/app';
  import { t } from './lib/i18n/index.svelte';
  import type { Tab } from './lib/state/nav.svelte';
  import { toast, toasts } from './lib/state/toast.svelte';
  import NowPlaying from './nowplaying/NowPlaying.svelte';
  import Browse from './views/Browse.svelte';
  import Home from './views/Home.svelte';
  import Preferences from './views/Preferences.svelte';
  import SearchView from './views/SearchView.svelte';
  import SourcesView from './views/SourcesView.svelte';

  let { app }: { app: AppState } = $props();
  // The app object is made once, before mounting, and never replaced.
  // svelte-ignore state_referenced_locally
  provideApp(app);
  // svelte-ignore state_referenced_locally
  const { nav, player, files } = app;

  const wide = new MediaQuery('min-width: 900px');
  let fileInput: HTMLInputElement | undefined = $state();
  let dragging = $state(false);
  let dragDepth = 0;
  let installEvent = $state<(Event & { prompt(): Promise<void> }) | null>(null);

  const TABS: [Tab, string, string][] = [
    ['home', 'Home', 'home'],
    ['search', 'Search', 'search'],
    ['sources', 'Sources', 'sources'],
    ['settings', 'Preferences', 'settings'],
  ];

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

<div class="shell" inert={nav.nowPlaying}>
  <nav class="sidebar" aria-label={t('Library sections')}>
    <div class="brand">
      <img src="./icons/icon.svg" alt="" width="32" height="32" />
      <span>Jellyfesh</span>
    </div>
    {#each TABS as [id, label, icon] (id)}
      <button class="nav-item" class:on={nav.tab === id} aria-current={nav.tab === id ? 'page' : undefined} onclick={() => nav.switchTab(id)}>
        <Icon name={icon} />{t(label)}
      </button>
    {/each}
    <button class="nav-item files" onclick={() => fileInput?.click()}><Icon name="file" />{t('Open Files…')}</button>
  </nav>

  <main class="content" id="main" tabindex="-1">
    {#if nav.stack.length > 1}
      <button class="back" onclick={() => nav.back()}><Icon name="back" size={18} />{t('Back')}</button>
    {/if}
    {#key nav.route}
      {@const route = nav.route}
      <div class="route" class:fade={nav.motion === 'back'}>
        {#if route.view === 'browse'}
          <Browse plugin={route.plugin} item={route.item} />
        {:else if route.view === 'search'}
          <SearchView />
        {:else if route.view === 'sources'}
          <SourcesView />
        {:else if route.view === 'settings'}
          <Preferences
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

  <nav class="tabs" aria-label={t('Library sections')}>
    {#each TABS as [id, label, icon] (id)}
      <button class:on={nav.tab === id} aria-current={nav.tab === id ? 'page' : undefined} onclick={() => nav.switchTab(id)}>
        <Icon name={icon} /><span>{t(label)}</span>
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
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100dvh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: max(var(--jf-space-s5), env(safe-area-inset-top)) var(--jf-space-s3) var(--jf-space-s5);
    background: var(--jf-tint-clear);
    box-shadow: inset -1px 0 0 var(--jf-line-soft);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    padding: 0 var(--jf-space-s2) var(--jf-space-s5);
    font-weight: 700;
    font-size: 18px;
    letter-spacing: -0.01em;
  }
  .brand img {
    border-radius: 8px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-height: 44px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-size: 15px;
    font-weight: 600;
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
  }
  .nav-item.files {
    margin-top: auto;
  }
  button:focus-visible,
  a:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .content {
    min-width: 0;
    overflow-y: auto;
    padding: max(var(--jf-space-s7), env(safe-area-inset-top)) var(--jf-layout-gutter-fluid) 140px;
    outline: none;
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
  .dock {
    position: fixed;
    left: 232px;
    right: 0;
    bottom: max(var(--jf-space-s4), env(safe-area-inset-bottom));
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
    padding: 0 var(--jf-space-s4);
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

  /* Phones: tabs at the bottom, the mini-player above them. */
  @media (max-width: 760px) {
    .shell {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) auto;
    }
    .sidebar {
      display: none;
    }
    .content {
      padding: max(var(--jf-space-s5), env(safe-area-inset-top)) var(--jf-space-s4) 150px;
    }
    .dock {
      left: 0;
      bottom: calc(64px + env(safe-area-inset-bottom) + var(--jf-space-s2));
      padding: 0 var(--jf-space-s3);
    }
    .tabs {
      display: flex;
      justify-content: space-around;
      padding: 4px 4px max(4px, env(safe-area-inset-bottom));
      background: var(--jf-tint-veil);
      backdrop-filter: blur(var(--jf-glass-veil-blur)) saturate(var(--jf-glass-sat));
      box-shadow: inset 0 1px 0 var(--jf-line-soft);
    }
    .tabs button {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 2px;
      min-height: 56px;
      padding: 6px 0 0;
      border: 0;
      border-radius: var(--jf-radius-r-md);
      background: transparent;
      color: var(--jf-ink-50);
      font: inherit;
      font-size: 11px;
      font-weight: 600;
      cursor: pointer;
    }
    .tabs button.on {
      color: var(--jf-accent-text);
    }
  }
</style>
