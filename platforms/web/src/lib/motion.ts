// Motion from Misty's tokens for the movements Svelte runs (Now Playing opening and closing; the
// rest is CSS). Durations and curves are read from the tokens' custom properties, so they stay the
// tokens; with reduced motion (Theme sets data-motion) a movement becomes a short fade. Svelte
// samples these into Web Animations of opacity and transform, which the compositor runs.

import type { TransitionConfig } from 'svelte/transition';

type Easing = (t: number) => number;

/** A CSS cubic-bezier(x1, y1, x2, y2) as an easing function of t in 0..1. */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): Easing {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const x = (s: number) => ((ax * s + bx) * s + cx) * s;
  const y = (s: number) => ((ay * s + by) * s + cy) * s;
  const dx = (s: number) => (3 * ax * s + 2 * bx) * s + cx;
  return (t) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    // Newton's method, then bisection where the slope is too flat.
    let s = t;
    for (let i = 0; i < 8; i++) {
      const error = x(s) - t;
      if (Math.abs(error) < 1e-6) return y(s);
      const slope = dx(s);
      if (Math.abs(slope) < 1e-6) break;
      s -= error / slope;
    }
    let lo = 0;
    let hi = 1;
    s = t;
    for (let i = 0; i < 40 && hi - lo > 1e-7; i++) {
      if (x(s) < t) lo = s;
      else hi = s;
      s = (lo + hi) / 2;
    }
    return y(s);
  };
}

function token(name: string): string {
  if (typeof document === 'undefined') return '';
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/**
 * A duration token (`240ms`, or `.24s` as the CSS minifier writes it) or a motion token that names
 * one (`"Deeper 320 ms"`), in ms.
 */
export function durationOf(value: string, fallback: number): number {
  const match = /(\d+(?:\.\d+)?|\.\d+)\s*(ms|s)\b/.exec(value);
  if (!match) return fallback;
  return Number(match[1]) * (match[2] === 's' ? 1000 : 1);
}

/** A cubic-bezier() token as an easing function. */
export function easingOf(value: string, fallback: [number, number, number, number]): Easing {
  const numbers = /cubic-bezier\(([^)]+)\)/.exec(value)?.[1].split(',').map(Number);
  const [x1, y1, x2, y2] = numbers?.length === 4 && numbers.every(Number.isFinite) ? numbers : fallback;
  return cubicBezier(x1, y1, x2, y2);
}

export function reducedMotion(): boolean {
  return typeof document !== 'undefined' && document.documentElement.dataset.motion === 'reduced';
}

function fade(): TransitionConfig {
  return { duration: durationOf(token('--jf-motion-dur-1'), 160), css: (t) => `opacity: ${t}` };
}

type ViewTransitionDocument = Document & {
  startViewTransition?: (update: () => Promise<void>) => { ready: Promise<void>; finished: Promise<void> };
};

/** The view transition running, by the name `viewTransition` gave it. */
export function runningViewTransition(): string | undefined {
  return typeof document === 'undefined' ? undefined : document.documentElement.dataset.viewTransition;
}

/**
 * Runs `update` as a view transition called `name` (on the root as data-view-transition while it
 * runs, for the CSS that names what morphs), where the browser has them (same-document view
 * transitions: Chrome and Edge 111, Safari 18, Firefox 144) and motion is allowed; else runs it
 * straight away. The browser runs the transition on the compositor. Resolves when it is over.
 */
export function viewTransition(name: string, update: () => Promise<void>): Promise<void> {
  const doc = document as ViewTransitionDocument;
  if (typeof doc.startViewTransition !== 'function' || reducedMotion()) {
    return update();
  }
  const root = document.documentElement;
  root.dataset.viewTransition = name;
  const transition = doc.startViewTransition(update);
  // A transition the browser skips (another one started, or it could not capture the page)
  // still runs the update: the change happens without the morph. Its `ready` then rejects.
  transition.ready.catch(() => undefined);
  return transition.finished
    .catch(() => undefined)
    .finally(() => {
      if (root.dataset.viewTransition === name) delete root.dataset.viewTransition;
    });
}

/** Waits until `element`'s pictures have loaded, at most `ms` (polled: a view transition holds the frames). */
export async function picturesReady(element: Element | null, ms = 160): Promise<void> {
  const until = performance.now() + ms;
  while (element && performance.now() < until) {
    const images = [...element.querySelectorAll('img')];
    if (images.length > 0 && images.every((img) => img.complete)) return;
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
}

/**
 * A new picture fades in over the last one, which fades out under it (artwork changes: dur-3
 * Fluid; reduced motion: a short fade). Opacity only.
 */
export function crossfade(_node: Element): TransitionConfig {
  if (reducedMotion()) return fade();
  return {
    duration: durationOf(token('--jf-motion-dur-3'), 380),
    easing: easingOf(token('--jf-motion-ease-fluid'), [0.22, 1, 0.36, 1]),
    css: (t) => `opacity: ${t}`,
  };
}

/**
 * Now Playing opens ("Deeper", player.open-now-playing, Fluid): it rises 24 px from 98.5 % and fades
 * in; inside a view transition the browser moves it instead (the artwork grows from the player's).
 */
export function deeper(_node: Element): TransitionConfig {
  if (runningViewTransition() === 'now-playing') return { duration: 0 };
  if (reducedMotion()) return fade();
  return {
    duration: durationOf(token('--jf-player-open-now-playing'), 320),
    easing: easingOf(token('--jf-motion-ease-fluid'), [0.22, 1, 0.36, 1]),
    css: (t, u) => `opacity: ${t}; transform: translateY(${(u * 24).toFixed(2)}px) scale(${(1 - u * 0.015).toFixed(4)})`,
  };
}

/**
 * Now Playing closes ("Back", player.close-now-playing, Exit): it sinks 24 px and fades out. (Svelte
 * makes it inert meanwhile, and lively again if it reopens before it is gone.)
 */
export function back(_node: Element): TransitionConfig {
  if (reducedMotion()) return fade();
  return {
    duration: durationOf(token('--jf-player-close-now-playing'), 300),
    easing: easingOf(token('--jf-motion-ease-exit'), [0.4, 0, 1, 1]),
    css: (t, u) => `opacity: ${t}; transform: translateY(${(u * 24).toFixed(2)}px) scale(${(1 - u * 0.015).toFixed(4)})`,
  };
}
