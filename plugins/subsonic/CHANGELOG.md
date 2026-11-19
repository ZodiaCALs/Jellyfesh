# Changelog: Subsonic plugin

## 0.0.7 — 2026-11-19

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). **Discover**: the server's album lists Recently Added, Most
  Played, Recently Played and Random Albums as shelves (`getAlbumList2`).
  **Details**: album notes (`getAlbumInfo2`) with version, genres, release
  date and label; artist biographies (`getArtistInfo2`); a song's genre,
  year and its composer, lyricist, arranger and producer (OpenSubsonic
  contributors); playlist descriptions; each with the server's cover.

## 0.0.6 — 2026-11-17

* With Jellyfesh v0.0.6: built with plugin SDK 1.1. The manifest still
  declares API 1.0, so the package also installs in Jellyfesh v0.0.5. No
  change in behaviour.

## 0.0.5 — 2026-11-15

* With Jellyfesh v0.0.5: the settings and sign-in labels are in Title Case,
  as in every app ("Server Address", "User Name", "API Key"), and the hints
  name the "Enter API Key" button.

## 0.0.3 — 2026-11-09

* First release, with Jellyfesh v0.0.3: sign-in with an OpenSubsonic API key
  or a salt and token pair (never the password), search, browse (artists,
  album lists, playlists, starred), streaming by quality, cover art, synced
  and plain lyrics, similar songs; for servers speaking the Subsonic API
  1.13.0 or newer.
