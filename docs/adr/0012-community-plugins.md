# ADR-0012: Community plugins: distribution and limits

* Status: Proposed
* Date: 2026-11-05
* Findings: [`../sources-report.md`](../sources-report.md#regional-services)

## Context

Regional services (QQ Music, NetEase Cloud Music, Kugou, …) will only ever
be community, unofficial, opt-in plugins, distributed separately and never
in store builds. Reference implementations may wrap open-source wrappers
whose licences allow it, but must never bundle proprietary binaries or
signing components extracted from official apps.

**Verified facts that shape this record:**

* Most maintained wrappers depend on secrets taken from the official
  **apps**: NetEase `eapi`/`linuxapi` keys and the 2026 `xeapi`/`neapi`
  schemes; Kugou's Android signing salts, RSA keys and WeChat app secrets;
  QQ Music's QIMEI device keys and the QRC lyric 3DES key; Kugou's KRC XOR
  key; Bilibili's app `appkey`/`appsec`.
* What works without app secrets: NetEase `weapi` web endpoints including
  QR login; QQ Music web endpoints, plain LRC lyrics and QQ/WeChat QR login;
  Kugou QR login only; Bilibili WBI-signed web endpoints and web QR login.
* Some wrappers copy vendor JavaScript (jsososo/QQMusicApi embeds Tencent's
  minified sign function) — a copyright problem separate from the terms.
* Legal signals: Binaryify archived NeteaseCloudMusicApi in 2024 "to
  protect copyright"; **bilibili-API-collect was taken down and archived in
  January 2026 after a lawyer's letter from Bilibili.**
* Licences vary: MIT (api-enhanced, ncmapi-rs, Rain120 and yakult forks,
  KuGouMusicApi, bpi-rs), GPL-3.0 (jsososo, L-1124), WTFPL (ncm-api-rs),
  inconsistent (`qqmusic-rs`: MIT on crates.io, GPL-3.0 in its repo).
* **Official doors have opened**: NetEase's open platform now accepts
  individual developers and its official CLI (`@music163/ncm-cli`) searches,
  plays, shows synced lyrics and logs in by QR; Tencent publishes
  `qqmusic-skills` (Apache-2.0), an official metadata API (search, charts,
  playlists; no streams, no lyrics) for personal use only. Neither set of
  terms could be read from here (sites unreachable).
* App Store: guideline 4.7 now allows plug-ins not embedded in the binary,
  with conditions (an index of offered software with universal links,
  reporting and filtering, age gating, no exposure of native APIs without
  permission); 5.2.2 and 5.2.3 require permission from third-party services
  whose content an app streams. Google Play's executable-code policy could
  not be read from here (support.google.com unreachable).

## Decision

1. **Separate home.** Community plugins and the template live in a separate
   repository with its own CI and releases. Release CI never checks it out.
   When store builds exist (the commercial phase, ADR-0016), they load only
   packages signed with our release key.
2. **Opt-in in direct-download builds**, which is every build for now. Sources settings has "Allow
   community plugins", off by default. Turning it on, and enabling each
   unofficial plugin the first time, shows the disclosure: may violate the
   service's terms, may break at any time, for personal use only. Plugins
   are installed from a file or URL the person chooses; there is no
   built-in catalogue of unofficial plugins.
3. **Hard rules for reference implementations**, enforced by review and an
   automated scan in the community CI:
   * no proprietary binaries; no keys, salts, certificates or secrets taken
     from official apps (the list above is the denylist seed);
   * no copied vendor JavaScript; a web-derived algorithm is re-implemented
     only after a legal check ⚑;
   * web endpoints and web QR login only; anything that needs app secrets
     is simply not implemented;
   * code ported only from MIT, Apache-2.0 or BSD sources, with attribution;
     nothing from GPL, AGPL or unclear-licence projects;
   * no DRM circumvention and no paywall bypass: VIP-only or trial-limited
     tracks stay unavailable, and the plugin says why.
4. **Reference set for Milestone 6 (proposal ⚑):**

   | Service | Plan |
   | --- | --- |
   | NetEase Cloud Music | First evaluate the **official open platform**. If its terms allow a third-party player, build an official third-party plugin in Stream mode; otherwise a community plugin limited to `weapi` (search, playlists, plain lyrics, QR login; stream URLs only if `weapi` still returns them) |
   | QQ Music | Official `qqmusic-skills` metadata plugin (search, charts, playlists, deep links) if its personal-use terms fit; community playback is not provided as a reference |
   | Kugou | Template documentation only: every useful endpoint needs app salts |
   | Bilibili | No reference plugin, given the January 2026 legal action |
   | YouTube | No reference plugin: its policies forbid audio-only, background and own-engine playback (sources report) |

5. **Lyrics plugins** for LRCLIB (MIT code, data terms unknown ⚑) and the
   AMLL TTML DB (CC0 for contributors' own work, upstream providers' terms
   for the rest) are separate from playback plugins and carry their source
   attribution in `LyricDoc.source`.

## Consequences

* The reference set is smaller than the brief's list, because most
  wrappers fail the "no app-extracted secrets" rule and two services have
  taken legal action against unofficial access.
* NetEase and QQ Music may move from "community" to "official" if their new
  platforms' terms allow third-party players; checking those terms is the
  first task of Milestone 6 (or earlier if you want them sooner).

## Amendment (2026-11-17, v0.0.6): bundled community plugins

The owner chose to ship the unofficial NetEase Cloud Music and QQ Music
plugins (`community.unofficial.netease`, `community.unofficial.qqmusic`,
sign-in by QR code with the person's own account) **inside the apps**, so
nobody needs an API key or developer registration for them. They stay
community plugins in every respect: built from `plugins/community/`, never
signed with the Jellyfesh release key, shown as **Community · unofficial**,
and installed only once the person turns on **Allow Community Plugins**
(the core keeps the bundled packages back until then, without an error);
each starts off until its disclosure is accepted. The official plugins keep
to the services' official interfaces.
