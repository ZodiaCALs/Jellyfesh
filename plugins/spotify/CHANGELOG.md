# Changelog: Spotify plugin

## 0.0.7 — 2026-11-20

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). **Discover**: Recently Played (`/me/player/recently-played`),
  Your Top Tracks and Your Top Artists (`/me/top/…`, last four weeks), Your
  Playlists and Saved Albums, only endpoints the Web API reference does not
  mark deprecated; a shelf Spotify refuses is left out. **Details**: albums
  (release date, type, copyrights), songs (release date, ISRC), playlists
  (description as plain text, owner) and artists (picture). New scopes
  `user-read-recently-played` and `user-top-read`.
* Covers: pictures from any host of `*.scdn.co` and `*.spotifycdn.com`
  (uploaded playlist covers come from `image-cdn-*.spotifycdn.com`).

## 0.0.6 — 2026-11-16

* First release (beta), with Jellyfesh v0.0.6 (plugin API 1.1): remote
  control of your own Spotify devices through the Web API with the Client ID
  of your own Spotify app (Authorization Code with PKCE, redirect
  `http://127.0.0.1:29817/callback`), search, your playlists, saved albums
  and liked songs, albums and artists, and Open in Spotify. Not yet tested
  against the live service.
* Sign in with just your Spotify account: release builds carry the Client
  ID of Jellyfesh's own Spotify app (the `JF_SPOTIFY_CLIENT_ID` secret at
  build time); the setting **Your Own Client ID** still uses your own app,
  and an account the built-in app has no seat for is told why.
