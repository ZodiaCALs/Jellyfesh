# Jellyfesh (working name)

An elegant, very lightweight music player for macOS, Windows, Linux,
Android and the web. It plays your own music folders, and servers and
services through sandboxed plugins, with lyrics that follow every word.

**Status: v0.0.5 "One Voice"** ([release notes](docs/releases/v0.0.5.md)).
Five apps over one Rust core, with the same places, words, settings and
steps in each ([`docs/ui.md`](docs/ui.md)):

| App | State |
| --- | --- |
| **Linux** (GTK 4, libadwaita) | Built, tested and run here. |
| **macOS** (SwiftUI) | Built and tested by CI (111 tests); not run on a Mac yet. |
| **Windows** (WinUI 3, .NET 10) | Built and tested by CI (105 tests); not run on Windows yet. |
| **Android** (Jetpack Compose) | Built and tested by CI (149 logic tests); not run on a device yet. |
| **Jellyfesh Web** (Svelte, WebAssembly) | Built, tested and run here (Chromium); on the website at `/app/` ([`docs/web.md`](docs/web.md)). |

What every app does:

* **Your music folders:** a library of albums, artists and songs with
  search, sorting, playlists, favourites and play history (Jellyfesh Web:
  folders read through the browser, without playlists or history).
* **Servers and services:** **Add a Source** connects Jellyfin, Navidrome or
  another Subsonic server; official plugins for Subsonic/OpenSubsonic and
  Jellyfin are bundled, other plugins install from `.jfpkg` files, and every
  plugin runs in a sandbox with the limits of [ADR-0004](docs/adr/0004-plugin-packaging-and-sandbox.md).
* **Now Playing** with the artwork or the Misty field, word-timed lyrics
  with translation and romanization, the queue and the song's details.
* **Playback:** gapless, crossfade with album continuity, ReplayGain,
  a ten-band equalizer with presets, output-device switching (not in the
  browser).
* **Misty**, Jellyfesh's design language: accent colours that follow the
  artwork, glass you can make clearer or solid, reduced motion, light and
  dark. English, Simplified Chinese, Traditional Chinese and Japanese.

```bash
cargo run --release -p jellyfesh-gtk                       # the Linux app (GTK 4.14+, libadwaita 1.5+)
cargo run --release -p jf-cli -- scan ~/Music              # the command-line player
cargo run --release -p jf-cli -- play "an album title" --lyrics
```

Downloads for every platform are on the GitHub release page; each app's
build notes are in [`docs/macos.md`](docs/macos.md),
[`docs/windows.md`](docs/windows.md), [`docs/android.md`](docs/android.md)
and [`docs/web.md`](docs/web.md); the Linux app's packaging is in
[`platforms/linux/packaging/`](platforms/linux/packaging/). To publish the
repository on GitHub with CI, releases and the website:
[`docs/github.md`](docs/github.md).

No licence has been chosen yet: all rights reserved (see ADR-0016).

| Area | Where |
| --- | --- |
| Release notes and version history | [`docs/releases/`](docs/releases/), [`CHANGELOG.md`](CHANGELOG.md) |
| One UI in every app (places, words, settings, keys) | [`docs/ui.md`](docs/ui.md) |
| Publishing on GitHub (CI, releases, website) | [`docs/github.md`](docs/github.md), [`tools/release/push-to-github.sh`](tools/release/push-to-github.sh) |
| How to test | [`docs/testing.md`](docs/testing.md) |
| Rust core and CLI | [`core/`](core/), [`apps/cli/`](apps/cli/) |
| The core API every app uses | [`docs/core-api.md`](docs/core-api.md), [`docs/ffi.md`](docs/ffi.md) |
| Apps | [`platforms/`](platforms/) (linux, macos, windows, android, web, shared) |
| Jellyfesh Web (browser) | [`platforms/web/`](platforms/web/), [`core/jf-web/`](core/jf-web/), [`docs/web.md`](docs/web.md) |
| Plugins: SDK, official plugins, authoring | [`plugins/`](plugins/), [`tools/jf-plugin/`](tools/jf-plugin/), [`docs/plugins/`](docs/plugins/) |
| Online sources: API and terms findings | [`docs/sources-report.md`](docs/sources-report.md) |
| Step 0 review package (design decisions) | [`docs/step-0/README.md`](docs/step-0/README.md) |
| Architecture overview | [`docs/architecture.md`](docs/architecture.md) |
| Decision records | [`docs/adr/`](docs/adr/) |
| Repository layout | [`docs/repo-layout.md`](docs/repo-layout.md) |
| Risks | [`docs/risks.md`](docs/risks.md) |
| Milestones | [`docs/milestones.md`](docs/milestones.md) |
| Design tokens | [`design/tokens.json`](design/tokens.json), [`docs/design-tokens.md`](docs/design-tokens.md) |
| Control styles | [`docs/design-controls.md`](docs/design-controls.md) |
| App icon and wordmark | [`design/icons/`](design/icons/), [`design/brand/`](design/brand/) |
