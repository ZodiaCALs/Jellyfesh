# Subsonic for Jellyfesh

Plays the music on your own Subsonic or OpenSubsonic server: Navidrome,
Gonic, Airsonic, LMS, Ampache and others. The plugin
(`org.jellyfesh.subsonic`) ships inside every Jellyfesh app and is also
published as a separate `.jfpkg`, signed with the Jellyfesh release key.
Setup, server preparation and troubleshooting: `docs/plugins/subsonic.md`.

## What it does

* **Search** artists, albums and tracks (`search3`).
* **Browse** artists → albums → tracks, album lists (recently added, recently
  played, most played, random, A to Z, by artist), playlists and starred
  songs, albums and artists.
* **Play** with the quality you choose: the original file when Jellyfesh can
  play its format within the quality's bit-rate limit (lossless: any
  bit rate), otherwise converted by the server to MP3, Opus or AAC at
  320, 192 or 96 kbit/s.
* **Cover art** at the size the screen needs, **lyrics** (synced lyrics from
  OpenSubsonic servers as timed lines, plain lyrics otherwise) and
  **similar songs** when the server offers them.

It works with servers that speak the Subsonic API 1.13.0 or newer. Your
server may be on your home network; Jellyfesh lets this plugin reach that one
address and nothing else.

## Signing in

First enter your server's address in the source's settings (the address you
open the server at, such as `http://192.168.1.20:4533`). Then:

* **API key** (servers with the OpenSubsonic API-key extension): create a key
  in the server's settings and paste it. This is the safer choice and the
  only one offered when the server supports keys.
* **User name and password** (other servers): Jellyfesh turns your password
  into one token (`md5(password + salt)`) on your device, checks it with the
  server and forgets the password. The token works like a password for this
  server only.

## What is stored where

| What | Where |
| --- | --- |
| The API key, or your user name with the salt and token, tied to the server address | your system's keystore (Keychain, Credential Manager, Secret Service, Android Keystore) |
| What the server supports, and whether your account may stream (checked daily) | the plugin's small private storage in Jellyfesh's data folder |
| Server address, quality, conversion format | the plugin's settings |

Your password is never stored, by the plugin or by Jellyfesh. A sign-in is
only ever sent to the address it was made for. Subsonic has no other way to
sign in a stream or cover request, so those URLs carry the API key or token
in their query; they are handed to the player in memory and are not written
to disk.

Signing out deletes the stored sign-in. A Subsonic sign-in is not a session
on the server; to make a key stop working everywhere, revoke it in the
server's settings.
