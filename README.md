<p align="center">
  <img src="website/assets/icon-ice.svg" width="112" height="112" alt="Jellyfesh app icon">
</p>

<h1 align="center">Jellyfesh</h1>

<p align="center">
  <strong>An elegant, lightweight music player for macOS, Windows, Linux, Android and the web.</strong><br>
  Your own music and the services you use, with lyrics that follow every word.
</p>

<p align="center">
  <a href="https://github.com/ZodiaCALs/Jellyfesh/actions/workflows/ci.yml"><img src="https://github.com/ZodiaCALs/Jellyfesh/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <a href="https://github.com/ZodiaCALs/Jellyfesh/actions/workflows/web.yml"><img src="https://github.com/ZodiaCALs/Jellyfesh/actions/workflows/web.yml/badge.svg?branch=main" alt="Jellyfesh Web status"></a>
</p>

> **Public Beta: v0.0.8 "Chorus".** One layout in every app, sound
> quality in the player bar, a mini player, search by source, your
> sources' likes and playlists, and downloads from your own servers.
> Jellyfesh is ready for daily listening and still changing quickly: read
> the [release notes](docs/releases/v0.0.8.md) for what is new, what has
> been tested and what is known not to work yet.

## Highlights

- **Your music, beautifully.** A library of your own folders, with albums,
  artists, songs, playlists, favourites and play history, fast search
  (including short Chinese and Japanese queries) and folders that stay up to
  date by themselves.
- **Lyrics that follow every word.** Word-timed lyrics from LRC, enhanced
  LRC, TTML and WebVTT, with translations and romanization, synchronised to
  within about 10 ms of what you hear, in Now Playing, a lyrics bar and a
  floating Desktop Lyrics window.
- **AI Lyrics** (Pro). Synced lyrics for songs that have none, and timing
  for plain lyrics: on your device with an open Whisper model, or online
  with your own OpenAI, Groq or self-hosted key, only after you agree.
- **Customize** (Pro). Corners, glass, aurora, typeface, text size,
  density, lyrics style and layout, applied live in every app.
- **Servers and services in one place.** Jellyfin, Navidrome and other
  Subsonic servers; QQ Music, Spotify and NetEase Cloud Music through
  official plugins; Apple Music on macOS. Every source opens on its own
  Discover page; Get Info shows the sound's real quality; your library and
  servers answer one search together, with a tab for each source; your
  sources' likes and playlists sit beside your own; each service keeps its
  own view where its terms ask for it.
- **Offline and in your control.** Download albums and playlists from your
  own Jellyfin or Navidrome server to play offline, with the folder, file
  names, quality and tags you choose. Pick the streaming quality from the
  player bar, and keep a mini player above your other windows.
- **Luminous design.** The colour of the cover fills the room: an aurora at
  the artwork's hue, glass where it helps, and a Now Playing stage with
  large lyrics and every control in one floating dock. Light and dark, with
  Reduce Motion and Transparency honoured everywhere.
- **Careful playback.** Gapless playback, crossfade that keeps albums
  continuous, ReplayGain, a ten-band equalizer with presets, and output
  device switching while playing.
- **Private and contained.** No account, no analytics and no tracking.
  Every online source is a sandboxed WebAssembly plugin that can reach only
  the servers its manifest names, within strict CPU, memory and time
  limits; sign-ins are kept in your system's keychain.
- **One app, one layout, five platforms.** The same places, words, steps
  and layout in every app, Jellyfesh Web's, each drawn natively over one
  Rust core ([`docs/ui.md`](docs/ui.md), ADR-0023). English, Simplified
  Chinese, Traditional Chinese and Japanese.

## Platforms

| App | Requirements | Status in v0.0.8 |
| --- | --- | --- |
| **macOS** (SwiftUI) | macOS 14.2 or later, Apple silicon or Intel | Beta. Built and tested by CI; not yet signed or notarized. |
| **Windows** (WinUI 3, .NET 10) | Windows 10 version 1809 or later, x64 or Arm64 | Beta. One installer; built, tested and started by CI on every change; not yet signed. |
| **Linux** (GTK 4, libadwaita) | GTK 4.14 and libadwaita 1.5 or later (Ubuntu 24.04, Debian 13, Fedora 40 and later) | Beta. Flatpak (with its GNOME runtime from Flathub); `platforms/linux/packaging/` also makes a tarball. |
| **Android** (Jetpack Compose) | Android 8 or later | Beta. Built and tested by CI; asks for Local Network access on Android 17. |
| **Jellyfesh Web** (Svelte, WebAssembly) | A current Chromium-based browser (others untested) | Beta. Online at the website's `/app/` and as a static folder to host yourself. |

The macOS, Windows and Android apps are built and tested by continuous
integration on every change (the Windows app is also started there) but
have not yet been run on physical devices by the project; see the release
notes' known limitations. Jellyfesh is free; **Jellyfesh Pro** adds AI
Lyrics and Customize and is free for everyone during the public beta
([ADR-0021](docs/adr/0021-jellyfesh-pro.md)).

## Sources

| Source | What you get | What you need |
| --- | --- | --- |
| **Your music folders** | The full library: playback, lyrics, playlists, favourites | Nothing |
| **Jellyfin** | Browse, search and stream your server | Your Jellyfin server |
| **Navidrome, Subsonic, OpenSubsonic** | Browse, search and stream your server | Your server (Navidrome, Airsonic, Gonic, Ampache and others) |
| **QQ Music** (official API, beta) | Search, charts, daily picks and playlists; songs open in QQ Music | Your own personal QQ Music API key |
| **Spotify** (remote control, beta) | Search and browse your library, then play on your own Spotify devices with Jellyfesh as the remote | Spotify Premium; sign in with your Spotify account (or use your own Spotify developer app) |
| **NetEase Cloud Music** (official open platform, beta) | Your playlists, streamed in Jellyfesh | Your own NetEase open-platform App ID and key |
| **Apple Music** (macOS, beta) | Search and play the Apple Music catalogue through MusicKit | An Apple Music subscription, and a build signed with Jellyfesh's MusicKit identity (not yet available) |
| **NetEase Cloud Music** (unofficial, community) | Search, your liked songs and playlists, streaming and synced lyrics | Sign in by QR code with your NetEase Cloud Music account; allow community plugins first |
| **QQ Music** (unofficial, community) | Search, your liked songs and playlists, streaming and synced lyrics | Sign in by QR code with WeChat; allow community plugins first |
| **Other community plugins** | Sources made by others, off until you allow them | Your consent to the community-plugin disclosure |

Official plugins are bundled with the apps and signed with the Jellyfesh
release key (see the release notes' "Signing and trust"); each uses the
service's own official interface. The two **unofficial community plugins**
let you sign in to NetEase Cloud Music or QQ Music with just your account
and a QR code. Builds made from source include them; from v0.0.7 the
release downloads leave them out, and they come as a separate download,
`Jellyfesh-<version>-Community-Plugins.zip`, installed with **Install from
File…** (ADR-0021). They use the services' private web
interfaces, are not signed by Jellyfesh, and stay off until you turn on
**Allow Community Plugins** and accept their disclosure (they may break when
a service changes, and may be against its terms). No plugin works around a
paywall, region lock or other restriction. All service plugins have so far
been tested against mock services only. Plugin pages:
[`docs/plugins/`](docs/plugins/).

## Download

Releases are published on the
[GitHub releases page](https://github.com/ZodiaCALs/Jellyfesh/releases)
with one download for each platform: `Jellyfesh-<version>-macOS.dmg`,
`-Windows-Setup.exe` (x64 and Arm64, installed for you without administrator
rights), `-Linux.flatpak`, `-Android.apk` and `-Web.zip`, plus
`-Community-Plugins.zip` with the unofficial plugins, which these builds do
not include, and `SHA256SUMS.txt`. Each release's text says how to install
each file. A build that is not signed yet says so there: macOS then asks you
to allow the app once in System Settings › Privacy & Security, and Windows
SmartScreen may ask you to confirm.

## Building from source

Jellyfesh is a Rust workspace (the toolchain is pinned in
`rust-toolchain.toml`) with one app per platform under
[`platforms/`](platforms/).

```bash
cargo test --workspace                                  # the core, plugin host, CLI and the Linux app
cargo run --release -p jellyfesh-gtk                    # the Linux app (needs GTK 4.14+ and libadwaita 1.5+)
cargo run --release -p jf-cli -- scan ~/Music           # jf, the command-line player
cargo run --release -p jf-cli -- play "an album" --lyrics
```

Each app's build is described in [`docs/macos.md`](docs/macos.md),
[`docs/windows.md`](docs/windows.md), [`docs/android.md`](docs/android.md)
and [`docs/web.md`](docs/web.md); the Linux packaging is in
[`platforms/linux/packaging/`](platforms/linux/packaging/). Plugins are
built with [`tools/jf-plugin`](tools/jf-plugin/) and the SDK in
[`plugins/sdk`](plugins/sdk/) ([`docs/plugins/authoring.md`](docs/plugins/authoring.md)).

## Documentation

| Topic | Where |
| --- | --- |
| Release notes and version history | [`docs/releases/`](docs/releases/), [`CHANGELOG.md`](CHANGELOG.md) |
| One UI in every app (places, words, settings, keys, Luminous) | [`docs/ui.md`](docs/ui.md) |
| The core API and the FFI | [`docs/core-api.md`](docs/core-api.md), [`docs/ffi.md`](docs/ffi.md) |
| Plugins: SDK, authoring, official plugins | [`docs/plugins/`](docs/plugins/) |
| Architecture and decision records | [`docs/architecture.md`](docs/architecture.md), [`docs/adr/`](docs/adr/) |
| Testing | [`docs/testing.md`](docs/testing.md) |
| Publishing (CI, releases, the website) | [`docs/github.md`](docs/github.md) |
| Design tokens and controls | [`design/tokens.json`](design/tokens.json), [`docs/design-tokens.md`](docs/design-tokens.md), [`docs/design-controls.md`](docs/design-controls.md) |
| Repository layout | [`docs/repo-layout.md`](docs/repo-layout.md) |

## Feedback and security

Please report bugs and ideas through
[GitHub Issues](https://github.com/ZodiaCALs/Jellyfesh/issues). Report
security problems privately through the repository's **Security** tab
(Report a vulnerability) rather than in a public issue.

## Legal

Copyright © 2026 ZodiaCALs. All rights reserved. No licence has been
granted yet (see [ADR-0016](docs/adr/0016-distribution-licence-versions.md));
third-party components keep their own licences, listed in
`THIRD_PARTY_NOTICES.md` inside every app. Drafts of the licence agreement,
privacy policy and terms of sale for Jellyfesh Pro are in
[`legal/`](legal/README.md); they are not in force until a lawyer has
reviewed them.

QQ Music is a trademark of Tencent; Spotify is a trademark of Spotify AB;
NetEase Cloud Music is a trademark of NetEase, Inc.; Apple Music is a
trademark of Apple Inc.; Jellyfin, Navidrome and Subsonic belong to their
respective owners. Jellyfesh is an independent project and is not
affiliated with, endorsed by or sponsored by any of them.
