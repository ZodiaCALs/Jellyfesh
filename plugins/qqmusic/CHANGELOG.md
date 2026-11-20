# Changelog: QQ Music plugin

## 0.0.7 — 2026-11-20

* With Jellyfesh v0.0.7: plugin API 1.2 (the package needs Jellyfesh
  v0.0.7). **Discover**: Daily 30 (`/discover/daily-mix`), Guess You Like
  (`/discover/radio`), AI Playlists (`/discover/ai-playlists`), the first
  chart's songs and the charts (`/charts`), each with See All. **Details**
  of playlists (`/playlists/detail`: description, cover) and charts
  (`/charts/detail`: description, header picture, listens, last update).
  Pictures load from `y.gtimg.cn` and `qpic.y.qq.com`, as https.

## 0.0.6 — 2026-11-16

* First release, with Jellyfesh v0.0.6 (plugin API 1.1): sign-in with your
  own QQ Music API key (`qmk-…`), search (songs, singers, albums,
  playlists), browse (Daily 30, Guess You Like, AI playlists, charts,
  playlists) and personal recommendations through QQ Music's open API
  (`qqmusic-skills` protocol 0.0.3). Nothing plays in Jellyfesh: the API has
  no audio, and every item opens in QQ Music. Not yet tested against the
  live service.
