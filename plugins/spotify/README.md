# Spotify for Jellyfesh (beta)

Controls Spotify on your own devices from Jellyfesh: your phone, computer,
speakers. Spotify plays the music itself; Jellyfesh is the remote. The
plugin (`org.jellyfesh.spotify`) ships inside every Jellyfesh app and is
also published as a separate `.jfpkg`, signed with the Jellyfesh release key.
Setup and troubleshooting: `docs/plugins/spotify.md`.

**Beta.** It has not yet been tested against the live service, only against
a mock server written from the Spotify Web API reference. Two questions
about Spotify's developer terms are still open and unanswered by Spotify:
whether people may each use the Client ID of their own app in a player like
Jellyfesh, and how the rule against products "integrated with streams or
content from another service" applies to a player with several sources.
Spotify stays in its own views in Jellyfesh until they are answered.

## What it does

* **Remote control**: pick a device, play an album, playlist, artist, your
  liked songs or any list of tracks from any track, pause, resume, skip,
  seek, volume, shuffle, repeat, and move playback to another device. Now
  Playing follows what Spotify plays.
* **Search** tracks, artists, albums and playlists.
* **Browse** your playlists, saved albums and liked songs, an album's tracks
  and an artist's albums.
* **Open in Spotify** on every track, album, artist and playlist.

Spotify items never join Jellyfesh's queue, are not kept in the library,
and show no synced lyrics, as Spotify's terms ask.

## What you need

* **Spotify Premium**: Spotify accepts playback commands only from Premium
  accounts. Without it you can browse and search, and every command says
  "Spotify Premium is needed to control playback."
* **Your own Spotify app** (free): Spotify lets a development app serve only a
  handful of people, so Jellyfesh cannot share one. In the Spotify Developer
  Dashboard (developer.spotify.com/dashboard), create an app for the Web API
  in Development Mode, add your own Spotify account as a user, and add this
  Redirect URI exactly: `http://127.0.0.1:29817/callback`. Copy its
  **Client ID** (32 letters and digits). No client secret is used.

## Signing in

In Sources, choose **Spotify**, then **Sign In**. Paste the Client ID, then
allow Jellyfesh in the browser page that opens; Spotify sends the browser
back to Jellyfesh on port 29817 of this device. If another program uses that
port, the sign-in says so: close it and try again.

## What is stored where

| What | Where |
| --- | --- |
| Your Client ID and the sign-in tokens | your system's keystore |
| How long Spotify asked Jellyfesh to wait after "too many requests" | the plugin's small private storage |
| Spotify's tracks, albums and playlists | nowhere: they are not kept in the library |

The tokens go only to Spotify (`accounts.spotify.com`, `api.spotify.com`).
Spotify ends a sign-in after six months at the latest; Jellyfesh then asks
you to sign in again. Signing out deletes the tokens and the Client ID from
this device; to revoke access completely, remove Jellyfesh's app from your
Spotify account's apps page or delete the app in the dashboard.

## Known limits

* Spotify does not publish its rate limits. Jellyfesh asks for the playback
  state about once a second while something plays and waits as long as
  Spotify says after "too many requests"; heavy use can still hit the
  limit of your app.
* Devices that refuse remote commands (Spotify marks them restricted) are
  not listed.
* Podcasts and local files are not shown.
* A list of tracks is sent 100 at a time around the one you start.
