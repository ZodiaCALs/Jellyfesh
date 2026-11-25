# Changelog: NetEase Cloud Music plugin

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
