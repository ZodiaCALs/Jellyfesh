# NetEase Cloud Music

The NetEase Cloud Music plugin plays your NetEase Cloud Music playlists in
Jellyfesh, through NetEase's open platform and with your own developer
credentials. It comes with every Jellyfesh app and updates with it; the same
plugin is also published as a separate download (`org.jellyfesh.netease`,
signed with the Jellyfesh release key). What it stores and where:
`plugins/netease/README.md`.

## What it is, and what it is not yet

NetEase's open-platform documentation was not reachable when the plugin was
written. It follows what NetEase's official command-line client
(`@music163/ncm-cli` 0.1.7) was found to send, and was tested against a mock
server built from that, **not yet against the live service**.

* **Yes**: sign-in with your App ID, private key and a QR code; your
  created and collected playlists and your liked songs (NetEase's red-heart
  playlist); streams by quality; NetEase's rules on what your account may
  play. From Jellyfesh v0.0.7: a **Discover** page of those lists (Liked
  Songs, Created Playlists, Collected Playlists, with covers) and **Get
  Info** on a playlist (its description, picture, plays, tags and creation
  date).
* **Not yet**: search ("Search arrives once NetEase's API documentation is
  available to us"), recommendations such as the daily picks, comments and
  lyrics: their paths were not in what could be read (the official client
  gets them from a list of commands on NetEase's server). Because it cannot search, NetEase Cloud
  Music is **not offered in Add a Source** yet; set it up from the installed
  plugins in Sources.
* **No Open in NetEase Cloud Music**: a song has an encrypted id for the API
  and a numeric one for NetEase's links, and Jellyfesh's link templates take
  one id.

## Before you start

* A NetEase Cloud Music account (VIP for VIP songs).
* **Your own individual developer registration** on NetEase Cloud Music's
  open platform: apply at
  <https://developer.music.163.com/st/developer/apply/account?type=INDIVIDUAL>
  with the same account. When it is approved, the console shows your **App
  ID** and an RSA **private key**. Every request is signed with that key, so
  Jellyfesh cannot ship one of its own, and NetEase lets only the account
  that registered sign in with it.

Whether NetEase's developer terms allow using personal credentials in a
player like Jellyfesh could not be checked.

## Set it up

1. In Sources, open **NetEase Cloud Music** among the installed plugins and
   sign in.
2. Enter the **App ID** and paste the **private key**, with or without its
   `BEGIN` and `END` lines. Jellyfesh checks that it can sign with it.
3. Scan the QR code with the NetEase Cloud Music app (signed in to the
   registered account) and confirm in the app. The code works for five
   minutes.

## Streaming quality

The plugin has no settings. The streaming quality is Jellyfesh's own
(Settings › Playback › Streaming, or the quality menu in the player bar,
from v0.0.8); a quality you had set in version 0.0.7 of the plugin is kept
once, as Jellyfesh's.

| Quality | NetEase is asked for |
| --- | --- |
| low, normal | 128 kbit/s |
| high | 320 kbit/s |
| lossless | its lossless files (999), where your account may play them |

## Favourites and Playlists

From Jellyfesh v0.0.8, Favourites shows your liked songs (the red-heart
playlist, newest first, as NetEase keeps it) and Playlists shows your
created playlists, then the ones you collected. The open platform offers no
liked albums or artists.

## What plays and what does not

| The song | In Jellyfesh |
| --- | --- |
| Free, or yours to play | plays |
| A free full listen NetEase offers | plays in full |
| VIP song | "needs a purchase", with NetEase's reason |
| From a digital album | "needs a purchase", with NetEase's reason |
| Only a preview is free | not played: Jellyfesh does not play previews |
| No rights on this client | unavailable |

## Signing out

Signing out deletes the tokens, the App ID and the private key on this
device. The next sign-in asks for them again.

## When something goes wrong

| Message | What to do |
| --- | --- |
| Jellyfesh cannot use that private key … | Paste the whole private key from the console (an encrypted key is not supported). |
| … the account that signs in must be the one that registered the App ID … | Scan with the account that registered, or use your own registration. |
| The QR code expired. | Start again and scan within five minutes. |
| NetEase Cloud Music's daily play limit is reached … (今日播放次数已达上限…) | Try again tomorrow. |
| Too many requests (the source pauses) | NetEase asks to slow down; wait a moment. |
| Jellyfesh cannot sign with your private key … | Sign out and sign in again with the key from the console. |
| Sign in again. | The sign-in expired and could not be renewed. |

## Privacy

Requests go only to `openncm.music.163.com`; audio and pictures come from
NetEase's servers (`*.music.126.net`). The private key is stored once, in
your system's keystore, and Jellyfesh signs with it there; the plugin never
reads it back. Jellyfesh sends `127.0.0.1` as your address instead of
looking up your public IP with outside services, and does not report plays.
