# Changelog

Every version is an annotated git tag, delivered as a `.zip` snapshot and a
`.bundle` with the full history (`git clone jellyfesh-<tag>.bundle
jellyfesh`). See ADR-0016.

## Unreleased

* **Add a Source** in Sources on every app and in Jellyfesh Web: pick
  Jellyfin, Navidrome or another Subsonic server, enter its address and sign
  in. A service is listed only while a plugin signed by Jellyfesh for it is
  installed; more services join the list as their plugins are ready
  (`jf_plugin_model::presets`, `Command::ConnectSource`).
* Settings forms list a plugin's settings in the plugin's own order (the
  server address first), on every app.
* macOS: a plugin's text settings show a bordered field with the plugin's
  description, and what is typed is saved without pressing Return, also
  just before signing in.

## v0.0.4 — 2026-11-13

Jellyfesh Web, a design pass on every app after Apple Music and Windows
Media Player within Misty, and one-command publishing on GitHub. Full
notes: `docs/releases/v0.0.4.md`.

* **Jellyfesh Web** (`platforms/web`, `core/jf-web`): Jellyfesh in the
  browser for Subsonic, Navidrome and Jellyfin servers, with Now Playing,
  word-timed lyrics, the queue, search, browse, sign-in and dropped files,
  in four languages; the core's logic compiled to WebAssembly and the
  official plugins in Web Workers; installable, and on the website at
  `/app/`. Tested in Chromium only.
* **Design pass (all apps):** a straight progress line with a hover thumb
  instead of the wave; a player bar for wide windows and a straight line on
  the mini-player; Now Playing and library motion; Preferences → Appearance
  with accent swatches and a hue slider, transparency, lyrics size and
  alignment.
* **Core:** settings `transparency`, `lyrics_size`, `lyrics_align`;
  `jf_palette::glass_alpha` with contract vectors and Swift, Kotlin and C#
  ports; `jf-plugin-model`, the plugin host's wasmtime-free half;
  `jf-session`'s SQLite store behind a default feature.
* **Publishing:** `tools/release/push-to-github.sh` and `docs/github.md`;
  `web.yml` (Jellyfesh Web's tests and the GitHub Pages deploy);
  `release.yml` builds `jellyfesh-web-vX.Y.Z.zip` and redeploys the site.
* **Fixed:** a seek while paused could show the old position until the next
  update (the engine announced an anchor before storing it); the notices
  left out the crates inside the bundled plugins; a core test depended on
  playback speed.

## v0.0.3 — 2026-11-10

The native apps, online sources through sandboxed plugins, and the
everyday features of a music player. Full notes: `docs/releases/v0.0.3.md`.

* **Apps:** Linux (GTK 4, libadwaita; built and run here), macOS (SwiftUI),
  Windows (WinUI 3, .NET 10) and Android (Compose). The last three were never
  compiled here; CI builds them for the first time.
* **Online sources:** plugin host (wasmtime, WIT, sandbox and limits, signed
  packages, community disclosure), official Subsonic/OpenSubsonic and
  Jellyfin plugins (bundled in every app), unified search, browsing,
  streaming source tracks, sign-ins in the system keystore, the plugin SDK
  and `jf-plugin`.
* **Playback:** crossfade with album continuity, switching output devices
  while playing, HTTP streaming, M4A/AAC gapless, Opus.
* **Library:** playlists with M3U8, favourites, play statistics, artist
  pages, thumbnails, folder watching.
* **Bindings:** UniFFI Swift and Kotlin, a C ABI with C#, drawing-time ports
  with contract tests, tokens for every platform.
* **Fixed:** a track could play and count twice when playback ran ahead of
  the core; a fixed hue now colours every app.
* 663 Rust, 68 C# and 109 Kotlin tests pass; Swift is not compiled here.

## v0.0.2 — 2026-11-06

Fixes from testing through a real audio output, and exact, cheap animation
of progress and lyrics. Full notes: `docs/releases/v0.0.2.md`.

* **Fixed:** the end of the last track is no longer cut; the playback clock
  starts when the output does (no running ahead and jumping back at each
  start); recoverable output reports no longer stop playback; a real output
  failure stops cleanly instead of hanging, and Play reopens the output.
* **Animation:** lyrics appear within about 10 ms of being heard (v0.0.1: up to
  0.9 s early or 84 ms late); `DisplayClock` and
  `LyricTimeline::next_change` for every shell; lyrics-cursor contract
  vectors; a `jf play` live view with a word sweep in the artwork's accent
  colour, a translation line and a progress bar, redrawn only on change.
* **Testing:** the device check (and CI `audio` job) measures lyric sync;
  259 tests pass.

## v0.0.1 — 2026-11-06

First code release: the Rust core and `jf`, a command-line player. The
start of Milestone 1. Full notes: `docs/releases/v0.0.1.md`.

* **Engine:** decode, ReplayGain, resampling, 10-band EQ, limiter and
  volume to an audio device, a WAV file or nowhere; sample-exact gapless
  joins; playback anchor.
* **Library:** SQLite catalogue, incremental scan, tags, artwork cache,
  embedded and sidecar lyrics, search including short CJK queries.
* **Lyrics:** LRC, enhanced LRC, TTML and WebVTT with golden tests; cursor
  lookup.
* **Colour:** Misty themes from one hue, artwork hue, baked backdrops with
  guaranteed text contrast; tokens generated for Rust.
* **Session:** queue, shuffle, repeat, history, saved and restored.
* **Core and CLI:** the `jf-core` facade and `jf` (`scan`, `roots`, `ls`,
  `search`, `play`, `queue`, `lyrics`, `theme`, `devices`).
* **Release files:** Linux x86_64 `jf` with SHA-256; source zip and bundle.
* 247 tests pass. Not yet run on real audio hardware, macOS or Windows.

## step-0.5 — 2026-11-05

Icon defaults, icon picker, formal and home-page wordmarks.

* **App icon decided:** variant B; Ice is the default light icon and Steel
  the default dark icon; all ten colours selectable in Settings → Appearance
  → App icon, with per-platform behaviour in **ADR-0017**. Mock-ups of the
  defaults in the Dock, taskbar, home screen and GNOME dash, and of the
  picker.
* **Wordmark:** spaced capitals are the formal wordmark (one-colour files
  added, shown in an About window, installer and document header); three
  fancier home-page heroes (bioluminescent glass, tentacle flourish, aurora
  glass capitals) in home-page mock-ups (`design/brand/`).
* **Tokens:** `brand.icon-default-light/dark`, `brand.icon-options`,
  `brand.wordmark-formal` decided; `brand.wordmark-hero` pending.
* **Still open:** home-page hero, control style, backdrop style, codec scope,
  Android library access.

## step-0.4 — 2026-11-05

Icon B colours and the wordmark.

* **Icons, round 4:** variant B in ten colour combinations, including a dark
  (Abyss) and a light (Ice) appearance (`design/icons/round4/`).
* **Wordmark:** six options (monoline lettering in the icon's line style,
  with and without a bell on the j, on the icon's tile; Inter Display type,
  with a bell, and spaced capitals) plus horizontal and stacked lockups
  (`design/brand/`).
* **Still open:** icon colours, wordmark, control style, backdrop style,
  codec scope, Android library access.

## step-0.3 — 2026-11-05

Icon round 3 and control styles.

* **Icons, round 3:** Depth's tile with Midnight's lines, plus three
  refinements (lift, outlined core, deeper top), each with a simplified
  16/32 px master (`design/icons/round3/`).
* **Control styles:** four options for bars and buttons within Misty: A after
  Apple, B after Microsoft, C after Google, D a blend
  (`docs/design-controls.md`, `design/canvas/Controls-*.dc.html`).
* **Still open:** icon pick, control style, backdrop style, codec scope,
  Android library access.

## step-0.2 — 2026-11-05

Step 0, revision 2: your answers recorded, icons round 2, style comparison.

* **Decisions recorded** in `docs/step-0/README.md`: Still mode defined;
  website and GitHub Releases first, commercial version later; no licence
  (all rights reserved); minimum OS versions fixed; both backdrop styles to be
  built and compared; performance-first trade-off order and proposed
  reference hardware; Subsonic token in the keystore; developer accounts
  deferred; name kept; launch languages; WIT confirmed.
* **ADR-0016** (new): distribution, unsigned builds until the commercial
  phase, licence policy for dependencies, versions and archives.
* **Deferred to the commercial phase:** Apple Music (ADR-0010), Spotify
  (ADR-0011), notarisation and code signing; Milestone 5 marked deferred.
* **Icons, round 2:** eight variants of Wavebell's shape in Tide's style, in
  deep blue and cyan, with translucent, glass and outlined-core lines
  (`design/icons/round2/`).
* **Now Playing comparison:** Artwork style versus Misty style, same
  placeholder track (`design/canvas/`, `design/mockups/`).
* **Tokens:** `brand.style` (artwork | misty); unsung lyric words at ink-70.
* **Still open:** icon choice, backdrop style, codec scope, Android library
  access (options in the Step 0 index).

## step-0.1 — 2026-11-05

The first Step 0 package for review.

* Architecture overview, 15 decision records, online sources report, risk
  register, milestone plan, repository layout, lyrics model.
* Proposed design tokens on Misty 1.1 (`design/tokens.json`).
* Five app icon concepts (`design/icons/concepts/`).
