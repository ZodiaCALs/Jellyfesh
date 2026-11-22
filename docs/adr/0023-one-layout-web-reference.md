# ADR-0023: One layout in every app: Jellyfesh Web's

* Status: **Accepted**
* Date: 2026-11-22
* Amends: [`../ui.md`](../ui.md) (its opening rule, Navigation, Settings and
  Allowed differences). ADR-0001 stands: no business logic in the apps.

## Context

For v0.0.8 the owner asked (2026-11-22) to "make the UI and layout same for
all platforms, which unifies all to the web view", to make the player bar
float on all pages, and for a floating window on the desktop.

Asked how, the owner chose **native apps that look and lay out like Jellyfesh
Web**, not the web UI shown in a webview. The rule against webview UIs
stays: the apps stay light and keep their native media keys, menus and
accessibility.

Until now `ui.md` let each app keep its platform's look and mechanics:
* the macOS Settings window and sidebar groups;
* the Windows navigation pane;
* GNOME's main menu and preferences dialog;
* Android's bottom tabs and a page for each source;
* the web's floating sidebar and phone tabs.

The player bar already floats in every app, but:
* it is absent from the macOS Settings window and the Linux Settings dialog;
* macOS keeps it under Now Playing while Windows hides it;
* Android's narrow capsule has only Play;
* the web bar has no device button or More.

Jellyfesh Web has no Playlists, Favourites or history.

## Decision

1. **Jellyfesh Web is the reference layout.** Every app draws its structure
   with its own toolkit (SwiftUI, WinUI 3, GTK 4 with libadwaita, Jetpack
   Compose), at the web's breakpoints:
   * **From 761 px (or dp) wide:** the floating sidebar panel, and pages
     with the web's titles, grids, rows and cards.
     * The panel holds the wordmark, then Home, Albums, Artists, Songs,
       Playlists, Favourites and Downloads, a separator, Search and Sources.
     * Open Files… and Settings sit at its foot.
   * **Up to 760 px:** the web's phone layout: tabs for Home, Library,
     Search, Sources and Settings. The Library tab switches between Albums,
     Artists, Songs, Playlists, Favourites and Downloads.
     * Android phones use this layout.
     * Android tablets and wide windows use the sidebar layout.
   * **Settings is a page in the main window in every app,** with the web's
     sections and groups.
     * On macOS, ⌘, and the app menu open that page.
     * About Jellyfesh is in Settings › About everywhere; the macOS app menu
       item opens it there.
2. **One player bar, floating over every page, Settings included.** It is
   the web's dock: the full bar from 900 px wide, the capsule below that.
   * It hides only while Now Playing is open, which has its own dock.
   * Every app shows the same controls in the same order: the web's, plus
     the device button and More (now on the web too), the quality menu and
     the mini player button (ADR-0026).
3. **The same components and measures.** The design tokens set page titles,
   artwork radii, row heights and spacing; where an app and the web differ,
   the web is right. Platform materials stay as `ui.md`'s Luminous section
   describes them (the macOS sidebar material, Mica and Acrylic, GTK
   `backdrop-filter`, Android blur).
4. **What stays native:**
   * window frames and title-bar controls;
   * the macOS menu bar and Dock menu;
   * system file pickers and share sheets;
   * notifications and media controls (Now Playing on macOS, SMTC, MPRIS,
     MediaSession);
   * accessibility APIs and scrolling physics;
   * the system font, which the web's font stack already uses;
   * keyboard conventions beyond the shared keys.
5. **Web first.** Each new v0.0.8 page or control is built in Jellyfesh Web
   before the native apps:
   * Favourites and Playlists, with the sources' likes and playlists
     (ADR-0024);
   * Recently Played on Home;
   * search by source (ADR-0024);
   * the quality menu and the mini player (ADR-0026).

   Screenshots of every reference page (light and dark, wide and phone) are
   committed under `docs/screenshots/web/`; they are the specification the
   native apps match. Downloads (ADR-0025) has no web page: a browser keeps
   no offline library, so Download there saves the file.
6. **`ui.md` is rewritten around this:**
   * the opening rule;
   * one navigation table with the wide and narrow layouts;
   * the new pages;
   * one Settings page, with Streaming in every app;
   * a much shorter list of allowed differences.

## Consequences

* Large UI changes in the four native apps:
  * macOS loses its Settings window and sidebar groups;
  * Linux loses its preferences dialog and most of its main menu;
  * Windows replaces the navigation pane with the sidebar panel;
  * Android phones get the web's five tabs.
* In places this departs from Apple's, Microsoft's and GNOME's guidelines
  (Settings in the main window). The owner chose one Jellyfesh over that.
* Parity reviews compare each app with the reference screenshots.

## Tests

* Web: end-to-end tests for each new page. A script regenerates the
  reference screenshots against the mock servers.
* Linux: the Xvfb screenshot tests cover the new layout and are compared
  with the references by structure, not by pixels.
* Each app's view-model and logic tests follow the moved places. No rule or
  list moves into an app (ADR-0001).
