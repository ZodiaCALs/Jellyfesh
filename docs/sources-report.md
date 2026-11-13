# Online sources: official APIs, terms and feasible playback modes

Status: **findings for review (Step 0)**, checked 2026-11-05.

**Legend.** **V**: read in a primary source (official repo, official docs,
licence file, crate or package registry, or a verbatim archive of the
official page). **U**: secondary source or inference; must be re-checked
before we build on it.

Several official sites could not be reached from this environment's
network policy: developer.spotify.com, developers.google.com,
support.google.com, opensubsonic.netlify.app, api.jellyfin.org, the MusicKit
JS v3 docs, developer.music.163.com, developer.y.qq.com, open.bilibili.com,
lrclib.net, developer.musixmatch.com, last.fm, musicbrainz.org,
learn.microsoft.com. Where a fact depends on one of them it is marked U.
Allowing those hosts in the session's network settings would let me
re-verify them.

## Summary

| Service | Official API | Feasible mode | Deciding constraint | Plan | Confidence |
| --- | --- | --- | --- | --- | --- |
| Local files | n/a | Stream (engine) | — | Built in | — |
| Subsonic / OpenSubsonic (Navidrome) | Yes | **Stream** | API-key auth is in the spec but not yet in a Navidrome release | First-party plugin, M3 | High |
| Jellyfin 12.x | Yes | **Stream** | `MediaBrowser` auth header required; legacy auth removed in 12.0 | First-party plugin, M3/M4 | High |
| Apple Music, macOS | MusicKit | **Vendor** | macOS 14+; Apple renders the audio | Commercial phase | High |
| Apple Music, Android | MusicKit for Android | Vendor (if it still works) | SDK docs return 404; status unclear | Spike S12 | Low |
| Apple Music, Windows | MusicKit JS | Vendor (webview) | WebView2 DRM undocumented; ADPLA scope; token service | Spike S13 + legal | Low–Med |
| Apple Music, Linux | MusicKit JS | **None** | WebKitGTK has no EME; previews would break the terms | Not offered | High |
| Spotify | Web API | **Remote** | Premium only; ~5 users in dev mode; no mixing with other services | Commercial phase; BYO client ID after terms review | Med |
| YouTube | Data API + IFrame player | **Metadata + deep link** | No audio-only, background or own-engine playback | No playback plugin | High |
| YouTube Music | None found | None | — | — | Med |
| SoundCloud | Yes | **Stream** (AAC HLS) | Full track only when `access = playable`; terms unread | Candidate, needs HLS in engine | High (API) / U (terms) |
| TIDAL | Yes + SDKs | Vendor (Widevine) | Third-party full-track access unclear | Not planned | Med |
| Deezer | Legacy | Preview | New app registration closed (U) | Not planned | Low |
| Qobuz | Partners only | Stream (partners) | Contract | Not planned | Low |
| NetEase Cloud Music | **Open platform, individual developers (new)** | Stream? | Terms unread | Evaluate official route first (ADR-0012) | Med |
| QQ Music | `qqmusic-skills` (metadata only) | Metadata + deep link | Personal use; no bulk use; key revocable | Possible official metadata plugin | Med |
| Kugou, Kuwo, Migu | None public | Community only | Everything useful needs app secrets (Kugou) | Template docs only | Med |
| Bilibili | Creator/live platform only | Community only | Legal action against unofficial API docs (Jan 2026) | No reference plugin | Med |
| LRCLIB | Yes (open) | Lyrics | Data licence unknown | Lyrics plugin | Med |
| AMLL TTML DB | Files on GitHub | Lyrics | CC0 for own work; upstream terms otherwise | Lyrics plugin (community) | High |
| Musixmatch | Paid tiers | Lyrics | Synced lyrics need a paid plan (U) | Not planned | Low |
| Last.fm / ListenBrainz / MusicBrainz | Yes | Scrobbling / metadata | Non-commercial (Last.fm, U); 1 req/s (MusicBrainz, U) | Candidates after v1 | Med |

## Subsonic and OpenSubsonic

* OpenSubsonic spec (github.com/opensubsonic/open-subsonic-api, 2026-11-02)
  lists `apiKeyAuthentication`, `formPost`, `transcodeOffset`, `songLyrics`
  (v1 line-synced; v2 adds word cues, `kind` and agents), `transcoding`,
  `playbackReport`, `indexBasedQueue` and others. **V**
* `apiKeyAuthentication` (2024-10): `apiKey=<key>` with no `u` parameter;
  keys do not expire until revoked; clients should prefer it after checking
  `getOpenSubsonicExtensions`, and should use form POST when available. **V**
* Classic token auth is `token = md5(password + salt)` with a fresh salt per
  call; the spec itself calls MD5 obsolete. Navidrome accepts a fixed
  stored (salt, token) pair (no replay check), which is password-equivalent. **V**
* Navidrome: `transcodeOffset`, `formPost`, `songLyrics` v1 since 0.51.0;
  `songLyrics` v2 in 0.63.0 (2026-07); latest release **v0.64.2**
  (2026-09-23). **API-key auth was merged on 2026-09-27 (PR #6219) but is in
  no release yet.** **V**
* Mode: **Stream.**
* **Auth (decided 2026-11-05):** the API key where the server supports it;
  otherwise a salt + token pair stored in the keystore with a clear note that
  it is password-equivalent; switch to the API key automatically when the
  server gains the extension.

## Jellyfin

* Latest stable **12.4** (2026-10-26); 12.0 (2026-09-07) dropped the "10."
  prefix. **V**
* Auth: `POST /Users/AuthenticateByName` → access token; **Quick Connect**
  on by default: `POST /QuickConnect/Initiate` gives a 6-digit code valid
  10 minutes, poll `GET /QuickConnect/Connect?secret=`, then
  `POST /Users/AuthenticateWithQuickConnect`. API keys are admin-only. **V**
* Clients must send `Authorization: MediaBrowser Client=, Device=,
  DeviceId=, Version=, Token=`; 12.0 removed legacy `X-Emby-*` headers,
  `api_key` and the `/emby/` prefixes. `ApiKey=<token>` in the query still
  works. **V**
* Audio: `/Audio/{id}/universal` (server picks direct, HLS or progressive
  transcode); `/Audio/{id}/stream?static=true` returns the original file
  with HTTP Range support; transcodes send `Accept-Ranges: none`. **V**
* Lyrics: `GET /Audio/{id}/Lyrics` with line `Start` ticks and optional
  word-level `Cues`. **V**
* Mode: **Stream.** Quick Connect maps onto our code-display auth flow,
  so no password is ever typed into Jellyfesh.

## Apple Music

See ADR-0010 for the decision.

* macOS: `ApplicationMusicPlayer` macOS 14+; `SystemMusicPlayer` not on
  macOS; automatic developer tokens with the MusicKit App Service; needs a
  developer membership and the user's subscription. **V**
* Android: MusicKit for Android download still listed; its docs URL is a
  404; automatic user tokens not available on Android. **V** SDK versions,
  sign-in via the Apple Music app, licence: **U**.
* MusicKit JS v3: ES256 developer token, `exp` ≤ 6 months, optional
  `origin`; user tokens managed by MusicKit JS. **V** Required key systems:
  **U** (docs unreachable).
* WebView2 DRM: undocumented; PlayReady reportedly works, Widevine request
  open (WebView2Feedback #4828). **U** WebKitGTK: EME off in distro builds. **V**
* ADPLA §3.3.6(D) (2026-10-29): access to the user's own subscription only;
  full songs; user-initiated; rendered only by MusicKit; **no
  synchronisation with other content**; artwork and text only with playback
  or playlist management; MusicKit JS unmodified, not re-hosted. "Application"
  is defined for Apple-branded products. **V** (interpretation for Windows: **U**)
* App Review 4.5.2(i) on Apple Music; no third-party lyrics API. **V**

## Spotify

See ADR-0011 for the decision. Sources: Spotify's official OpenAPI schema
(verbatim copy) for endpoints; Developer Terms and Policy via secondary
sources.

* PKCE; `127.0.0.1` loopback; implicit grant removed (2025). **U**
* Development mode ~5 users, owner needs Premium (Feb 2026); Extended Quota
  only for organisations with ≥ 250 000 MAU (May 2025). **U**
* Player endpoints Premium-only; order of execution not guaranteed;
  many endpoints deprecated in Nov 2024 and Feb 2026; search limit 10. **V (copy)**
* Terms: Remote control makes a Streaming SDA (no ads, not sold); no
  integration with content from another service; no syncing with lyrics;
  temporary caching only; attribution required. **U**
* Android App Remote SDK 0.8.0 (2023); Android auth 5.0.0 (2026-07);
  Web Playback SDK is browser-only. **V**

## YouTube and YouTube Music

* Data API v3 and the IFrame Player API; no official YouTube Music API
  found. **U**
* Developer Policies (verbatim archive, 2026-11-02): no downloading,
  caching or storing audiovisual content; do not "separate, isolate, or
  modify the audio or video components"; no background player; no
  technology other than YouTube API Services to retrieve content; do not
  replace YouTube's own apps; unauthorised data kept at most 30 days. **V (copy)**
* Quota 10 000 units/day; increases need an audit. **V (copy)**
* Conclusion: metadata and an "Open in YouTube" link only. Any audio
  extraction plugin would breach these policies; we will not provide one.

## SoundCloud, TIDAL, Deezer, Qobuz

* SoundCloud: OAuth 2.1 + PKCE, one app per person; `/tracks/{urn}/streams`
  returns AAC HLS; each track's `access` is playable, preview or blocked. **V**
  Terms and rate limits **U**. Feasible in Stream mode once the engine
  supports HLS (also useful for Jellyfin's universal endpoint).
* TIDAL: API and Widevine player modules; client credentials give
  30-second previews; full tracks need user auth; third-party terms **U**.
* Deezer: registration closed, previews only **U**. Qobuz: partners only **U**.

## Regional services

See ADR-0012 for the decision.

| Service | Official route today | Without app-extracted secrets | QR login (web) |
| --- | --- | --- | --- |
| NetEase Cloud Music | Open platform for individual developers (appId + private key); official `@music163/ncm-cli` built on it does search, playback, synced lyrics, QR login. **V** Terms **U** | `weapi` endpoints incl. QR login; whether `weapi` still returns stream URLs **U** | Yes (`/weapi/login/qrcode/…`) **V** |
| QQ Music | `tencentmusic/qqmusic-skills` (Apache-2.0): search, charts, playlists, reports; returns web links, no streams or lyrics; personal use only, no bulk crawling, key revocable. **V** | Web endpoints, plain LRC lyrics; the web "zzb" signature must be re-implemented, not copied | QQ (ptlogin) and WeChat **V** |
| Kugou | Mini-program and IoT SDK platforms only **U** | QR login only; everything else uses Android salts **V** | Yes, with the web salt **V** |
| Kuwo, Migu | None found **U** | — | — |
| Bilibili | Creator, account and live APIs; no playback API **U** | WBI-signed web endpoints **V** | Yes **V** |

Legal signals: NeteaseCloudMusicApi archived in 2024 "to protect
copyright" **V**; bilibili-API-collect taken down after a lawyer's letter on
2026-01-28 and archived 2026-01-30 **V**; PiliPala client stopped after an
infringement notice in 2025 **U**.

Wrapper licences (from LICENSE files) and secret dependence:

| Wrapper | Licence | Status | App-extracted secrets |
| --- | --- | --- | --- |
| Binaryify/NeteaseCloudMusicApi (npm) | MIT | repo archived; npm 4.32.0 (2026-05) | Yes (eapi for song URL, YRC) |
| NeteaseCloudMusicApiEnhanced/api-enhanced | MIT | active (4.41.1) | Yes (eapi default; xeapi/neapi from Android) |
| two-mountains/ncmapi-rs | MIT | 1.0.0 (2026-08) | Includes eapi, linuxapi |
| SPlayer-Dev/ncm-api-rs | WTFPL | 0.1.0 | Includes eapi, linuxapi |
| jsososo/QQMusicApi | GPL-3.0 | dormant since 2022 | No, but embeds Tencent web code |
| Rain120/qq-music-api | MIT | active | No (web, no login) |
| yakult-green-tea/qq-music-api | MIT | active | Partly (QIMEI, MQTT QR) |
| L-1124/QQMusicApi | GPL-3.0-or-later | active | Yes by default |
| `qqmusic-rs` | MIT on crates.io, GPL-3.0 in repo | 2024 | U |
| MakcRe/KuGouMusicApi | MIT | active | Yes (Android salts, RSA, WeChat secrets) |
| SocialSisterYi/bilibili-API-collect | CC BY-NC 4.0 (U) | taken down | Documented both |
| Yuelioi/bpi-rs | MIT | active | Web no; has an app-signing module |

## Lyrics sources

* LRCLIB: server code MIT (tranxuanthang/lrclib, active); `/api/get`,
  `/api/search`, publish with proof-of-work; responses carry plain, synced
  and a new word-level `lyricsfile` (YAML). **V** Data licence, key
  requirements and User-Agent etiquette **U**.
* AMLL TTML DB (amll-dev/amll-ttml-db): CC0-1.0 for contributors' own work;
  data from other providers follows their terms; keyed by NetEase, QQ,
  Apple Music and Spotify IDs; auto-converted to TTML, LRC, YRC, QRC, LYS,
  ESLRC. **V** CC0 cannot waive the songwriters' copyright in the words.
* Musixmatch: partial lyrics on free tiers; synced and word-level need paid
  tiers; attribution required. **U**
* Proprietary word-timed formats: YRC (NetEase), QRC (QQ Music, 3DES +
  zlib in transit), KRC (Kugou, XOR + zlib). **V** `jf-lyrics` can parse their
  plain-text forms; decryption keys are app secrets and stay out of our code.

## Reference projects

* Folia (chthollyphile/folia-major): **AGPL-3.0** confirmed; Electron desktop
  app, web app, PWA; animated full-screen lyrics, AI colour themes (a
  non-goal for us), local LRC/VTT/TTML/QRC/YRC/KRC. We take ideas only and do
  not read its code. **V**
* skydoves/Cloudy: **Apache-2.0**; now a Kotlin Multiplatform Compose
  effects library. No separate "Cloudy" music player with a glass design was
  found, so the brief's Cloudy reference is the library. **V**
