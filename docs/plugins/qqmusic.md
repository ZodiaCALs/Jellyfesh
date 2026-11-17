# QQ Music

The QQ Music plugin finds music in QQ Music with your own API key and opens
it in QQ Music. It comes with every Jellyfesh app and updates with it; the
same plugin is also published as a separate download
(`org.jellyfesh.qqmusic`, signed with the Jellyfesh release key). What it
stores and where: `plugins/qqmusic/README.md`.

## What it is

QQ Music's official open API (the `qqmusic-skills` protocol, version 0.0.3)
gives song, album, singer, playlist and chart information and links to QQ
Music's pages, never audio. So nothing plays in Jellyfesh: every item says
"QQ Music's open API does not stream; open it in QQ Music.", and **Open in
QQ Music** is its main action.

The plugin was tested against a mock server written from the API's
documentation, **not yet against the live service**.

## Before you start

You need a QQ Music account and your own API key: sign in at
<https://y.qq.com/n/ryqq_v2/qqmusic_skills> and create one (it starts with
`qmk-`). QQ Music issues these keys for personal music lookup only and may
revoke a key that is used for anything else, such as collecting its
catalogue. Do not share the key; if it leaks, reset it on that page.

## Set it up

1. In Sources, choose **QQ Music** in Add a Source (or open it in the list
   of installed plugins).
2. **Enter API Key**: paste the key. Jellyfesh checks it with one small
   request and keeps it in your system's keystore.

There are no settings.

## What you can do

* **Search** songs, singers, albums and playlists (QQ Music's own view; its
  results are not mixed into searching everything).
* **Browse** Daily 30, Guess You Like, AI Playlists and the charts (by
  group); open a chart or a playlist to see its songs.
* **For You**: Daily 30, then Guess You Like.
* **Open in QQ Music** on any song, album, singer or playlist.

Albums and singers cannot be opened in Jellyfesh: the API cannot list their
music, so they open in QQ Music.

## Signing out

Signing out deletes the stored key on this device. The key keeps working
until you delete or reset it on the key page.

## When something goes wrong

| Message | What to do |
| --- | --- |
| A QQ Music API key starts with qmk- … | Copy the whole key again from the key page. |
| QQ Music did not accept this API key | The key is wrong, reset or revoked: create a new one and sign in again. |
| Sign in again. | QQ Music stopped accepting the stored key. Sign in with a current one. |
| QQ Music asks to slow down | Its rate limit was reached; wait a minute. Jellyfesh sends at most two requests a second. |
| QQ Music's open API cannot list … Open … in QQ Music | Use **Open in QQ Music**. |
| Another message from QQ Music | Shown as QQ Music wrote it. |

## Privacy

The key goes only to QQ Music's API (`a.y.qq.com`), in a request header.
Nothing from QQ Music is kept in your library. The plugin does not use the
API's listening report (`/me/report`).
