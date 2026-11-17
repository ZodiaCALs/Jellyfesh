# QQ Music (Unofficial)

A community source plugin for Jellyfesh (`community.unofficial.qqmusic`).
**Not made, endorsed or supported by Tencent or by Jellyfesh.** It uses the
y.qq.com web player's own interface, may break or be blocked at any time,
may conflict with Tencent's terms, and plays only what your account may
play. Read the [community plugins' README](../README.md) first. The apps
bundle it unsigned, off until you allow community plugins; how to turn it
on: [`docs/plugins/community-qqmusic.md`](../../../docs/plugins/community-qqmusic.md).

> **Not tested against the live service.** y.qq.com, u.y.qq.com and WeChat
> could not be reached from where this plugin was written. Everything below
> is tested against a local mock built from the author's understanding of
> the endpoints, with invented data. See [What to check first](#what-to-check-first).

## What it does

* **Sign in** with WeChat: scan the QR code with WeChat on your phone and
  confirm the sign-in to QQ Music there. Your WeChat account must already be
  linked to QQ Music.
* **Search** songs, singers, albums and playlists.
* **Library**: Liked Songs and My Playlists; a playlist's and an album's
  tracks, a singer's albums.
* **Play** at the quality you choose or below: the plugin asks for FLAC, 320
  and 128 kbit/s MP3 and 96 kbit/s AAC, best first down from the chosen
  quality (Lossless, High, Normal, Low), and plays the first file QQ Music
  grants your account; without VIP that may be a lower quality, or nothing.
* **Lyrics**: plain synced LRC, with QQ Music's translation shown under each
  line when it has one (setting **Translated Lyrics**).
* **Artwork** from QQ Music's picture service at the nearest size (90 to
  800 px), for its own items and, matched by title, artist and length, for
  tracks and albums from other sources (the same matching finds lyrics).

### What it never does

* Play a preview in place of the song. It takes only the file it asked for
  by name: the stream answer's preview files (`testfile2g`, `testfilewifi`)
  are never read, and a path for another file than the one named is refused
  as a preview.
* Get around a paywall, a region lock or DRM. When QQ Music grants nothing,
  the track is refused with the reason (a VIP subscription or purchase
  needed when the song says so; otherwise unavailable, possibly regional,
  with QQ Music's code), and later listings show it as unplayable.
* Use anything from QQ Music's apps: no QIMEI device keys, no app sign-in,
  and no QRC word-timed lyrics (encrypted with an app key; an encrypted
  answer means no lyrics).
* Reach any host but the six in its manifest: `u.y.qq.com` (the API),
  `open.weixin.qq.com` and `lp.open.weixin.qq.com` (WeChat sign-in),
  `*.stream.qqmusic.qq.com` (streams), `y.gtimg.cn` and `qpic.y.qq.com`
  (pictures). A stream host outside that list is never used.

**QQ's own QR sign-in (ptlogin) is not offered.** It hands out its QR code
only as a picture, and Jellyfesh draws QR codes itself from their text;
reading the picture would need a PNG and a QR decoder inside the plugin. The
owner may decide to add that later.

## How it talks to QQ Music

Every call is `POST https://u.y.qq.com/cgi-bin/musics.fcg?_=<ms>&sign=<sign>`
with a JSON body `{"comm": {…}, "req_1": {"module": …, "method": …,
"param": …}}`. The `sign` is the web player's "zzb" signature over the exact
body (`src/sign.rs`, written from the protocol, not from Tencent's
JavaScript): the MD5 of the body as uppercase hex, eight digits at fixed
positions, the sixteen digest bytes mixed with sixteen fixed bytes in base64
without `/`, `+` and `=`, eight more digits, all after `zzb` in lowercase.
`comm` carries `g_tk` (the web player's hash of the session key) and, when
signed in, `uin` (the account's `musicid`), `authst` (its `musickey`) and
`tmeLoginType` 1; the key also goes in the cookies the web player sends
(`uin`, `qqmusic_key`, `qm_keyst`).

| What | Call |
| --- | --- |
| WeChat sign-in | `GET open.weixin.qq.com/connect/qrconnect` (app id `wx48db31d50e334801`, the uuid in the page), QR payload `https://open.weixin.qq.com/connect/confirm?uuid=…`, long poll `lp.open.weixin.qq.com/connect/l/qrconnect` (408 waiting, 404 scanned, 405 confirmed with the code, 403 cancelled, 402 expired) |
| Session from the code; renewing the key | `music.login.LoginServer` `Login` (`code`, `strAppid`; or `refresh_key`, `refresh_token`) |
| Account name | `music.UserInfo.userInfoServer` `GetLoginUserInfo` (best effort) |
| Search | `music.search.SearchCgiService` `DoSearchForQQMusicDesktop` (`search_type` 0 songs, 1 singers, 2 albums, 3 playlists) |
| Own playlists, liked songs (folder 201) | `music.musicasset.PlaylistBaseRead` `GetPlaylistByUin` |
| A playlist | `music.srfDissInfo.aiDissInfo` `uniform_get_Dissinfo` |
| An album, a singer | `music.musichallAlbum.AlbumSongList` `GetAlbumSongList`, `music.musichallAlbum.AlbumListServer` `GetAlbumList` |
| A song | `music.pf_song_detail_svr` `get_song_detail_yqq` |
| Stream | `vkey.GetVkeyServer` `CgiGetVkey` (file names `F000`/`M800`/`M500`/`C400` + media id) |
| Lyrics | `music.musichallSong.PlayLyricInfo` `GetPlayLyricInfo` (`qrc` 0, `crypt` 0) |
| Pictures | `y.gtimg.cn/music/photo_new/T002R<size>x<size>M000<album mid>.jpg` (albums), `T001…` (singers) |

### What is stored where

| What | Where |
| --- | --- |
| Your `musicid`, `musickey`, what renews the key, and your name | your system's keystore |
| A random 10-digit player id (`guid`) for the stream service | the plugin's small private storage |
| Cookies QQ Music or WeChat set | the host's cookie jar for this plugin, encrypted on disk |
| Quality and translation settings | the plugin's settings |

The web player has no sign-out call; signing out forgets the session and
Jellyfesh deletes the keystore entry and the cookies.

## What to check first

In this order, against the live service with a real account (a free one and,
if possible, a VIP one), before anyone relies on the plugin:

1. **The signature.** Does `musics.fcg` accept the zzb `sign`? QQ Music has
   changed its web signature before and may use a newer scheme now; if calls
   are refused, compare the plugin's `sign` with the browser's for the same
   body, and update `src/sign.rs` (after the legal check the community
   plugins' README mentions). Also check the body's content type (the
   plugin sends the JSON as `application/x-www-form-urlencoded`, as it
   understands the web player does) and the `comm` fields (`cv` 4747474,
   `ct` 24, `platform` `yqq.json`).
2. **WeChat sign-in.** The app id, the `redirect_uri`, where the uuid is in
   WeChat's page, the long poll's answers and its `last` parameter, and that
   WeChat scans the payload `https://open.weixin.qq.com/connect/confirm?uuid=…`.
3. **The session.** `Login` with `code`, `strAppid` and `tmeLoginType` 1:
   the field names (`musicid`, `musickey`, `refresh_key`, `refresh_token`,
   `keyExpiresIn`), which of `comm.authst` or the cookies the service reads,
   the "not signed in" code (the plugin assumes 1000), and the renewal
   parameters.
4. **Streams.** `CgiGetVkey`: the file name prefixes, `purl`, the `sip`
   hosts over https, what a free account gets for VIP songs and for
   region-locked ones (and their `result` codes: today both show as
   unplayable; map a clear region code to `region` in `src/stream.rs`).
5. **Search** `search_type` numbers and the field names of each kind
   (`src/items.rs` tries several).
6. **The library calls**: `GetPlaylistByUin` (folder 201 for the liked
   songs, field names), `uniform_get_Dissinfo` paging, `GetAlbumSongList`,
   `GetAlbumList`, `get_song_detail_yqq`.
7. **Lyrics**: `GetPlayLyricInfo` answers base64 `lyric` and `trans` with
   `crypt` 0 when `qrc` is 0.
8. **Pictures**: the `T002`/`T001` sizes, and `qpic.y.qq.com` covers over
   https.
9. **The account name** from `GetLoginUserInfo` (otherwise the plugin shows
   "QQ Music user <number>").

## Tests

`cargo test -p jf-community-qqmusic`: 25 unit tests and 17 integration tests
that run the plugin's component in a real Jellyfesh plugin host, installed
unsigned as a community plugin, against the mock in `tests/mock` (it checks
every request's signature with its own implementation of the scheme). The
mock and its data are invented; see the [community plugins'
README](../README.md#test) for how the test build reaches it.
