// Desktop Lyrics on the web (docs/ui.md "Lyrics, word by word"): the line being sung and the next
// one in a Document Picture-in-Picture window, which the browser keeps above other windows, where
// it offers one (Chromium on desktops). It follows the Desktop Lyrics setting: turning the setting
// on opens the window (the click lets the page open one); closing the window, from its own button
// or the browser's, turns the setting off, as closing the apps' window does. A browser opens such a
// window only right after a click or key, so a page that opens with the setting on waits: the
// player bar then offers Show Desktop Lyrics, rather than taking the next click on the page,
// which may be one that needs it (picking folders again, for one).

import { mount, unmount } from 'svelte';
import DesktopLyrics from '../../nowplaying/DesktopLyrics.svelte';
import type { App } from '../app';
import { activationActive, openPip, pipSupported } from '../pip';

export const DESKTOP_LYRICS_SIZE = { width: 640, height: 136 };

export class DesktopLyricsWindow {
  /** The window is open. */
  shown = $state(false);
  /** The setting is on but the window waits for a click (Show Desktop Lyrics). */
  waiting = $state(false);
  readonly supported = pipSupported();
  private app: App | null = null;
  private win: Window | null = null;
  private view: ReturnType<typeof mount> | null = null;
  private opening = false;

  attach(app: App) {
    this.app = app;
    this.follow(app.settings.value.desktop_lyrics);
  }

  /** The setting changed (or the page opened): open or close the window. */
  follow(on: boolean) {
    if (!on || !this.supported) {
      this.waiting = false;
      this.close();
      return;
    }
    if (this.win || this.opening) return;
    if (activationActive()) void this.show();
    else this.waiting = true;
  }

  /** Opens the window (call from a click). */
  async show() {
    const app = this.app;
    if (!app || this.win || this.opening || !this.supported) return;
    this.opening = true;
    const win = await openPip(DESKTOP_LYRICS_SIZE.width, DESKTOP_LYRICS_SIZE.height, (closing) => this.closed(closing));
    this.opening = false;
    if (!win) {
      this.waiting = app.settings.value.desktop_lyrics;
      return;
    }
    if (!app.settings.value.desktop_lyrics) {
      // Turned off while the browser opened it.
      win.close();
      return;
    }
    this.win = win;
    this.waiting = false;
    this.shown = true;
    this.view = mount(DesktopLyrics, { target: win.document.body, props: { app, win, onclose: () => this.turnOff() } });
  }

  /** Close Desktop Lyrics: the setting goes off, which closes the window. */
  turnOff() {
    const app = this.app;
    if (app?.settings.value.desktop_lyrics) app.settings.update((d) => (d.desktop_lyrics = false));
    else this.close();
  }

  private close() {
    const win = this.win;
    this.unmountView();
    this.win = null;
    this.shown = false;
    win?.close();
  }

  /** The window closed, however it closed: the setting follows it. */
  private closed(win: Window) {
    if (win !== this.win) return;
    this.unmountView();
    this.win = null;
    this.shown = false;
    if (this.app?.settings.value.desktop_lyrics) this.app.settings.update((d) => (d.desktop_lyrics = false));
  }

  private unmountView() {
    if (this.view) void unmount(this.view);
    this.view = null;
  }
}
