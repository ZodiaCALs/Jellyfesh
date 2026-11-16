<script lang="ts" module>
  export interface MenuItem {
    label: string;
    action: () => void;
    disabled?: boolean;
  }
</script>

<script lang="ts">
  // A "More" menu: a popover (light dismiss, Esc) placed next to its button, growing out of it;
  // arrow keys move.
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './Icon.svelte';

  let { items, label = t('More') }: { items: MenuItem[]; label?: string } = $props();
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let open = $state(false);

  function place() {
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

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    if (!menu) return;
    if (open) menu.hidePopover();
    else {
      menu.showPopover();
      place();
      menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    }
  }

  function keys(event: KeyboardEvent) {
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
    menu?.hidePopover();
    button?.focus();
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
  <Icon name="more" />
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
  {#each items as item (item.label)}
    <button role="menuitem" disabled={item.disabled} onclick={() => run(item)}>{item.label}</button>
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
    font-size: 14px;
    text-align: start;
    cursor: pointer;
  }
  .menu button:hover:not(:disabled) {
    background: var(--jf-fill-hover);
  }
  .menu button:disabled {
    color: var(--jf-ink-30);
    cursor: default;
  }
</style>
