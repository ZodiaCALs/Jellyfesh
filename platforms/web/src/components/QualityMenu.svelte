<script lang="ts">
  // The quality badge and its menu (ADR-0026 §3, docs/ui.md "The player bar and the mini
  // player"). The badge shows the playing quality's label (the core's, as in Get Info). Its menu
  // has the format line ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"), then the levels the core
  // offers for what plays with the chosen one checked; with none, why: "Files play as they are."
  // or "Quality is set in {Service}.". A new level applies at once when the song can seek, and from
  // the next song otherwise (the player decides). The same setting as Settings › Playback ›
  // Streaming.
  import { tick } from 'svelte';
  import { useApp } from '../lib/app';
  import type { StreamingQuality } from '../lib/core/types';
  import { formatLine, qualityText } from '../lib/details';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

  const app = useApp();
  const { player, settings } = app;
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let open = $state(false);

  function levelName(level: StreamingQuality): string {
    switch (level) {
      case 'auto':
        return t('Auto');
      case 'lossless':
        return t('Lossless');
      case 'high':
        return t('High');
      case 'normal':
        return t('Normal');
      case 'low':
        return t('Low');
    }
  }
  const quality = $derived(player.qualityMenu);
  const offered = $derived(quality.qualities);
  const chosen = $derived(settings.value.streaming_quality);
  const label = $derived(player.format?.quality ? qualityText(player.format.quality) : levelName(chosen));
  const line = $derived(player.format ? formatLine(player.format) : '');
  const why = $derived.by(() => {
    const note = quality.note;
    if (offered.length > 0 || !note) return null;
    return note.kind === 'set_in_service' ? t('Quality is set in {service}.', { service: note.service }) : t('Files play as they are.');
  });

  function place() {
    if (!button || !menu) return;
    const box = button.getBoundingClientRect();
    const width = menu.offsetWidth;
    const height = menu.offsetHeight;
    const left = Math.min(innerWidth - width - 8, Math.max(8, box.left + box.width / 2 - width / 2));
    const above = box.top - height - 8 >= 8;
    menu.style.left = `${left}px`;
    menu.style.top = `${above ? box.top - height - 8 : box.bottom + 8}px`;
    menu.style.transformOrigin = `${box.left + box.width / 2 - left}px ${above ? height : 0}px`;
  }

  async function toggle(event: MouseEvent) {
    event.stopPropagation();
    if (!menu) return;
    if (open) {
      menu.hidePopover();
      return;
    }
    menu.showPopover();
    await tick();
    place();
    (menu.querySelector<HTMLButtonElement>('[aria-checked="true"]') ?? menu.querySelector<HTMLButtonElement>('button'))?.focus();
  }

  function choose(level: StreamingQuality) {
    menu?.hidePopover();
    button?.focus();
    if (level !== chosen) settings.update((d) => (d.streaming_quality = level));
  }

  function keys(event: KeyboardEvent) {
    const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    if (buttons.length === 0) return;
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next = -1;
    if (event.key === 'ArrowDown') next = (at + 1) % buttons.length;
    else if (event.key === 'ArrowUp') next = (at - 1 + buttons.length) % buttons.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = buttons.length - 1;
    else return;
    event.preventDefault();
    buttons[next]?.focus();
  }
</script>

<button
  class="badge"
  bind:this={button}
  aria-haspopup="menu"
  aria-expanded={open}
  aria-label={t('Quality: {label}', { label })}
  title={t('Streaming Quality')}
  onclick={toggle}
>
  {label}
</button>
<div
  class="menu"
  role="menu"
  aria-label={t('Streaming Quality')}
  tabindex="-1"
  popover="auto"
  bind:this={menu}
  ontoggle={(event) => (open = (event as ToggleEvent).newState === 'open')}
  onkeydown={keys}
>
  {#if line}<p class="format">{line}</p>{/if}
  {#if offered.length > 0}
    <p class="heading" aria-hidden="true">{t('Streaming Quality')}</p>
    {#each offered as level (level)}
      <button role="menuitemradio" aria-checked={level === chosen} onclick={() => choose(level)}>
        <span class="mark" aria-hidden="true">{#if level === chosen}<Icon name="check" size={16} />{/if}</span>{levelName(level)}
      </button>
    {/each}
  {:else if why}
    <p class="why">{why}</p>
  {/if}
</div>

<style>
  .badge {
    flex: none;
    max-width: 132px;
    min-height: 28px;
    padding: 0 10px;
    overflow: hidden;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-size: calc(12px * var(--jf-text-scale, 1));
    font-weight: 700;
    letter-spacing: 0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  .badge:hover {
    background: var(--jf-fill-hover);
  }
  .badge:focus-visible,
  .menu button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 220px;
    max-width: min(320px, calc(100vw - 16px));
    padding: 6px;
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-3);
  }
  .menu:popover-open {
    animation: jf-pop-in var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) .menu:popover-open {
    animation: jf-fade-in var(--jf-motion-dur-1) linear;
  }
  p {
    margin: 0;
    padding: 8px var(--jf-space-s3);
  }
  .format {
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-90);
    font-variant-numeric: tabular-nums;
  }
  .heading {
    padding-bottom: 4px;
    font-size: calc(12px * var(--jf-text-scale, 1));
    font-weight: 700;
    color: var(--jf-ink-70);
  }
  .why {
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
    line-height: 1.4;
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    width: 100%;
    min-height: 40px;
    padding: 0 var(--jf-space-s3) 0 var(--jf-space-s2);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: calc(14px * var(--jf-text-scale, 1));
    text-align: start;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--jf-fill-hover);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    color: var(--jf-accent-text);
  }
</style>
