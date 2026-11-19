# Changelog: Jellyfin plugin

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
