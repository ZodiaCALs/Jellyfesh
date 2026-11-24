// The mini player on the web (ADR-0026 §4, docs/ui.md "The player bar and the mini player"): a
// small window above other windows with the cover, the title and the artist, previous, play and
// next, a progress line and the line being sung. It is a Document Picture-in-Picture window, which
// browsers offer on desktops only (Chrome and Edge 116+, Firefox 151+); elsewhere (Safari,
// phones) its button is hidden and Ctrl+M does nothing. A page has one such window, so opening
// the mini player closes Desktop Lyrics and the other way round (docs/ui.md, Allowed differences).
// It opens from a click or a key (the browser's rule) and closes from its own button.

import { mount, unmount } from 'svelte';
import MiniPlayerView from '../../nowplaying/MiniPlayerView.svelte';
import type { App } from '../app';
import { openPip, pipSupported } from '../pip';

export const MINI_PLAYER_SIZE = { width: 420, height: 148 };

export class MiniPlayerWindow {
  /** The window is open. */
  shown = $state(false);
  readonly supported = pipSupported();
  private app: App | null = null;
  private win: Window | null = null;
  private view: ReturnType<typeof mount> | null = null;
  private opening = false;

  attach(app: App) {
    this.app = app;
  }

  /** Opens the window, or closes it when it is open (call from a click or a key). */
  async toggle() {
    if (this.win) this.close();
    else await this.open();
  }

  async open() {
    const app = this.app;
    if (!app || this.win || this.opening || !this.supported) return;
    this.opening = true;
    const win = await openPip(MINI_PLAYER_SIZE.width, MINI_PLAYER_SIZE.height, (closing) => this.closed(closing));
    this.opening = false;
    if (!win) return;
    this.win = win;
    this.shown = true;
    this.view = mount(MiniPlayerView, { target: win.document.body, props: { app, win, onclose: () => this.close() } });
  }

  close() {
    const win = this.win;
    this.unmountView();
    this.win = null;
    this.shown = false;
    win?.close();
  }

  /** The window closed, however it closed (its button, the browser's, or Desktop Lyrics opening). */
  private closed(win: Window) {
    if (win !== this.win) return;
    this.unmountView();
    this.win = null;
    this.shown = false;
  }

  private unmountView() {
    if (this.view) void unmount(this.view);
    this.view = null;
  }
}
