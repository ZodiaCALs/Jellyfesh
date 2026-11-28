# Changelog: NetEase Cloud Music plugin

## Unreleased

* **Search** (ADR-0027 §5): songs, albums, artists and playlists, each kind
  on the path that independent public clients of the open platform call
  (`search/song/get/v3`, `search/album/get/v2`, `search/artists/get/v2`,
  `search/playlist/get/v2`) with `keyword`, `limit` and `offset`; the cursor
  is the offset. NetEase's terms could not be read, so `unified_search`
  keeps its default. An album opens to its songs (`album/song/list/get/v2`),
  an artist to their popular songs (`artists/hotsong/list/get/v2`; its
  parameter `artistId` is inferred from the artist detail's). Songs keep
  their album and first artist (`al:` and `ar:` before NetEase's ids) for Go
  to Album and Go to Artist.
* **Liked songs**: an answer that names no playlist is the error "NetEase
  Cloud Music didn't say which playlist holds your liked songs." instead of
  an empty Favourites, and so are songs NetEase says are there but does not
  send in a form the plugin can read. The playlist needs only its id (`id`,
  `playlistId` or `encryptedId`, also under `playlist` or first in a list).
* Every list the plugin cannot read is an error in plain words, logged with
  the answer's shape (field names and value types, never values); Discover
  leaves such a shelf out. Pages end where NetEase's `recordCount` says.
* Not yet tested against the live service.

## 0.0.8 — unreleased

* With Jellyfesh v0.0.8: plugin API 1.3 (the package needs Jellyfesh
  v0.0.8).
* The **Streaming Quality** setting is gone (it was the only setting):
  streams use the level Jellyfesh asks for (ADR-0026), and the manifest
  lists the levels the quality menu offers. Low and Normal both ask NetEase
  for 128 kbit/s, its lowest; High for 320, Lossless for 999. Jellyfesh
  keeps a quality set here once, as its own.
* **Library** (ADR-0024): the liked songs (the red-heart playlist) for
  Favourites, and the created playlists, then the collected ones as
  followed, for Playlists, with one cursor across both lists. The open
  platform has no liked albums or artists.

## 0.0.7 — 2026-11-20

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). **Discover**: Liked Songs, Created Playlists and Collected
  Playlists. **Details** of playlists (`playlist/detail/get/v2`:
  `describe`, `coverImgUrl`, `playCount`, `tags`, `createTime`).
* Liked songs: `playlist/star/get/v2` answers with one playlist (the
  red-heart playlist), not a list, as NetEase's official client reads it;
  the folder Starred Playlists is now Liked Songs and lists its songs.
* Lists read `recordCount` for their size and `creatorNickName` for the
  creator, as the official client does; songs carry a cover where a list
  names one (`al.picUrl`, `album.picUrl` or `coverImgUrl`); NetEase's code
  404 is "not found".

## 0.0.6 — 2026-11-16

* First release, with Jellyfesh v0.0.6 (plugin API 1.1): sign-in with your
  own open-platform App ID and RSA private key (requests signed by Jellyfesh
  from your system's keystore) and a QR code for the NetEase Cloud Music
  app; your created, collected and starred playlists; streams at 128 or
  320 kbit/s or lossless, with VIP songs and digital albums marked as
  needing a purchase and previews not played. No search yet. Not yet tested
  against the live service.
