# NetEase Cloud Music (Unofficial)

**NetEase Cloud Music (Unofficial)** (`community.unofficial.netease`) is a
**community plugin** that comes with every Jellyfesh app from v0.0.6 on
(not with Jellyfesh Web). You sign in with your own NetEase Cloud Music
account by scanning a QR code: no API key and no developer registration.
It is **not made, endorsed or supported by NetEase or by Jellyfesh**. It
uses the music.163.com web player's own interface, which NetEase may change
or close to other clients at any time. How it works, what it stores and
where: `plugins/community/netease/README.md`.

The official NetEase Cloud Music plugin (`org.jellyfesh.netease`) is a
different plugin, on NetEase's open platform with your own developer
credentials: [netease.md](netease.md). Both can be installed at once.

## Before you turn it on

* **A community plugin.** It is not signed with the Jellyfesh release key
  and shows as **Community · unofficial**. Jellyfesh installs it only after
  you allow community plugins, and it stays off until you accept its
  disclosure.
* **It may break or be blocked at any time**, and **using it may go against
  NetEase's terms of service.** Personal use only, at your own risk.
* You need a NetEase Cloud Music account (VIP for VIP songs) and the NetEase
  Cloud Music app on your phone to scan the code.

## Turn it on

1. Open **Settings › Sources › Community Plugins** and choose **Allow
   Community Plugins**. Read and accept the disclosure: the plugin may
   violate the service's terms, may break at any time, and is for personal
   use only. Jellyfesh then installs the community plugins that come with
   the app, turned off.
2. Under **Installed**, turn on **NetEase Cloud Music (Unofficial)** and
   accept its disclosure.
3. Choose **Sign In with QR Code** and scan the code with the NetEase Cloud
   Music app (the scan button on its home screen), then confirm on your
   phone. If the code expires, start again for a new one.

Turning **Allow Community Plugins** off again keeps the plugin installed
but unusable until you allow them again.

## What it does

* **Search** songs, albums, artists and playlists.
* **Your library**: Liked Songs, My Playlists and Saved Playlists; the
  tracks of a playlist or album, an artist's albums.
* **Plays** whole tracks at the quality you choose, as far as your account
  allows.
* **Lyrics**: synced lyrics, with NetEase's translation under each line when
  it has one; instrumentals have none. No word-by-word lyrics (they need
  NetEase's app keys, which this plugin does not use).
* **Artwork** for its own items and, matched by title, artist and length,
  for songs and albums from your other sources (the same matching finds
  lyrics for them).

## Settings

| Setting | Values | What it does |
| --- | --- | --- |
| Streaming Quality | `app` (default), `lossless`, `high`, `normal`, `low` | `app` follows the quality you chose in Jellyfesh for each network; the others always ask NetEase for that quality |
| Translated Lyrics | on (default), off | NetEase's translation under each line of the lyrics |

| Quality | NetEase is asked for |
| --- | --- |
| low | `standard`, 128 kbit/s MP3 |
| normal | `higher`, 192 kbit/s |
| high | `exhigh`, 320 kbit/s |
| lossless | `lossless`, FLAC |

Your account decides what you get: without VIP, NetEase may give a lower
quality or nothing.

## What plays and what does not

The plugin plays only what your account may play. It never gets around a
paywall, a region lock or DRM, and never plays a preview in place of the
song.

| The song | In Jellyfesh |
| --- | --- |
| Free, or yours to play | plays |
| Needs VIP or an album purchase | not playable, "needs a purchase", with NetEase's reason |
| NetEase offers only a preview | not played: Jellyfesh does not play previews |
| No rights, or possibly limited to some regions | unavailable |

## What is not verified

* **Tested only against a mock service.** The plugin was written and tested
  where music.163.com could not be reached: every test runs against a local
  mock built from the author's understanding of the web player's interface,
  with invented data. Whether NetEase accepts its requests, its QR sign-in
  and its stream requests today is not known.
* **NetEase's private web interface may change or forbid this** at any time.
  NetEase may also ask for a security check the plugin cannot show (it says
  so and asks you to try later), turn requests away as automated, or, from
  outside mainland China, send scrambled search results, which the plugin
  reports as an error.
* **No paywall bypass**, as above: a song your account may not play stays
  unplayable.
* **Not in Jellyfesh Web**: music.163.com's web interface does not let a
  web page on another site call it (CORS), so only the apps carry the
  plugin.

## What is stored where

| What | Where |
| --- | --- |
| The session cookie and the other cookies NetEase sets | the plugin's cookie jar, encrypted on disk |
| Your account id, its name and the `__csrf` value | your system's keystore |
| Quality and translation settings | the plugin's settings |

**Sign Out** ends the session on NetEase; Jellyfesh then deletes the
keystore entry and the cookies.
