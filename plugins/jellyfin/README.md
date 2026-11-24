# Jellyfin for Jellyfesh

Plays the music on your own Jellyfin server (10.9 or newer, including the
12.x releases). The plugin (`org.jellyfesh.jellyfin`) ships inside every
Jellyfesh app and is also published as a separate `.jfpkg`, signed with the
Jellyfesh release key. Setup, server preparation and troubleshooting:
`docs/plugins/jellyfin.md`.

## What it does

* **Search** tracks, albums, artists and playlists.
* **Browse** your music libraries (artists → albums → tracks, all albums,
  recently added), your playlists and your favourite songs, albums and
  artists. With several music libraries, each one is a folder.
* **Play** at the streaming quality chosen in Jellyfesh: the original file
  when Jellyfesh can play its format within the quality's bit-rate limit
  (lossless: any bit rate), otherwise converted by Jellyfin to AAC, MP3 or
  Opus at 320, 192 or 96 kbit/s.
* **Cover art** at the size the screen needs, **lyrics** (Jellyfin 10.9 and
  later; synced lyrics as timed lines) and **instant mix** suggestions for
  a track, album, artist or playlist.
* **Discover** (plugin API 1.2): Recently Added, Continue Listening,
  Favourites and Suggestions. **Details**: an item's overview, genres,
  release date, labels and the people on a song.

Your server may be on your home network; Jellyfesh lets this plugin reach
that one address and nothing else.

## Signing in

First enter your server's address in the source's settings (the address you
open Jellyfin at, such as `http://192.168.1.20:8096`). Then:

* **Quick Connect** (no password typed in Jellyfesh): Jellyfesh shows a
  six-digit code; in Jellyfin, open your profile, choose Quick Connect and
  enter it. The code works for ten minutes.
* **User name and password**: sent once to your server, which answers with an
  access token; the password is not kept.

## What is stored where

| What | Where |
| --- | --- |
| The access token, tied to the server address | your system's keystore (Keychain, Credential Manager, Secret Service, Android Keystore) |
| A random device id, so Jellyfin sees one device across sign-ins | the plugin's small private storage in Jellyfesh's data folder |
| Server address, conversion format, device name | the plugin's settings |

Your password is never stored, by the plugin or by Jellyfesh. The token is
only ever sent to the address it was made for, in the `Authorization` header
of each request; it never appears in a URL. Cover art needs no sign-in, so
cover URLs carry none.

Signing out tells Jellyfin the session ended (the token stops working on
the server too) and deletes the stored token.
