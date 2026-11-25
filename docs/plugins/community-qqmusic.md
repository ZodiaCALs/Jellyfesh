# QQ Music (Unofficial)

**QQ Music (Unofficial)** (`community.unofficial.qqmusic`) is a **community
plugin** that comes with every Jellyfesh app from v0.0.6 on (not with
Jellyfesh Web). You sign in with your own QQ Music account by scanning a
WeChat QR code: no API key and no developer registration. It is **not made,
endorsed or supported by Tencent or by Jellyfesh**. It uses the y.qq.com
web player's own interface, which Tencent may change or close to other
clients at any time. How it works, what it stores and where:
`plugins/community/qqmusic/README.md`.

The official QQ Music plugin (`org.jellyfesh.qqmusic`) is a different
plugin, on QQ Music's official API with your own personal API key, whose
songs open in QQ Music: [qqmusic.md](qqmusic.md). Both can be installed at
once.

## Before you turn it on

* **A community plugin.** It is not signed with the Jellyfesh release key
  and shows as **Community · unofficial**. Jellyfesh installs it only after
  you allow community plugins, and it stays off until you accept its
  disclosure.
* **It may break or be blocked at any time**, and **using it may go against
  Tencent's terms of service.** Personal use only, at your own risk.
* You need a QQ Music account linked to your **WeChat** account (VIP for VIP
  songs), and WeChat on your phone to scan the code. Signing in with a QQ
  account's own QR code is not offered.

## Turn it on

1. Open **Settings › Sources › Community Plugins** and choose **Allow
   Community Plugins**. Read and accept the disclosure: the plugin may
   violate the service's terms, may break at any time, and is for personal
   use only. Jellyfesh then installs the community plugins that come with
   the app, turned off.
2. Under **Installed**, turn on **QQ Music (Unofficial)** and accept its
   disclosure.
3. Choose **Sign In with QR Code**, scan the code with WeChat and confirm the
   sign-in to QQ Music on your phone. If Jellyfesh says QQ Music did not
   accept the WeChat account, link it to QQ Music in the QQ Music app first.
   If the code expires, start again for a new one.

Turning **Allow Community Plugins** off again keeps the plugin installed
but unusable until you allow them again.

## What it does

* **Search** songs, singers, albums and playlists.
* **Your library**: Liked Songs and My Playlists; the tracks of a playlist
  or album, a singer's albums. From version 0.1.3 (Jellyfesh v0.0.8)
  Favourites shows the liked songs and Playlists shows your playlists.
* **Plays** whole tracks at the streaming quality chosen in Jellyfesh: it asks QQ Music for
  that file and the ones below it, best first, and plays the first one QQ
  Music grants your account.
* **Lyrics**: synced lyrics, with QQ Music's translation under each line
  when it has one. No word-by-word lyrics (QQ Music encrypts them with an
  app key, which this plugin does not use).
* **Artwork** from QQ Music's picture service, for its own items and,
  matched by title, artist and length, for songs and albums from your other
  sources (the same matching finds lyrics for them).
* **Discover** (version 0.1.2, with Jellyfesh v0.0.7): Recommended Playlists
  and the Charts, each with See All; a chart opens to its current songs.
* **Get Info** (0.1.2) on a song (language, genre, release date, lyricist,
  composer, notes), an album (notes, release date, genre, language, kind,
  company), a playlist (description, listens, tags) or a chart
  (introduction, listens, last update). Not for singers, and **no comments**:
  QQ Music's interfaces for those could not be checked.

## Settings

| Setting | Values | What it does |
| --- | --- | --- |
| Translated Lyrics | on (default), off | QQ Music's translation under each line of the lyrics |

The streaming quality is Jellyfesh's own (Settings › Playback › Streaming,
or the quality menu in the player bar, from v0.0.8). Version 0.1.3 of the
plugin no longer has a quality setting of its own; a quality you had set
there is kept once, as Jellyfesh's.

| Quality | QQ Music is asked for, in this order |
| --- | --- |
| lossless | FLAC, 320 kbit/s MP3, 128 kbit/s MP3, 96 kbit/s AAC |
| high | 320 kbit/s MP3, 128 kbit/s MP3, 96 kbit/s AAC |
| normal | 128 kbit/s MP3, 96 kbit/s AAC |
| low | 96 kbit/s AAC, 128 kbit/s MP3 |

Your account decides what you get: without VIP, QQ Music may give a lower
quality or nothing.

## What plays and what does not

The plugin plays only what your account may play. It never gets around a
paywall, a region lock or DRM, and never plays a preview in place of the
song.

| The song | In Jellyfesh |
| --- | --- |
| Free, or yours to play | plays |
| Needs VIP or a purchase (when QQ Music says so) | not playable, "needs a purchase" |
| QQ Music grants no file for your account | unavailable, possibly regional, with QQ Music's code |
| Only a preview file | not played: Jellyfesh does not play previews |

## What is not verified

* **Tested only against mock services.** The plugin was written and tested
  where y.qq.com, u.y.qq.com and WeChat could not be reached: every test runs
  against local mocks built from the author's understanding of the web
  player's interface and WeChat's web sign-in, with invented data. Whether
  QQ Music accepts its signed requests and WeChat its sign-in today is not
  known; QQ Music has changed its web request signature before.
* **Tencent's private web interfaces may change or forbid this** at any
  time, as may WeChat's web sign-in.
* **No paywall bypass**, as above: a song your account may not play stays
  unplayable.
* **Not in Jellyfesh Web**: QQ Music's and WeChat's web interfaces do not let
  a web page on another site call them (CORS), so only the apps carry the
  plugin.

## What is stored where

| What | Where |
| --- | --- |
| Your account number, its session key, what renews the key, and your name | your system's keystore |
| A random 10-digit player id for the stream service | the plugin's small private storage |
| Cookies QQ Music or WeChat set | the plugin's cookie jar, encrypted on disk |
| The translation setting | the plugin's settings |

The web player has no sign-out call: **Sign Out** forgets the session, and
Jellyfesh deletes the keystore entry and the cookies.
