# QQ Music for Jellyfesh

Finds music in QQ Music with your own API key and opens it in QQ Music. The
plugin (`org.jellyfesh.qqmusic`) ships inside every Jellyfesh app and is
also published as a separate `.jfpkg`, signed with the Jellyfesh release key.
Setup and troubleshooting: `docs/plugins/qqmusic.md`.

It speaks QQ Music's official open API as the `qqmusic-skills` repository
(protocol version 0.0.3) documents it. **It has not yet been tested against
the live service**: only against a mock server written from that
documentation.

## What it does

* **Search** songs, singers, albums and playlists.
* **Browse** Daily 30, Guess You Like, AI Playlists, and the charts (by
  group, then each chart's songs); open a playlist to see its songs.
* **For You**: Daily 30, then Guess You Like.
* **Open in QQ Music** on every song, album, singer and playlist
  (`https://i2.y.qq.com/a/…` pages, which the API documentation gives).

Nothing plays in Jellyfesh: QQ Music's open API returns information and
links, never audio, so every item says "QQ Music's open API does not stream;
open it in QQ Music." Jellyfesh does not work around that.

## What you need

A QQ Music account and your own API key from
<https://y.qq.com/n/ryqq_v2/qqmusic_skills> (it starts with `qmk-`). The key
is tied to your account and, by QQ Music's rules for it, for personal music
lookup only. Jellyfesh has no key of its own.

## Signing in

In Sources, choose **QQ Music**, then **Enter API Key**, and paste the key.
Jellyfesh checks it with one small request (`/skill/version`) and keeps it.
If the key leaks, reset it on the key page and sign in again.

## What is stored where

| What | Where |
| --- | --- |
| Your API key | your system's keystore (Keychain, Credential Manager, Secret Service, Android Keystore) |
| Search results and lists | nowhere: QQ Music items are not kept in the library |

The key is sent only to QQ Music's API (`a.y.qq.com`), in the
`Authorization` header. Signing out deletes it from this device; to make it
stop working everywhere, delete or reset it on the key page.

## Known limits

* QQ Music's open API cannot list an album's tracks or a singer's music, or
  your own playlists and favourites; albums and singers open in QQ Music.
* Songs come without length, album or cover, so they cannot be matched with
  the same songs from other sources, and QQ Music results are not part of
  searching everything.
* QQ Music limits how fast its API may be called but does not say how much;
  the plugin sends at most two requests a second.
* Not checked against the live service: answers that differ from the
  documentation (for example other error codes) may show up as errors.
