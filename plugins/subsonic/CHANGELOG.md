# Changelog: Subsonic plugin

## 0.0.8 — unreleased

* With Jellyfesh v0.0.8: plugin API 1.3 (the package needs Jellyfesh
  v0.0.8).
* The **Streaming Quality** setting is gone: streams use the level
  Jellyfesh asks for (its one streaming quality, ADR-0026), and the manifest
  lists the levels the quality menu offers (lossless, high, normal, low).
  Jellyfesh keeps a quality set here once, as its own. The conversion format
  stays.
* **Library** (ADR-0024): starred songs, albums and artists for Favourites,
  newest star first (`getStarred2`), and the playlists the server lists for
  the account for Playlists: its own, and other accounts' public ones as
  followed (`getPlaylists`).
* **Download** (ADR-0025): always through `download`, never `stream`. The
  original file (`format=raw`, with its size and name), or on Navidrome,
  whose `download` also converts, MP3 or Opus at 320, 192 or 128 kbit/s for
  High, Normal and Low when the file is above that. An account without the
  download role (Navidrome with downloads turned off) is told the server
  does not allow downloads.
* Tests: a golden table of what each level asks of the server for FLAC,
  MP3, AAC and WMA files.

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
