# NetEase Cloud Music (Unofficial)

A community source plugin for Jellyfesh (`community.unofficial.netease`).
**Not made, endorsed or supported by NetEase or by Jellyfesh.** It uses the
music.163.com web player's own interface, may break or be blocked at any
time, may conflict with NetEase's terms, and plays only what your account
may play. Read the [community plugins' README](../README.md) first. The
apps bundle it unsigned, off until you allow community plugins; how to turn
it on: [`docs/plugins/community-netease.md`](../../../docs/plugins/community-netease.md).

> **Not tested against the live service.** music.163.com could not be
> reached from where this plugin was written. Everything below is tested
> against a local mock built from the author's understanding of the
> endpoints, with invented data. See [What to check first](#what-to-check-first).

## What it does

* **Sign in** with the web player's QR code: scan it with the NetEase Cloud
  Music app (the scan button on its home screen) and confirm on your phone.
* **Search** songs, albums, artists and playlists.
* **Library**: Liked Songs, My Playlists, Saved Playlists; a playlist's and
  an album's tracks, an artist's albums.
* **Play** at the quality you choose: Low → `standard` (128 kbit/s MP3),
  Normal → `higher` (192), High → `exhigh` (320), Lossless → `lossless`
  (FLAC). Your account decides what you get; NetEase may give less.
* **Lyrics**: plain synced LRC, with NetEase's translation shown under each
  line when it has one (setting **Translated Lyrics**); credits at the top
  become timed lines; instrumentals give no lyrics. **Word-timed lyrics**
  (NetEase's YRC, asked for with `yv`/`ytv` on `/song/lyric/v1`; the older
  `/song/lyric` when that fails) become enhanced LRC in `src/yrc.rs`, with
  the YRC translation (`ytlrc`) under each line; golden tests in
  `tests/golden/yrc/`.
* **Discover** (plugin API 1.2): Daily Picks (`/v3/discovery/recommend/songs`,
  signed in), Recommended Playlists (`/v1/discovery/recommend/resource`
  signed in, `/personalized/playlist` otherwise) and Charts (`/toplist`),
  with See All (`ncm:f:daily`, `ncm:f:recommended`, `ncm:f:charts`).
* **Details** of songs, albums, artists and playlists (the detail answers the
  plugin already reads: release dates, notes, label, kind, biography, other
  names, plays, tags, comment counts) and **comments** on songs, albums and
  playlists (`/v1/resource/comments/<thread>`: `R_SO_4_`, `R_AL_3_`,
  `A_PL_0_`), hot ones on the first page.
* **Artwork** at the size the screen needs, for its own items and, matched by
  title, artist and length, for tracks and albums from other sources (the
  same matching finds lyrics for them).

### What it never does

* Play a preview in place of the song. NetEase offers non-subscribers clips
  of VIP songs; a stream marked as a preview (`freeTrialInfo`) or much
  shorter than the song is refused: "NetEase Cloud Music offers your account
  only a preview of this track…".
* Get around a paywall, a region lock or DRM. Tracks the account may not play
  are listed as unplayable with NetEase's reason (from the `privilege` NetEase
  sends with every listing: a VIP subscription or an album purchase needed,
  or no rights).
* Use anything from NetEase's apps: the `eapi`/`linuxapi` schemes and their
  keys and app-only stream endpoints are not implemented. Word-timed lyrics
  come only if the web interface's own lyric answer carries them.
* Reach any host but `music.163.com` (the API) and `*.music.126.net`
  (streams and pictures). A stream on any other host is refused.

## How it talks to NetEase

Every request is `POST https://music.163.com/weapi/<path>?csrf_token=…` with
a form of two fields, encoded the way the web player encodes them
(`src/weapi.rs`, written from the protocol):

1. the parameters as JSON, with `csrf_token`;
2. AES-128-CBC under the web client's fixed key and IV, then base64;
3. AES-128-CBC again under a fresh random 16-character key, then base64:
   `params`;
4. that key, reversed, raised to 65537 modulo the web client's 1024-bit
   public modulus without padding, as 256 hex digits: `encSecKey`. The
   host's crypto import only does padded RSA, so this step is done in the
   plugin (`src/rsa_raw.rs`, Montgomery multiplication), checked against
   reference values computed independently.

Requests carry the web player's `Referer` and `Origin` and a desktop
browser's `User-Agent`.

| What | Endpoint (`/weapi…`) |
| --- | --- |
| QR sign-in | `/login/qrcode/unikey` (`type` 1), `/login/qrcode/client/login` (801 waiting, 802 scanned, 803 signed in, 800 expired), then `/nuser/account/get` |
| Sign out | `/logout` |
| Search | `/cloudsearch/get/web` (`type` 1 songs, 10 albums, 100 artists, 1000 playlists) |
| Liked songs | `/song/like/get` (ids), then `/v3/song/detail` |
| Own and saved playlists | `/user/playlist` |
| A playlist | `/v6/playlist/detail` (its `trackIds`), then `/v3/song/detail` per page |
| An album, an artist | `/v1/album/<id>`, `/artist/albums/<id>`, `/v1/artist/<id>` |
| Stream | `/song/enhance/player/url/v1` (`level`), else `/song/enhance/player/url` (`br`) |
| Lyrics | `/song/lyric` (`lv`, `tv`) |
| Pictures | the picture's URL with `?param=<size>y<size>` |

### What is stored where

| What | Where |
| --- | --- |
| The session cookie (`MUSIC_U`) and the other cookies NetEase sets | the host's cookie jar for this plugin, encrypted on disk |
| Your account id, its name and the `__csrf` value | your system's keystore |
| Quality and translation settings | the plugin's settings |

Signing out ends the session on NetEase (`/logout`); Jellyfesh then deletes
the keystore entry and the cookies.

## What to check first

In this order, against the live service with a real account (a free one and,
if possible, a VIP one), before anyone relies on the plugin:

1. **The request encoding.** Does `/weapi/login/qrcode/unikey` answer
   `code: 200` with a `unikey`? If NetEase cannot decrypt a request it
   answers an error or nothing: compare the fixed key, IV and modulus in
   `src/weapi.rs` with what the web player sends.
2. **QR sign-in.** The QR payload `https://music.163.com/login?codekey=<key>`
   scans in the app; the poll codes are 801/802/803/800; the 803 answer sets
   `MUSIC_U` and `__csrf` cookies (the plugin reads `__csrf` from
   `set-cookie`); `/nuser/account/get` answers `profile.userId` and
   `nickname`. NetEase may answer 8821 or -462 (a security check the plugin
   cannot show).
3. **Streams.** Whether the weapi stream endpoints still give URLs at all
   (the Jellyfesh sources report could not confirm it); the level names; that
   the URLs are on `*.music.126.net` and work over https (the plugin upgrades
   `http` and refuses other hosts).
4. **What a free account gets for a VIP song**: a URL with `freeTrialInfo`
   (refused as a preview), or `url: null` (refused with the reason). And how
   `privilege` looks for it (`pl: 0`, `fee: 1`): listings rely on that. How a
   region-locked song looks: it shows as unavailable today; if NetEase says
   so explicitly, map it to `region` in `src/items.rs` and `src/stream.rs`.
5. **Search** with all four `type`s and their result keys; from outside
   mainland China NetEase may send scrambled results (`abroad: true`), which
   the plugin reports as an error rather than decoding.
6. **The library calls**: `/v3/song/detail` with the `c` parameter,
   `/user/playlist` (`specialType` 5 for the liked songs), `/song/like/get`,
   `/v6/playlist/detail` (`trackIds`), `/v1/album/<id>` (whether its songs
   carry `privilege`; if not, the plugin asks `/v3/song/detail`),
   `/artist/albums/<id>`, `/v1/artist/<id>`.
7. **Lyrics**: `/song/lyric/v1` with `lv`/`tv`/`yv`/`ytv` (whether weapi
   answers it, and with `yrc` and `ytlrc`; the plugin falls back to
   `/song/lyric`), the YRC line and word stamps (absolute milliseconds; the
   converter reads stamps more than a second before their line as
   relative), the JSON credit lines, the instrumental marker
   (`纯音乐，请欣赏` at 99:00).
8. **Discover, details and comments**: `/v3/discovery/recommend/songs`
   (`data.dailySongs`, with or without `privilege`),
   `/v1/discovery/recommend/resource` (`recommend`, `picUrl`),
   `/personalized/playlist` (`result`), `/toplist` (`list`), the detail
   fields (`description`, `company`, `subType`, `info.commentCount`,
   `briefDesc`, `alias`, `tags`, `playCount`, `commentCount`,
   `updateFrequency`) and `/v1/resource/comments/<thread>` (`rid`, `offset`,
   `limit`; `hotComments`, `comments`, `total`, `more`, `commentId`,
   `user.nickname`, `user.avatarUrl`, `likedCount`, `time`).
9. **Rate**: whether 5 requests a second is accepted (code 405 means "too
   many").

## Tests

`cargo test -p jf-community-netease`: 38 unit tests (among them the YRC golden
cases in `tests/golden/yrc/`) and 24 integration tests
that run the plugin's component in a real Jellyfesh plugin host, installed
unsigned as a community plugin, against the mock in `tests/mock` (it opens
every request, so a wrongly encoded one fails). The mock and its data are
invented; see the [community plugins' README](../README.md#test) for how
the test build reaches it.
