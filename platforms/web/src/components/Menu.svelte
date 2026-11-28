<script lang="ts" module>
  export interface MenuItem {
    label: string;
    action: () => void;
    disabled?: boolean;
    /** A check item (Desktop Lyrics): whether it is on. */
    checked?: boolean;
  }
</script>

<script lang="ts">
  // A "More" menu: a popover (light dismiss, Esc) placed next to its button, growing out of it;
  // arrow keys move. Its items can be given as a function, which runs when it opens (a long list's
  // rows then make no menus until one is opened). `openAt` opens the same menu without the button:
  // at the pointer for a right-click or a long press, under an element for the keyboard's menu key
  // (docs/ui.md, "Rows and cards"). Closing it returns the focus to where the person was.
  import { tick } from 'svelte';
  import type { MenuPlace } from '../lib/contextMenu';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

  let {
    items,
    label = t('More'),
    icon = 'more',
  }: { items: MenuItem[] | (() => MenuItem[]); label?: string; icon?: string } = $props();
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let opened = $state.raw<MenuItem[]>([]);
  const shown = $derived(typeof items === 'function' ? opened : items);
  /** Where the focus goes back to when the menu closes: the button, or what was right-clicked. */
  let back: HTMLElement | null = null;

  /** Next to the button, its end edge on the button's, below it where it fits. */
  function placeByButton() {
    if (!button || !menu) return;
    const box = button.getBoundingClientRect();
    const width = menu.offsetWidth;
    const height = menu.offsetHeight;
    const left = Math.min(innerWidth - width - 8, Math.max(8, box.right - width));
    const below = box.bottom + 4 + height <= innerHeight - 8;
    menu.style.left = `${left}px`;
    menu.style.top = `${below ? box.bottom + 4 : Math.max(8, box.top - height - 4)}px`;
    // It grows out of the button's corner.
    menu.style.transformOrigin = `${box.right - left}px ${below ? 0 : height}px`;
  }

  /** At a point (below it where it fits, else above), or under an element's start corner. */
  function placeAt(place: MenuPlace) {
    if (!menu) return;
    const width = menu.offsetWidth;
    const height = menu.offsetHeight;
    let x: number;
    let low: number;
    let high: number;
    if ('element' in place) {
      const box = place.element.getBoundingClientRect();
      x = box.left;
      low = box.bottom + 4;
      high = box.top - 4;
    } else {
      // A finger would hide what is right under it.
      const gap = place.touch ? 12 : 0;
      x = place.x;
      low = place.y + gap;
      high = place.y - gap;
    }
    const left = Math.min(innerWidth - width - 8, Math.max(8, x));
    const below = low + height <= innerHeight - 8;
    const top = below ? low : Math.max(8, high - height);
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;
    // It grows out of the point.
    menu.style.transformOrigin = `${Math.min(width, Math.max(0, x - left))}px ${below ? 0 : height}px`;
  }

  async function show(place: MenuPlace | null) {
    if (!menu) return;
    if (typeof items === 'function') {
      opened = items();
      await tick();
    }
    if (shown.length === 0) return;
    menu.showPopover();
    if (place) placeAt(place);
    else placeByButton();
    menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
  }

  async function toggle(event: MouseEvent) {
    event.stopPropagation();
    if (!menu) return;
    if (open) menu.hidePopover();
    else {
      back = button ?? null;
      await show(null);
    }
  }

  /**
   * Opens the menu without its button: at `place`, with the same items. The focus returns to
   * `returnTo` (what was right-clicked or long-pressed) when it closes.
   */
  export async function openAt(place: MenuPlace, returnTo: HTMLElement | null = null) {
    if (!menu) return;
    if (open) menu.hidePopover();
    back = returnTo ?? button ?? null;
    await show(place);
  }

  /** Closes it; the focus goes back to where it was. */
  function close() {
    menu?.hidePopover();
    const to = back?.isConnected ? back : button;
    to?.focus({ preventScroll: true });
  }

  function keys(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
      return;
    }
    const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])];
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

  function run(item: MenuItem) {
    close();
    item.action();
  }
</script>

<button
  class="more"
  bind:this={button}
  aria-label={label}
  title={label}
  aria-haspopup="menu"
  aria-expanded={open}
  onclick={toggle}
>
  <Icon name={icon} />
</button>
<div
  class="menu"
  role="menu"
  tabindex="-1"
  popover="auto"
  bind:this={menu}
  ontoggle={(event) => (open = (event as ToggleEvent).newState === 'open')}
  onkeydown={keys}
>
  {#each shown as item (item.label)}
    {#if item.checked === undefined}
      <button role="menuitem" disabled={item.disabled} onclick={() => run(item)}>{item.label}</button>
    {:else}
      <button class="check" role="menuitemcheckbox" aria-checked={item.checked} disabled={item.disabled} onclick={() => run(item)}>
        <span class="mark" aria-hidden="true">{#if item.checked}<Icon name="check" size={16} />{/if}</span>{item.label}
      </button>
    {/if}
  {/each}
</div>

<style>
  .more {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    cursor: pointer;
  }
  .more:hover {
    background: var(--jf-fill-hover);
    color: var(--jf-ink-90);
  }
  .more:focus-visible,
  .menu button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 200px;
    padding: 6px;
    border: 0;
    border-radius: var(--jf-radius-r-lg);
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow:
      inset 0 0 0 1px var(--jf-line-soft),
      var(--jf-elevation-shadow-3);
  }
  /* Popovers scale from 96 % and fade in (dur-2 Fluid); with reduced motion they only fade. */
  .menu:popover-open {
    animation: jf-pop-in var(--jf-motion-dur-2) var(--jf-motion-ease-fluid);
  }
  :global(:root[data-motion='reduced']) .menu:popover-open {
    animation: jf-fade-in var(--jf-motion-dur-1) linear;
  }
  .menu button {
    display: block;
    width: 100%;
    min-height: 40px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: calc(14px * var(--jf-text-scale, 1));
    text-align: start;
    cursor: pointer;
  }
  .menu button:hover:not(:disabled) {
    background: var(--jf-fill-hover);
  }
  .menu .check {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
    padding-inline-start: var(--jf-space-s2);
  }
  .mark {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    color: var(--jf-accent-text);
  }
  .menu button:disabled {
    color: var(--jf-ink-30);
    cursor: default;
  }
</style>
