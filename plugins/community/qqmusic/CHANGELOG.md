# Changelog: QQ Music (Unofficial) plugin

## 0.1.3 — unreleased

* With Jellyfesh v0.0.8: plugin API 1.3 (the package needs Jellyfesh
  v0.0.8).
* The **Streaming Quality** setting is gone: streams use the level
  Jellyfesh asks for (ADR-0026), from the levels the manifest lists for the
  quality menu (FLAC, 320 and 128 kbit/s MP3, 96 kbit/s AAC, best first down from the level). Jellyfesh keeps a quality set here once, as
  its own. Translated Lyrics stays.
* **Library** (ADR-0024): Liked Songs (folder 201) for Favourites, and My Playlists for Playlists. A first page of liked songs asks
  QQ Music again; liked albums and artists are not offered yet.

## 0.1.2 — 2026-11-20

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). Its own version line continues (0.1.x), not Jellyfesh's.
* **Discover**: Recommended Playlists and Charts (a chart lists its songs
  for its current period). **Details** of songs, albums, playlists and
  charts. No singer details, no comments and no word-timed lyrics (QQ Music
  encrypts those; the plugin does not decrypt them). Not yet tested against
  the live service.

## 0.1.1 — 2026-11-17

* Bundled with Jellyfesh v0.0.6 as a community plugin: unsigned (never
  signed with the Jellyfesh release key), installed only once community
  plugins are allowed, and off until its disclosure is accepted. Moved
  from its own repository into the Jellyfesh repository
  (`plugins/community/qqmusic`); the plugin works as in 0.1.0. Still not
  tested against the live service.

## 0.1.0 — 2026-11-16

* First version, in a separate repository, installed from a file: sign-in
  with WeChat's web QR code, search, liked songs, playlists, albums and
  singers, streams at the chosen quality or below (whole tracks only: no
  previews, no paywall or region bypass), synced lyrics with translations,
  and artwork. Tested only against mock services with invented data.
