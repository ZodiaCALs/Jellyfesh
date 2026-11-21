# Unofficial community plugins

Two source plugins that the Jellyfesh apps bundle as **community plugins**
(ADR-0012, amended for v0.0.6):

| Plugin | Id | Package |
| --- | --- | --- |
| **NetEase Cloud Music (Unofficial)** ([netease/](netease/README.md)) | `community.unofficial.netease` | `dist/plugins/community.unofficial.netease.jfpkg` |
| **QQ Music (Unofficial)** ([qqmusic/](qqmusic/README.md)) | `community.unofficial.qqmusic` | `dist/plugins/community.unofficial.qqmusic.jfpkg` |

Both sign in with a QR code (NetEase's own web QR sign-in; WeChat's for QQ
Music), search, browse your liked songs and playlists, play, and fetch
synced lyrics and cover art, with the person's own account and no API key.
From version 0.1.2 (Jellyfesh v0.0.7, plugin API 1.2) both have a Discover
page and details; the NetEase plugin also shows comments and turns NetEase's
word-timed lyrics (YRC) into enhanced LRC, word by word.
For people using them: [`docs/plugins/community-netease.md`](../../docs/plugins/community-netease.md)
and [`docs/plugins/community-qqmusic.md`](../../docs/plugins/community-qqmusic.md).

They were written in a separate repository up to version 0.1.0 and moved
here, without that history, for Jellyfesh v0.0.6 (version 0.1.1).

## Read this first

* **Unofficial and unsupported.** These plugins are not made, endorsed or
  supported by NetEase, Tencent or the Jellyfesh project. They are community
  plugins (Jellyfesh ADR-0012): never signed with the Jellyfesh release key,
  never in store builds, installed only once the person allows community
  plugins, off until they accept the plugin's disclosure, and shown as
  **Community · unofficial**.
* **They may break or be blocked at any time.** They talk to the services'
  own web players' interfaces, which can change without notice, and the
  services may refuse clients like these.
* **They may conflict with the services' terms.** Using them may go against
  NetEase's or Tencent's terms of service. Personal use only; the person
  decides whether to use them, at their own risk.
* **They play only what the account may play.** No paywall, VIP or region
  bypass, no DRM circumvention, no preview passed off as the song: when a
  service gives the account no full-track stream, the track is unplayable,
  with the reason the service gives (a subscription or purchase needed:
  `paywall`; anything else: `unavailable`). Neither service's web answers
  say reliably when a track is limited to a region, so a region lock shows
  as unavailable, worded as possibly regional.
* **Not tested against the live services.** They were written and tested
  where music.163.com, y.qq.com and WeChat could not be reached: every test
  runs against a local mock built from the author's understanding of the
  endpoints, with invented data. Each plugin's README lists what to check
  first against the real service.

## Turning them on

Every Jellyfesh app from v0.0.6 on carries both packages (Jellyfesh Web
does not: the services' web interfaces do not allow a browser page on
another site to call them, CORS). The core installs them only once the
person allows community plugins:

1. **Settings › Sources › Community Plugins › Allow Community Plugins**, and
   accept the disclosure: the plugin may violate the service's terms, may
   break at any time, and is for personal use only. The bundled community
   plugins are installed then, turned off.
2. Under **Installed**, turn the plugin on; Jellyfesh shows the disclosure
   for that plugin first.
3. **Sign In with QR Code**: scan the code with the NetEase Cloud Music app,
   or with WeChat for QQ Music, and confirm on the phone.

Each plugin has two settings: **Streaming Quality** (follow Jellyfesh, or
always use one quality; the account still decides what it gets) and
**Translated Lyrics**.

The release also publishes both packages as separate downloads
(`community.unofficial.netease-<release>.jfpkg`,
`community.unofficial.qqmusic-<release>.jfpkg`): **Install from File…**
installs one once community plugins are allowed, for example after the
bundled one was uninstalled. Version 0.1.2 declares plugin API 1.2 and
needs Jellyfesh v0.0.7 or later; 0.1.1 declared API 1.0 (0.1.0 was also
checked in the API 1.0 host of Jellyfesh 0.0.5; 0.1.1 was not).

## Build

`plugins/release.sh` (run from the repository root) builds both in release
mode with the official plugins and writes them **unsigned** to
`dist/plugins/community.unofficial.netease.jfpkg` and
`dist/plugins/community.unofficial.qqmusic.jfpkg`, checked with the app's
own install checks. It never signs them, also when it signs the official
plugins with the release key (their builds do not even see the key), and
fails if one comes out signed or with the tests' `mock_origin` setting. The
apps' builds bundle every `dist/plugins/*.jfpkg` (Jellyfesh Web only its
own list). One plugin by hand:

```sh
cargo run -p jf-plugin -- pack --release --path plugins/community/netease --out netease.jfpkg
cargo run -p jf-plugin -- verify netease.jfpkg      # signature: none: unsigned …
```

## Test

```sh
cargo test -p jf-community-netease -p jf-community-qqmusic -p jf-community-common -p jf-community-testsupport
```

* Unit tests (plain Rust, no host): the NetEase request encoding against
  reference values computed independently (Python), the QQ Music signature
  against reference values from an independent implementation of the same
  description, LRC handling, item mapping, playability rules, paging.
* Integration tests: each plugin's real WebAssembly component in the real
  Jellyfesh plugin host (production sandbox limits, cookie jar, keystore
  stand-in, answer checks), installed unsigned as a community plugin, against
  a mock of its service on 127.0.0.1 (`tests/mock`, invented data in
  `tests/fixtures`).

The published plugins reach only their services' fixed hosts over https,
which a test cannot serve. So the tests build the same crate with its
`mock-service` feature, which sends every request to the origin in a
`mock_origin` setting (keeping the service's host name in the path), and pack
it with a test copy of the manifest whose allowlist is only that origin. For
NetEase the feature also swaps the service's RSA public key for a prime
stand-in, so the mock can open the encrypted requests (with rsa's
crypto-bigint numbers, not the plugin's own arithmetic). `plugins/release.sh`
never turns the feature on, the published settings schema has no
`mock_origin` (the host refuses a setting its schema does not name; both
plugins' `installs_only_as_a_community_plugin` tests check it), and the
published allowlist could not reach the mock anyway.

The tests build those components with a nested cargo in a target folder of
its own (`target/community-plugin-tests`): a build in the test binary's own
folder would wait for the build lock of the `cargo test` running it.

## What is in here

| Path | Crate | What |
| --- | --- | --- |
| `netease/` | `jf-community-netease` | NetEase Cloud Music (Unofficial): the plugin crate, manifest, settings schema, tests |
| `qqmusic/` | `jf-community-qqmusic` | QQ Music (Unofficial): the same |
| `common/` | `jf-community-common` | helpers both plugins use: base64, LRC lyrics, paging, the quality setting, matching items from other sources |
| `testsupport/` | `jf-community-testsupport` | test-only: builds a plugin with `mock-service` and installs it in a real host as a community plugin |

## Rules these plugins keep

Jellyfesh ADR-0012 sets the rules for community reference plugins, and these
follow them:

* **Web endpoints and web QR sign-in only.** Nothing that needs keys, salts
  or certificates from the services' apps: NetEase's `eapi`/`linuxapi`
  schemes, QQ Music's QIMEI device keys and the QRC lyric key are not used,
  so word-timed lyrics (YRC, QRC) are not offered.
* **No copied vendor code.** NetEase's weapi request encoding and QQ Music's
  web request signature were re-implemented from the protocols, not taken
  from NeteaseCloudMusicApi, QQMusicApi, Tencent's JavaScript or any other
  project. The constants they use (a fixed AES key and IV and an RSA public
  key for NetEase; digit positions and sixteen mixing bytes for QQ Music) are
  what every browser receives with the web players. ADR-0012 asks for a legal
  check before a web-derived algorithm is re-implemented (⚑): that check is
  the owner's.
* **No paywall or region bypass, no DRM circumvention**, as above.
* **Only the hosts each plugin needs** (see each manifest), at most five
  requests a second, streams never cached (`library_storage = "metadata"`).

The official NetEase Cloud Music and QQ Music plugins (`plugins/netease`,
`plugins/qqmusic`) use the services' official interfaces with the person's
own developer credentials; they share no code with these, and both kinds
can be installed side by side.

## Licence

None chosen yet: all rights reserved, as for the rest of the repository
(ADR-0016).
