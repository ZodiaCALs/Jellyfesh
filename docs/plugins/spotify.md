# Spotify (beta)

The Spotify plugin turns Jellyfesh into a remote for Spotify on your own
devices: Spotify plays the music on your phone, computer or speaker, and
Jellyfesh shows and controls it. It comes with every Jellyfesh app and
updates with it; the same plugin is also published as a separate download
(`org.jellyfesh.spotify`, signed with the Jellyfesh release key). What it
stores and where: `plugins/spotify/README.md`.

## Beta, and why

* It was tested against a mock server written from the Spotify Web API
  reference, **not yet against the live service**.
* Two questions about Spotify's developer terms are open: whether each
  person may use the Client ID of their own app in a player like Jellyfesh,
  and how the rule against products "integrated with streams or content from
  another service" applies to a player with several sources. Until Spotify
  answers, Spotify stays in its own views: its results are not mixed into
  searching everything, its tracks never join Jellyfesh's queue, nothing is
  kept in the library, and no synced lyrics show while it plays.

## Before you start

* **Spotify Premium.** Spotify accepts playback commands (play, pause,
  skip, volume…) only from Premium accounts. Without it you can still search
  and browse.
* **Your own Spotify app.** Spotify lets a development app serve only a
  handful of people, so each person uses their own:
  1. Open the Spotify Developer Dashboard (developer.spotify.com/dashboard)
     and create an app for the **Web API**. It stays in Development Mode.
  2. Add this **Redirect URI** exactly:
     `http://127.0.0.1:29817/callback`
  3. In the app's user management, add the Spotify account you will use.
  4. Copy the **Client ID** (32 letters and digits). Jellyfesh uses no client
     secret.

## Set it up

1. In Sources, choose **Spotify** in Add a Source, then **Sign In**.
2. Paste the Client ID. (Jellyfesh asks only the first time; signing out
   forgets it.)
3. Allow Jellyfesh on the Spotify page that opens in your browser. Spotify
   sends the browser back to `http://127.0.0.1:29817/callback`, which
   Jellyfesh listens on during the sign-in.

There are no settings.

## What you can do

* **Play on your devices**: choose a device; play an album, playlist,
  artist, Liked Songs or a list of tracks from any track; pause, resume,
  skip, seek, volume, shuffle and repeat; move playback to another device.
  Now Playing follows what Spotify plays, even when you change it in
  Spotify.
* **Search** tracks, artists, albums and playlists, ten per type at a time.
* **Browse** Your Playlists, Saved Albums and Liked Songs, an album's tracks
  and an artist's albums.
* **Open in Spotify** on every track, album, artist and playlist.

## Signing out

Signing out deletes the tokens and the Client ID on this device. To revoke
Jellyfesh's access completely, remove the app from your Spotify account's
apps, or delete it in the dashboard. Spotify ends a sign-in after six months
at the latest; Jellyfesh then asks you to sign in again.

## When something goes wrong

| Message | What to do |
| --- | --- |
| That is not a Client ID | Copy the 32 letters and digits from your app's settings. |
| Port 29817 on this device is in use by another program … | Close that program (or a sign-in still waiting in another window) and try again. |
| Spotify refused the sign-in / does not accept this Client ID | Check that the Redirect URI is exactly `http://127.0.0.1:29817/callback`, that your account is added as a user of the app, and the Client ID. |
| Spotify Premium is needed to control playback. | Commands need Premium; browsing and searching do not. |
| No Spotify device is active … | Open Spotify on one of your devices, or choose a device in Jellyfesh. |
| Sign in again. | The sign-in expired or was revoked. |
| Too many requests (the source pauses) | Spotify's rate limit for your app was reached; Jellyfesh waits as long as Spotify asks. |

Devices that Spotify marks as not accepting remote commands are not listed.

## Privacy

The plugin talks only to Spotify: `accounts.spotify.com` for sign-in,
`api.spotify.com` for everything else, and Spotify's picture servers. The
Client ID and tokens stay in your system's keystore.
