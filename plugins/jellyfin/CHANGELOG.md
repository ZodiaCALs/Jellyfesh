# Changelog: Jellyfin plugin

## 0.0.8 — unreleased

* With Jellyfesh v0.0.8: plugin API 1.3 (the package needs Jellyfesh
  v0.0.8).
* The **Streaming Quality** setting is gone; streams use the level
  Jellyfesh asks for (its one streaming quality, ADR-0026), and the manifest
  lists the levels the quality menu offers (lossless, high, normal, low).
  Jellyfesh keeps a quality set here once, as its own. The conversion
  format and device name stay. Still `/Audio/{id}/universal` with
  `transcodingProtocol=http`.
* **Library** (ADR-0024): favourite songs, albums and artists for
  Favourites (`isFavorite=true`, in name order: Jellyfin keeps no date for
  a favourite) and audio playlists for Playlists.
* **Download** (ADR-0025): the original file from `/Items/{id}/Download`,
  which keeps the account's `EnableContentDownloading` policy, with its size
  and name. High, Normal and Low convert a file above 320, 192 or
  128 kbit/s (or in a format Jellyfesh cannot play) to Opus, only once
  `CanDownload` says the account may download it. Never
  `/Audio/{id}/stream?static=true`, which skips the policy. An account
  without the policy is told the server does not allow downloads.
* Tests: a golden table of what each level asks of Jellyfin and gets back
  for FLAC, MP3, AAC, WMA and Opus files, and a check that no
  `/Users/{userId}/…` route (obsolete since Jellyfin 10.9) is used.

## 0.0.7 — 2026-11-19

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). **Discover**: Recently Added (`/Items/Latest`, songs grouped into
  albums), Continue Listening (`/UserItems/Resume`), Favourites and
  Suggestions (`/Items/Suggestions`). **Details**: an item's overview,
  genres, release date, labels (`Studios`) and the composers, lyricists,
  arrangers and producers among its people, with its picture.

## 0.0.6 — 2026-11-17

* With Jellyfesh v0.0.6: built with plugin SDK 1.1. The manifest still
  declares API 1.0, so the package also installs in Jellyfesh v0.0.5. No
  change in behaviour.

## 0.0.5 — 2026-11-15

* With Jellyfesh v0.0.5: the settings and sign-in labels are in Title Case,
  as in every app ("Server Address", "User Name", "Device Name").

## 0.0.3 — 2026-11-09

* First release, with Jellyfesh v0.0.3: sign-in with Quick Connect or user
  name and password (only the access token kept), search, browse (music
  libraries, playlists, favourites), streaming by quality, cover art, lyrics,
  instant mix, and logout reported to the server; for Jellyfin 10.9 and newer.
