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
* Two questions about Spotify's developer terms are open: whether a
  player like Jellyfesh may sign people in through its own development app
  or their own,
  and how the rule against products "integrated with streams or content from
  another service" applies to a player with several sources. Until Spotify
  answers, Spotify stays in its own views: its results are not mixed into
  searching everything, its tracks never join Jellyfesh's queue, nothing is
  kept in the library, and no synced lyrics show while it plays.

## Before you start

* **Spotify Premium.** Spotify accepts playback commands (play, pause,
  skip, volume…) only from Premium accounts. Without it you can still search
  and browse.
* **A seat in Jellyfesh's Spotify app.** Release builds sign in through one
  Spotify app that the Jellyfesh owner runs, so you only sign in with your
  Spotify account. Spotify lets such an app (in Development Mode) serve only
  a few accounts that its owner adds by hand, so ask for yours to be added.
  If there is no seat for you, or your build has no built-in app, use your
  own app instead (below).

## Set it up

1. In Sources, choose **Spotify** in Add a Source, then **Sign In**.
2. Allow Jellyfesh on the Spotify page that opens in your browser. Spotify
   sends the browser back to `http://127.0.0.1:29817/callback`, which
   Jellyfesh listens on during the sign-in (the port must be free).

If Spotify refuses your account ("Jellyfesh's Spotify app has no place for
this account yet"), the app has no seat for you: use your own app.

### Your own Spotify app (optional)

1. Open the Spotify Developer Dashboard (developer.spotify.com/dashboard)
   and create an app for the **Web API**. It stays in Development Mode.
2. Add this **Redirect URI** exactly: `http://127.0.0.1:29817/callback`
3. In the app's user management, add the Spotify account you will use.
4. Copy the **Client ID** (32 letters and digits) into the Spotify source's
   setting **Your Own Client ID** (Settings › Sources › Installed ›
   Spotify), then sign in again. Builds without a built-in app ask for it
   during the sign-in instead. Jellyfesh uses no client secret.

## Settings

* **Your Own Client ID** (optional): the Client ID of your own Spotify app.
  Leave it empty to sign in through Jellyfesh's.

## For whoever publishes Jellyfesh: the built-in app

1. Create one Spotify app as above (Web API, Redirect URI
   `http://127.0.0.1:29817/callback`), with a Spotify account that has
   Premium (Spotify requires it of the app's owner).
2. In its user management, add each person who will sign in (Development
   Mode allows five; more needs Spotify's extended quota, which Spotify
   grants only to organisations).
3. Add its Client ID to the GitHub repository as the Actions secret
   **`JF_SPOTIFY_CLIENT_ID`**. The release workflow builds it into the
   Spotify plugin (`option_env!` at compile time); a Client ID is public, so
   it is safe in the package. Without the secret the plugin asks each person
   for their own Client ID, as before.

## What you can do

* **Play on your devices**: choose a device; play an album, playlist,
  artist, Liked Songs or a list of tracks from any track; pause, resume,
  skip, seek, volume, shuffle and repeat; move playback to another device.
  Now Playing follows what Spotify plays, even when you change it in
  Spotify.
* **Search** tracks, artists, albums and playlists, ten per type at a time.
* **Browse** Your Playlists, Saved Albums and Liked Songs, an album's tracks
  and an artist's albums. Spotify lists a playlist's songs only when you own
  it or collaborate on it (one of its 2026 changes); a playlist you only
  follow says "Spotify lists this playlist's songs only in Spotify." It
  still plays on your device as a whole, and **Open in Spotify** opens it.
* **Favourites and Playlists** (Jellyfesh v0.0.8): your Liked Songs, Saved
  Albums and the artists you follow, and your playlists, each in a Spotify
  section of its own. Following artists needs the permission
  `user-follow-read`, which Jellyfesh asks for from v0.0.8: if you signed in
  before, sign out and in again to see the artists you follow.
* **Discover** (Jellyfesh v0.0.7): Recently Played, Your Top Tracks and Your
  Top Artists of the last four weeks, Your Playlists and Saved Albums. These
  are the lists Spotify still lets apps like yours read: since its 2024 and
  February 2026 changes, new releases, featured playlists, categories,
  recommendations and an artist's top tracks are closed to apps in
  Development Mode. If you signed in before v0.0.7, sign out and in again to
  allow Recently Played and Top (Spotify asks for two more permissions);
  until then those shelves are left out.
* **Get Info** (v0.0.7) on an album shows its release date, kind and
  copyright lines; on a song its release date and ISRC; on a playlist its
  description and owner; with the largest picture. Spotify has no artist
  biographies, labels or genres for these apps, and no comments.
* **Covers** load from Spotify's picture servers, including uploaded
  playlist covers and mosaics.
* **Open in Spotify** on every track, album, artist and playlist.

## Signing out

Signing out deletes the tokens (and a Client ID you typed at sign-in) on
this device. To revoke
Jellyfesh's access completely, remove the app from your Spotify account's
apps, or delete it in the dashboard. Spotify ends a sign-in after six months
at the latest; Jellyfesh then says "Sign in to Spotify again."

## When something goes wrong

| Message | What to do |
| --- | --- |
| That is not a Client ID | Copy the 32 letters and digits from your app's settings. |
| Port 29817 on this device is in use by another program … | Close that program (or a sign-in still waiting in another window) and try again. |
| Spotify refused the sign-in / does not accept this Client ID | Check that the Redirect URI is exactly `http://127.0.0.1:29817/callback`, that your account is added as a user of the app, and the Client ID. |
| Spotify Premium is needed to control playback. | Commands need Premium; browsing and searching do not. |
| No Spotify device is active … | Open Spotify on one of your devices, or choose a device in Jellyfesh. |
| Sign in to Spotify again. | Spotify ended the sign-in: it does after six months, or when access was revoked or the app removed. Sign in again. |
| Spotify lists this playlist's songs only in Spotify. | A playlist you follow but neither own nor collaborate on: play it as a whole, or open it in Spotify. |
| Too many requests (the source pauses) | Spotify's rate limit for your app was reached; Jellyfesh waits as long as Spotify asks. |

Devices that Spotify marks as not accepting remote commands are not listed.

## Privacy

The plugin talks only to Spotify: `accounts.spotify.com` for sign-in,
`api.spotify.com` for everything else, and Spotify's picture servers. The
Client ID and tokens stay in your system's keystore.
