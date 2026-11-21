# Changelog: NetEase Cloud Music (Unofficial) plugin

## 0.1.2 — 2026-11-20

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). Its own version line continues (0.1.x), not Jellyfesh's.
* **Word-by-word lyrics**: lyrics come from `/song/lyric/v1`, asking for
  NetEase's word timing (`yv`, `ytv`; `/song/lyric` when that fails); YRC
  becomes enhanced LRC (`src/yrc.rs`) with the YRC translation under each
  line, checked by golden tests (`tests/golden/yrc/`). Not confirmed against
  the live service.
* **Discover**: Daily Picks (signed in), Recommended Playlists and Charts.
  **Details** of songs, albums, artists and playlists. **Comments** on
  songs, albums and playlists, hot ones first. Recommended playlists' covers
  (`picUrl`).

## 0.1.1 — 2026-11-17

* Bundled with Jellyfesh v0.0.6 as a community plugin: unsigned (never
  signed with the Jellyfesh release key), installed only once community
  plugins are allowed, and off until its disclosure is accepted. Moved
  from its own repository into the Jellyfesh repository
  (`plugins/community/netease`); the plugin works as in 0.1.0. Still not
  tested against the live service.

## 0.1.0 — 2026-11-16

* First version, in a separate repository, installed from a file: sign-in
  with the web player's QR code (scanned with the NetEase Cloud Music
  app), search, liked songs, playlists, albums and artists, streams at the
  chosen quality (whole tracks only: no previews, no paywall or region
  bypass), synced lyrics with translations, and artwork. Tested only
  against a mock service with invented data.
