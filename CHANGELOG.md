# Changelog

All notable changes to Jellyfesh are documented in this file. The format
follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and
versions follow [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html):
until 1.0.0, any release may change anything.

Every release is an annotated git tag with release notes in
[`docs/releases/`](docs/releases/) and downloads on the GitHub release page.
Design steps before the first code release are tagged `step-0.x`
(ADR-0016).

## [Unreleased]

## [0.0.6] - 2026-11-17

The first public beta: the Luminous design in every app, official plugins
for QQ Music, Spotify and NetEase Cloud Music, Apple Music prepared on
macOS, and item links and remote playback in the core
([release notes](docs/releases/v0.0.6.md)).

### Added

- **Official plugins:** QQ Music (official API; search, charts, daily picks
  and playlists, opening in QQ Music), Spotify (beta; remote control of
  your own devices with your own Client ID) and NetEase Cloud Music (beta;
  your playlists, streamed, with your own open-platform credentials). QQ
  Music and Spotify are in Add a Source.
- **Open in {Service}** for every item whose source declares a link, in
  every app; items that cannot play in Jellyfesh open their link as their
  main action.
- **Remote playback:** playing a remote source's songs starts a remote
  session that the player bar, mini-player and Now Playing follow, with
  Playing on {Device}, a Devices picker, the device's volume and Stop
  Controlling (the core's `Remote` slice and `Command::Remote`).
- **Apple Music on macOS** through MusicKit, offered only in builds signed
  with Jellyfesh's MusicKit identity (not this release's builds).
- **Plugin API 1.1:** the `remote` interface, `[links]`, RSA signing with a
  key kept in the keystore, and a fixed OAuth redirect port; 1.0 plugins
  load unchanged.
- `ExternalPlayer` in the core and the FFI, for players an app provides.

### Changed

- **The Luminous design** in the macOS, Windows, Linux and Android apps,
  Jellyfesh Web and the website: a stronger aurora at the artwork's hue,
  floating glass panels, Now Playing as a stage with 40-point lyrics and
  one control dock, 48-point page titles and lifted covers; new token
  values and `field.aurora-strength`.
- The bundled Subsonic and Jellyfin plugins are 0.0.6, built with plugin
  SDK 1.1 (still installable in v0.0.5).
- `rsa` 0.10.0, which fixes RUSTSEC-2023-0071.
- This changelog follows Keep a Changelog.

### Fixed

- Linux: switching to dark left Now Playing on the light backdrop.
- Android: songs that cannot play showed "File not found" instead of their
  reason.
- Search lists on Linux started at the wrong song when some rows could not
  play.

## [0.0.5] - 2026-11-15

One app on five platforms, a music library and plugin installs in
Jellyfesh Web, Add a Source everywhere, and the fixes from the first CI
builds of the macOS, Windows and Android apps
([release notes](docs/releases/v0.0.5.md)).

### Added

- **Add a Source** in every app: pick Jellyfin, Navidrome or another
  Subsonic server, enter its address and sign in (`jf_plugin_model::presets`,
  `Command::ConnectSource`); the Sources page lists Your Sources and opens
  Sources Settings….
- **Jellyfesh Web:** a library of the person's own folders
  (`jf_web::library`, by `jf-library`'s rules), and installing and
  uninstalling plugins from `.jfpkg` files (transpiled in the browser).
- **Core:** `jf_plugin_model::install`, the install rules shared with the
  web; `eq_presets()` through the FFI.
- Missing features filled in across the apps: Play, Shuffle and sort menus
  with Descending, Most Played, More menus on album, artist and playlist
  pages, Go to Artist, playlist import and export on Android, equalizer
  presets from the core with Preamp and Reset to Flat, one sign-in button
  per method on Windows, Ctrl+F for Search.

### Changed

- **One UI** (`docs/ui.md`): the same places, Settings sections and groups,
  words (Title Case everywhere, Windows included), confirmations and keys in
  the macOS, Windows, Linux and Android apps and Jellyfesh Web, each in its
  platform's look; Jellyfesh Web takes the apps' layout.
- Official plugins 0.0.5, with Title Case labels.

### Fixed

- A server address typed on macOS was saved only after pressing Return.
- Settings forms listed a plugin's settings alphabetically instead of in the
  plugin's order.
- Windows reported a missing music folder as a generic error.
- The first CI builds of the macOS and Android apps, and tests that failed
  now and then.

## [0.0.4] - 2026-11-13

Jellyfesh Web, a design pass on every app within Misty, and one-command
publishing on GitHub ([release notes](docs/releases/v0.0.4.md)).

### Added

- **Jellyfesh Web** (`platforms/web`, `core/jf-web`): Jellyfesh in the
  browser for Subsonic, Navidrome and Jellyfin servers, with Now Playing,
  word-timed lyrics, the queue, search, browse, sign-in and dropped files,
  in four languages; the core's logic compiled to WebAssembly and the
  official plugins in Web Workers; installable, and on the website at
  `/app/`. Tested in Chromium only.
- **Core:** the settings `transparency`, `lyrics_size` and `lyrics_align`;
  `jf_palette::glass_alpha` with contract vectors and Swift, Kotlin and C#
  ports; `jf-plugin-model`, the plugin host's wasmtime-free half.
- **Publishing:** `tools/release/push-to-github.sh` and `docs/github.md`;
  `web.yml` (Jellyfesh Web's tests and the GitHub Pages deploy);
  `release.yml` builds `jellyfesh-web-vX.Y.Z.zip` and redeploys the site.

### Changed

- **Design pass (all apps):** a straight progress line with a hover thumb
  instead of the wave; a player bar for wide windows; Now Playing and
  library motion; Appearance settings with accent swatches and a hue
  slider, transparency, lyrics size and alignment.
- `jf-session`'s SQLite store moved behind a default feature.

### Fixed

- A seek while paused could show the old position until the next update.
- The third-party notices left out the crates inside the bundled plugins.
- A core test depended on playback speed.

## [0.0.3] - 2026-11-10

The native apps, online sources through sandboxed plugins, and the
everyday features of a music player ([release notes](docs/releases/v0.0.3.md)).

### Added

- **Apps:** Linux (GTK 4, libadwaita), macOS (SwiftUI), Windows (WinUI 3,
  .NET 10) and Android (Jetpack Compose).
- **Online sources:** the plugin host (wasmtime, WIT, sandbox and limits,
  signed packages, community disclosure), official Subsonic/OpenSubsonic and
  Jellyfin plugins bundled in every app, unified search, browsing,
  streaming, sign-ins in the system keystore, the plugin SDK and
  `jf-plugin`.
- **Playback:** crossfade with album continuity, switching output devices
  while playing, HTTP streaming, M4A/AAC gapless, Opus.
- **Library:** playlists with M3U8, favourites, play statistics, artist
  pages, thumbnails, folder watching.
- **Bindings:** UniFFI Swift and Kotlin, a C ABI with C#, drawing-time ports
  with contract tests, design tokens for every platform.

### Fixed

- A track could play and count twice when playback ran ahead of the core.
- A fixed hue now colours every app.

## [0.0.2] - 2026-11-06

Fixes from testing through a real audio output, and exact, cheap animation
of progress and lyrics ([release notes](docs/releases/v0.0.2.md)).

### Added

- `DisplayClock` and `LyricTimeline::next_change` for every app;
  lyrics-cursor contract vectors.
- A `jf play` live view with a word sweep in the artwork's accent colour, a
  translation line and a progress bar, redrawn only on change.
- The device check (and the CI `audio` job) measures lyric sync.

### Changed

- Lyrics appear within about 10 ms of being heard (0.0.1: up to 0.9 s early
  or 84 ms late).

### Fixed

- The end of the last track was cut.
- The playback clock started before the output did, running ahead and
  jumping back at each start.
- Recoverable output reports stopped playback; a real output failure hung
  instead of stopping cleanly (Play now reopens the output).

## [0.0.1] - 2026-11-06

First code release: the Rust core and `jf`, a command-line player
([release notes](docs/releases/v0.0.1.md)).

### Added

- **Engine:** decoding, ReplayGain, resampling, a 10-band equalizer,
  limiter and volume to an audio device, a WAV file or nowhere;
  sample-exact gapless joins; the playback anchor.
- **Library:** SQLite catalogue, incremental scan, tags, artwork cache,
  embedded and sidecar lyrics, search including short CJK queries.
- **Lyrics:** LRC, enhanced LRC, TTML and WebVTT with golden tests; cursor
  lookup.
- **Colour:** Misty themes from one hue, the artwork's hue, baked backdrops
  with guaranteed text contrast; tokens generated for Rust.
- **Session:** queue, shuffle, repeat, history, saved and restored.
- **Core and CLI:** the `jf-core` facade and `jf` (`scan`, `roots`, `ls`,
  `search`, `play`, `queue`, `lyrics`, `theme`, `devices`).

## [step-0.5] - 2026-11-05

### Added

- App icon decided (variant B; Ice light and Steel dark by default; ten
  colours in Settings → Appearance → App Icon; ADR-0017).
- Spaced capitals as the formal wordmark, and three home-page wordmark
  options (`design/brand/`).

## [step-0.4] - 2026-11-05

### Added

- Icon round 4: variant B in ten colour combinations
  (`design/icons/round4/`).
- Six wordmark options with horizontal and stacked lockups.

## [step-0.3] - 2026-11-05

### Added

- Icon round 3 with simplified 16/32 px masters (`design/icons/round3/`).
- Four control styles within Misty (`docs/design-controls.md`).

## [step-0.2] - 2026-11-05

### Added

- The owner's Step 0 answers (`docs/step-0/README.md`) and ADR-0016
  (distribution, unsigned builds, licence policy, versions and archives).
- Icon round 2 and the Now Playing style comparison.

### Changed

- Apple Music (ADR-0010) and Spotify (ADR-0011) deferred until developer
  accounts exist.

## [step-0.1] - 2026-11-05

### Added

- The first Step 0 package: architecture overview, 15 decision records,
  online sources report, risk register, milestone plan, repository layout,
  lyrics model, proposed design tokens on Misty 1.1 and five icon concepts.

[Unreleased]: https://github.com/ZodiaCALs/Jellyfesh/compare/v0.0.6...HEAD
[0.0.6]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.6
[0.0.5]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.5
[0.0.4]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.4
[0.0.3]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.3
[0.0.2]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.2
[0.0.1]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.1
[step-0.5]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/step-0.5
[step-0.4]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/step-0.4
[step-0.3]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/step-0.3
[step-0.2]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/step-0.2
[step-0.1]: https://github.com/ZodiaCALs/Jellyfesh/releases/tag/step-0.1
