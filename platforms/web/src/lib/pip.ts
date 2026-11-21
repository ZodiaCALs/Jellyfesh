// Document Picture-in-Picture (WICG spec; Chromium 116 and later on desktops): a small window of
// the page's own, which the browser keeps above other windows. Jellyfesh Web shows Desktop Lyrics
// in it. Opening one needs a click or a key (transient user activation); the window shares the
// page's script, so the page draws into its document, which starts empty: the page's styles and
// root attributes (theme, motion) are copied in and kept in step.

interface DocumentPictureInPictureApi extends EventTarget {
  requestWindow(options?: { width?: number; height?: number; disallowReturnToOpener?: boolean }): Promise<Window>;
  readonly window: Window | null;
}

function api(): DocumentPictureInPictureApi | null {
  if (typeof window === 'undefined') return null;
  return (window as unknown as { documentPictureInPicture?: DocumentPictureInPictureApi }).documentPictureInPicture ?? null;
}

/** Whether this browser offers Document Picture-in-Picture windows. */
export function pipSupported(): boolean {
  return api() !== null;
}

/** Whether a click or key a moment ago lets the page open a window now. */
export function activationActive(): boolean {
  const activation = (navigator as unknown as { userActivation?: { isActive: boolean } }).userActivation;
  return activation?.isActive ?? false;
}

/** Copies the page's style sheets (links and style elements) into `to`, replacing earlier copies. */
function copyStyles(from: Document, to: Document) {
  for (const old of to.head.querySelectorAll('[data-jf-copied]')) old.remove();
  for (const element of from.head.querySelectorAll<HTMLLinkElement | HTMLStyleElement>('link[rel="stylesheet"], style')) {
    let copy: HTMLElement;
    if (element instanceof HTMLLinkElement) {
      const link = to.createElement('link');
      link.rel = 'stylesheet';
      link.href = element.href;
      copy = link;
    } else {
      copy = to.createElement('style');
      copy.textContent = element.textContent;
    }
    copy.dataset.jfCopied = '';
    to.head.append(copy);
  }
}

/** Copies the root element's attributes (data-appearance, data-motion, lang, style…). */
function copyRoot(from: Document, to: Document) {
  const source = from.documentElement;
  const target = to.documentElement;
  for (const name of target.getAttributeNames()) if (!source.hasAttribute(name)) target.removeAttribute(name);
  for (const name of source.getAttributeNames()) target.setAttribute(name, source.getAttribute(name) ?? '');
}

/**
 * Opens a Picture-in-Picture window `width`×`height` CSS px with the page's styles, or null where
 * the browser has none or refuses (no click or key just before, for one). `onclose` runs when the
 * window closes, however it closes.
 */
export async function openPip(width: number, height: number, onclose: (win: Window) => void): Promise<Window | null> {
  const pip = api();
  if (!pip) return null;
  let win: Window;
  try {
    win = await pip.requestWindow({ width, height });
  } catch {
    return null;
  }
  const doc = win.document;
  // The window's own page: no margins or scrolling, the page's ground and type. (Only here: the
  // same rules in a component's styles would reach the page itself.)
  const base = doc.createElement('style');
  base.textContent =
    'html,body{height:100%;margin:0;overflow:hidden}' +
    'body{background:var(--jf-cloud,Canvas);color:var(--jf-ink-90,CanvasText);font-family:var(--jf-font-sans,system-ui,sans-serif)}';
  doc.head.append(base);
  copyStyles(document, doc);
  copyRoot(document, doc);
  // The theme rewrites its style element; settings change the root's attributes.
  const styles = new MutationObserver(() => copyStyles(document, doc));
  styles.observe(document.head, { childList: true, subtree: true, characterData: true });
  const root = new MutationObserver(() => copyRoot(document, doc));
  root.observe(document.documentElement, { attributes: true });
  win.addEventListener(
    'pagehide',
    () => {
      styles.disconnect();
      root.disconnect();
      onclose(win);
    },
    { once: true },
  );
  return win;
}
