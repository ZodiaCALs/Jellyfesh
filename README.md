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

> **Public Beta: v0.0.6 "Luminous".** Jellyfesh is feature-complete enough
> for daily listening and still changing quickly. Read the
> [release notes](docs/releases/v0.0.6.md) for what is new, what has been
> tested and what is known not to work yet.

## Highlights

- **Your music, beautifully.** A library of your own folders, with albums,
  artists, songs, playlists, favourites and play history, fast search
  (including short Chinese and Japanese queries) and folders that stay up to
  date by themselves.
- **Lyrics that follow every word.** Word-timed lyrics from LRC, enhanced
  LRC, TTML and WebVTT, with translations and romanization, synchronised to
  within about 10 ms of what you hear.
- **Servers and services in one place.** Jellyfin, Navidrome and other
  Subsonic servers; QQ Music, Spotify and NetEase Cloud Music through
  official plugins; Apple Music on macOS. Your library and servers answer
  one search together; each service keeps its own view where its terms ask
  for it.
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
- **One app, five platforms.** The same places, words and steps in every
  app, each with its platform's own look, over one Rust core
  ([`docs/ui.md`](docs/ui.md)). English, Simplified Chinese, Traditional
  Chinese and Japanese.

## Platforms

| App | Requirements | Status in v0.0.6 |
| --- | --- | --- |
| **macOS** (SwiftUI) | macOS 14.2 or later, Apple silicon or Intel | Beta. Built and tested by CI; not yet signed or notarized. |
| **Windows** (WinUI 3, .NET 10) | Windows 10 version 1809 or later, x64 or Arm64 | Beta. Built and tested by CI; not yet signed. |
| **Linux** (GTK 4, libadwaita) | GTK 4.14 and libadwaita 1.5 or later (Ubuntu 24.04, Debian 13, Fedora 40 and later) | Beta. Tarball and Flatpak. |
| **Android** (Jetpack Compose) | Android 8 or later | Beta. Built and tested by CI. |
| **Jellyfesh Web** (Svelte, WebAssembly) | A current Chromium-based browser (others untested) | Beta. Online at the website's `/app/` and as a static folder to host yourself. |

The macOS, Windows and Android apps are built and tested by continuous
integration on every change but have not yet been run on physical devices
by the project; see the release notes' known limitations.

## Sources

| Source | What you get | What you need |
| --- | --- | --- |
| **Your music folders** | The full library: playback, lyrics, playlists, favourites | Nothing |
| **Jellyfin** | Browse, search and stream your server | Your Jellyfin server |
| **Navidrome, Subsonic, OpenSubsonic** | Browse, search and stream your server | Your server (Navidrome, Airsonic, Gonic, Ampache and others) |
| **QQ Music** (official API, beta) | Search, charts, daily picks and playlists; songs open in QQ Music | Your own personal QQ Music API key |
| **Spotify** (remote control, beta) | Search and browse your library, then play on your own Spotify devices with Jellyfesh as the remote | Spotify Premium and your own Spotify developer Client ID |
| **NetEase Cloud Music** (official open platform, beta) | Your playlists, streamed in Jellyfesh | Your own NetEase open-platform App ID and key |
| **Apple Music** (macOS, beta) | Search and play the Apple Music catalogue through MusicKit | An Apple Music subscription, and a build signed with Jellyfesh's MusicKit identity (not yet available) |
| **Community plugins** | Sources made by others, off until you allow them | Your consent to the community-plugin disclosure |

Official plugins are bundled with the apps and signed with the Jellyfesh
release key (see the release notes' "Signing and trust"). Each service is
used through its own official interface, with your own account and keys;
Jellyfesh never works around a paywall, region lock or other restriction.
The service plugins have so far been tested against mock services only.
Plugin pages: [`docs/plugins/`](docs/plugins/).

## Download

Releases are published on the
[GitHub releases page](https://github.com/ZodiaCALs/Jellyfesh/releases)
with SHA-256 checksums for every file. The release notes list each file
and how to install it. Builds are not yet code-signed: macOS asks you to
allow the app once in System Settings › Privacy & Security, and Windows
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
`THIRD_PARTY_NOTICES.md` in every release.

QQ Music is a trademark of Tencent; Spotify is a trademark of Spotify AB;
NetEase Cloud Music is a trademark of NetEase, Inc.; Apple Music is a
trademark of Apple Inc.; Jellyfin, Navidrome and Subsonic belong to their
respective owners. Jellyfesh is an independent project and is not
affiliated with, endorsed by or sponsored by any of them.
