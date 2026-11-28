// Right-click and long press open a row's or a card's menu, the same menu as its More button
// (docs/ui.md, "Rows and cards"). A Svelte action on the row or card:
//
// * a right-click opens the menu at the pointer, once the button is released: a menu shown while
//   the button is still down would close again at its release (the popover's light dismiss);
// * the keyboard's menu key (or Shift+F10) opens it under the element that has the focus;
// * a press of half a second on a touch screen opens it where the finger was, once the finger
//   lifts, and the tap that ends the press does not also play or open the item. Moving the finger
//   (scrolling) cancels it. Browsers that send their own `contextmenu` for a long press (Chrome on
//   Android) open it once;
// * the browser keeps its own menu on fields and links, and over selected text, so it can be
//   copied.

/**
 * Where a menu opened without its button goes (src/components/Menu.svelte, `openAt`): at a point,
 * a little away from a finger, or under an element.
 */
export type MenuPlace = { x: number; y: number; touch?: boolean } | { element: HTMLElement };

/** How long a finger rests before it is a long press. */
export const LONG_PRESS_MS = 500;
/** How far a finger may wander and still be pressing (beyond it, it scrolls). */
const SLOP_PX = 10;
/** A right-click this soon after its button went up came from it (Windows sends it then). */
const AFTER_RELEASE_MS = 1000;

/** Where the browser's own menu stays: fields, links, editable text. */
const OWN_MENU = 'input, textarea, select, a[href], [contenteditable]:not([contenteditable="false"])';

/** Whether a right-click on `node` should leave the browser's own menu. */
export function keepsBrowserMenu(event: Event, node: HTMLElement): boolean {
  const target = event.target instanceof Element ? event.target : null;
  if (target?.closest(OWN_MENU)) return true;
  const selection = typeof getSelection === 'function' ? getSelection() : null;
  return selection !== null && !selection.isCollapsed && selection.containsNode(node, true);
}

/** In the menu itself (its popover is inside the row or card): not a press on the item. */
function inMenu(event: Event): boolean {
  return event.target instanceof Element && event.target.closest('[popover]') !== null;
}

interface Press {
  id: number;
  x: number;
  y: number;
  touch: boolean;
  timer: ReturnType<typeof setTimeout> | null;
}

export function contextMenu(node: HTMLElement, open: (place: MenuPlace) => void) {
  let handler = open;
  let press: Press | null = null;
  /** A menu waiting for the press to end. */
  let pending: MenuPlace | null = null;
  /** When the last press ended (ms, the events' clock). */
  let releasedAt = -Infinity;
  /** The tap that ended a long press is not a tap on the item. */
  let swallow = false;

  function settle(place: MenuPlace) {
    // After the release's own events, so the menu stays open.
    setTimeout(() => handler(place), 0);
  }

  function end(event: PointerEvent) {
    if (!press || event.pointerId !== press.id) return;
    if (press.timer) clearTimeout(press.timer);
    press = null;
    releasedAt = event.timeStamp;
    removeEventListener('pointerup', end, true);
    removeEventListener('pointercancel', end, true);
    if (pending) {
      const place = pending;
      pending = null;
      settle(place);
    }
  }

  function down(event: PointerEvent) {
    swallow = false;
    if (inMenu(event)) return;
    pending = null;
    if (press?.timer) clearTimeout(press.timer);
    const touch = event.pointerType === 'touch' || event.pointerType === 'pen';
    press = { id: event.pointerId, x: event.clientX, y: event.clientY, touch, timer: null };
    if (touch && event.isPrimary) {
      const at = press;
      at.timer = setTimeout(() => {
        at.timer = null;
        if (press !== at || keepsBrowserMenu(event, node)) return;
        pending = { x: at.x, y: at.y, touch: true };
        swallow = true;
      }, LONG_PRESS_MS);
    }
    addEventListener('pointerup', end, true);
    addEventListener('pointercancel', end, true);
  }

  function move(event: PointerEvent) {
    if (!press?.timer || event.pointerId !== press.id) return;
    if (Math.hypot(event.clientX - press.x, event.clientY - press.y) > SLOP_PX) {
      clearTimeout(press.timer);
      press.timer = null;
    }
  }

  function menu(event: MouseEvent) {
    if (keepsBrowserMenu(event, node)) return;
    event.preventDefault();
    if (inMenu(event)) return;
    if (press) {
      // Still pressed: at the pointer, once released (a long press already waiting stays as is).
      if (press.timer) clearTimeout(press.timer);
      press.timer = null;
      pending ??= { x: event.clientX, y: event.clientY, touch: press.touch };
      if (press.touch) swallow = true;
    } else if (event.timeStamp - releasedAt < AFTER_RELEASE_MS) {
      handler({ x: event.clientX, y: event.clientY });
    } else {
      // The keyboard: under what has the focus.
      const focused = document.activeElement instanceof HTMLElement && node.contains(document.activeElement) ? document.activeElement : node;
      handler({ element: focused });
    }
  }

  function click(event: MouseEvent) {
    if (!swallow) return;
    swallow = false;
    event.preventDefault();
    event.stopPropagation();
  }

  node.addEventListener('pointerdown', down);
  node.addEventListener('pointermove', move);
  node.addEventListener('contextmenu', menu);
  node.addEventListener('click', click, true);
  return {
    update(next: (place: MenuPlace) => void) {
      handler = next;
    },
    destroy() {
      if (press?.timer) clearTimeout(press.timer);
      removeEventListener('pointerup', end, true);
      removeEventListener('pointercancel', end, true);
      node.removeEventListener('pointerdown', down);
      node.removeEventListener('pointermove', move);
      node.removeEventListener('contextmenu', menu);
      node.removeEventListener('click', click, true);
    },
  };
}
