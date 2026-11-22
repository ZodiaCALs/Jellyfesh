# ADR-0026: Streaming quality in the bar, and the mini player window

* Status: **Accepted**
* Date: 2026-11-22
* Amends: ADR-0022 §6 (floating windows), [`../ui.md`](../ui.md) (Settings ›
  Playback › Streaming, the player bar)

## Context

For v0.0.8 the owner asked (2026-11-22) for "sound quality options in the
bar" and "options of enabling a floating window on the os". We read these as
a streaming quality menu and an optional mini player that stays above other
windows, and told the owner so.

The v0.0.8 survey found the following.

**Quality:**
* The plugin API already has `playback.quality` (low, normal, high,
  lossless) and `stream-info` codec and bitrate hints.
* The native core always asks for lossless ("there is no quality setting
  yet") and drops the hints. Only Jellyfesh Web has Streaming Quality, in
  Settings.
* The Jellyfin plugin's own quality setting overrides whatever the app asks
  for.
* The core already knows what is playing: `NowPlaying.format` (the decoded
  codec, rate, depth and bitrate) and `NowPlaying.quality`, its label
  (Hi-Res Lossless, Lossless, High, Standard).

**Floating windows:** Desktop Lyrics already floats on four platforms:
* macOS: a non-activating `NSPanel` at `.floating` level;
* Windows: an always-on-top tool window;
* Linux: `_NET_WM_STATE_ABOVE` on X11, a normal window on Wayland;
* Chromium: Document Picture-in-Picture.

Windows also has an always-on-top compact player (Ctrl+M) that no other app
has. Platform facts were checked on 2026-11-22 (MDN's browser compatibility
data 8.1.10, the Android picture-in-picture guide, GTK and
wayland-protocols).

## Decision

1. **Streaming Quality, one setting in the core** (`Settings.streaming_quality`):
   * The values are **Auto**, **Lossless**, **High**, **Normal** and
     **Low**; the last four map to the API's enum.
   * **Auto** means Lossless, except on a connection the platform reports as
     metered (Android, and Windows when it says so), where it means High.
     Jellyfesh Web keeps stepping down when the browser cannot play a
     format.
   * The core passes the level to `resolve-stream` and keeps the codec and
     bitrate hints for the format line.
   * Changing the level reopens the playing song at the same position when
     its source can seek. Otherwise the change applies from the next song.
2. **What each source offers.** A plugin lists the levels it supports in its
   manifest (`content_policy.qualities`). The core gives the apps the levels
   for what is playing (`NowPlaying.qualities`).
   * Library files and remote playback (Spotify) offer none.
   * The plugins' own quality settings go, so the bar's menu never checks
     one level while another plays. Jellyfin, Subsonic, NetEase and the
     community NetEase and QQ Music plugins each had one ("app", or a fixed
     level).
   * Migration: the core reads the old values from the stored settings
     before the new schema drops them.
     * If every plugin that set a fixed level agrees, that level becomes the
       app's level.
     * Otherwise the app keeps Auto.
3. **The quality menu in the bar**, on every app and the web:
   * The bar shows a badge with the playing quality's label (the core's
     `QualityLabel`).
   * Choosing the badge opens the menu: the levels the source offers, the
     chosen one checked, and the format line ("FLAC · 24-bit · 96 kHz",
     "AAC · 256 kbit/s").
   * With no levels it shows the format line and why: "Files play as they
     are." or "Quality is set in {Service}."
   * Settings › Playback › Streaming has the same setting in every app (no
     longer web-only).
4. **The mini player.** A small window above other windows, opened with the
   bar's mini player button or **Open Mini Player** (Ctrl+M or ⌘M where
   free). It shows:
   * the cover, the title and the artist;
   * previous, play and next;
   * a progress line;
   * optionally the lyrics bar's current line.

   Settings › Appearance › Mini Player has **Keep on Top** (on) and **Open
   When the Window Is Closed** (off: closing the main window leaves the mini
   player while music plays). Each platform builds on what Desktop Lyrics
   already does:
   * **macOS:** an `NSPanel` at `.floating` level, on every Space.
   * **Windows:** the compact player becomes this mini player
     (always-on-top presenter).
   * **Linux:** an undecorated window, above other windows on X11.
     * Wayland lets no app keep its own window on top: GTK 4 has no
       keep-above, and xdg-shell has no such state.
     * There the mini player is a normal window, and **Keep on Top…** opens
       the compositor's window menu (`gdk_toplevel_show_window_menu`),
       where the person can choose Always on Top if their desktop offers
       it.
     * No layer-shell in v0.0.8: GNOME lacks it, Sway hides it from
       Flatpak, and the crate needs a newer Rust than the workspace. An
       allowed difference.
   * **Jellyfesh Web:** Document Picture-in-Picture: Chrome and Edge 116 and
     later, Firefox 151 and later, desktops only. Elsewhere (Safari, phones)
     the button is hidden.
   * **Android:** picture-in-picture when leaving the app while music plays.
     * It needs **Mini Player in Picture-in-Picture** in Settings, which is
       off by default: Android documents picture-in-picture for video,
       calls and navigation.
     * The controls come from the MediaSession, with seamless resize off.
     * Never the overlay permission (`SYSTEM_ALERT_WINDOW`).

   Desktop Lyrics and the mini player can be open together.
5. **Where it lives.** The level, the offered levels and the mini player
   settings are in the core and its settings. Windows, panels and menus are
   app code with no rules of their own (ADR-0001).

## Consequences

* Every official and community plugin that streams declares its levels;
  Jellyfin loses its own quality setting.
* The phone layout's bar has less room: the badge is shown in the full bar
  and in Now Playing, not in the capsule.

## Tests

* Plugins: level-to-request mapping for Jellyfin, Subsonic, NetEase and QQ
  Music, with recorded responses.
* Core: the level reaches `resolve-stream`; Auto on a metered connection;
  reopening at the same position; hints kept; offered levels for library
  files, remote playback and plugins.
* FFI contract tests for the new settings and `NowPlaying` fields.
* Apps: view-model tests for the badge and the menu's levels. The mini player
  opens and closes in each app's smoke test where the platform allows.
