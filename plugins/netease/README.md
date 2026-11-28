# NetEase Cloud Music for Jellyfesh

Plays your NetEase Cloud Music playlists through NetEase's open platform,
with your own developer credentials. The plugin (`org.jellyfesh.netease`)
ships inside every Jellyfesh app and is also published as a separate
`.jfpkg`, signed with the Jellyfesh release key. Setup and troubleshooting:
`docs/plugins/netease.md`.

**It has not yet been tested against the live service**: NetEase's API
documentation was not reachable when it was written, so it follows what the
official command-line client (`@music163/ncm-cli` 0.1.7) was found to send,
and was tested against a mock server built from that. Search, and the songs
of an album or an artist, follow what independent public clients of the open
platform send. Some answers (the playlist lists, the profile, token renewal,
search, an album's and an artist's songs) are not documented anywhere we
could read, and may differ.

## What it does

* **Browse** your created and collected playlists and your liked songs (the
  red-heart playlist; 0.0.6 listed it as Starred Playlists and, reading the
  answer as a list, showed nothing), with covers where NetEase names them.
* **Search** songs, albums, artists and playlists (from Jellyfesh v0.0.9).
  An album opens to its songs and an artist to their popular songs, and so
  do Go to Album and Go to Artist on a song that names them.
* **Discover** (plugin API 1.2): Liked Songs, Created Playlists and Collected
  Playlists. **Details** of a playlist: description, picture, plays, tags
  and creation date (`playlist/detail/get/v2`, read with the field names the
  official client reads).
* **Play** at the streaming quality chosen in Jellyfesh: 128 kbit/s (Low and
  Normal), 320 kbit/s (High) or NetEase's lossless files (Lossless), where
  your account may play them.
* **Library** (plugin API 1.3): your liked songs on Favourites, and your
  created and collected playlists on Playlists.
* **Respects NetEase's rules**: VIP songs and songs from digital albums show
  as needing a purchase, songs NetEase may not play here as unavailable, and
  songs of which only a preview is free are not played at all (Jellyfesh
  does not play previews). Free full listens play in full.

Not yet: recommendations, comments and lyrics, whose paths were not found,
and details of albums and artists. NetEase Cloud Music is not yet offered in
Add a Source, whose list of services is Jellyfesh's own: sign in from the
installed plugins in the Sources settings. There is no Open in NetEase Cloud
Music either: a song has an encrypted id for the API and a numeric one for
links, and Jellyfesh's links take one id.

## What you need

* A NetEase Cloud Music account (VIP for VIP songs).
* **Your own developer registration**: register as an individual developer
  on NetEase Cloud Music's open platform
  (<https://developer.music.163.com/st/developer/apply/account?type=INDIVIDUAL>)
  with that account, and take the **App ID** and **private key** from the
  console. The account that signs in must be the one that registered.

Jellyfesh has no NetEase credentials of its own: the requests are signed
with a private key, and a shared key would be in every copy of the app.

## Signing in

In Sources, open **NetEase Cloud Music** and sign in. Enter the App ID and
paste the private key (with or without its `BEGIN` and `END` lines). Then
scan the QR code with the NetEase Cloud Music app and confirm there. The
code works for five minutes.

## What is stored where

| What | Where |
| --- | --- |
| Your App ID, private key and the sign-in tokens | your system's keystore |
| A random id for this installation (sent to NetEase as the device id) | the plugin's small private storage |

The private key is stored once, when you enter it, and never read back by
the plugin: Jellyfesh signs each request with it from the keystore. Requests
go only to `openncm.music.163.com`, and audio comes from NetEase's servers
(`*.music.126.net`). Jellyfesh sends `127.0.0.1` as the client address
instead of looking up your public IP with outside services, and does not
report plays. Signing out deletes the tokens, the App ID and the key from
this device.

## Known limits

* NetEase limits plays per day and requests per moment; Jellyfesh shows
  NetEase's message when a limit is reached.
* When Jellyfesh cannot read one of NetEase's answers (it differs from what
  the research found), it says so instead of showing an empty list, for
  example "NetEase Cloud Music didn't say which playlist holds your liked
  songs.", and logs the answer's shape: its field names and the types of
  their values, never the values.
* Stream addresses outside `*.music.126.net` are refused by Jellyfesh's
  sandbox; whether NetEase serves audio from elsewhere is not known.
* NetEase's terms for its open platform could not be read: whether
  personal credentials may be used in a player like Jellyfesh is unverified,
  and nothing known in them keeps its search results apart, so they show
  with the library's and other sources' (Jellyfesh's default).
