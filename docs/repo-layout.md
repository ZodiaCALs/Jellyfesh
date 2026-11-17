# Repository layout

Status: **proposed (Step 0)**. One monorepo: the core, the four shells,
the plugin SDK, first-party plugins and the design source move together, and
FFI contract tests can run on every change.

```text
jellyfesh/
├── Cargo.toml                 Rust workspace (core, plugin SDK/CLI, Linux shell, xtask)
├── rust-toolchain.toml        pinned stable toolchain (MSRV ≥ 1.92, ADR-0014)
├── deny.toml                  cargo-deny: licences, advisories, duplicate versions
├── NOTICE  LICENSES/          attribution (Misty tokens, Apache-2.0)
│
├── core/                      headless Rust core: no UI code, no platform SDKs
│   ├── jf-engine/             decode, DSP, gapless, crossfade, output, HTTP streaming
│   ├── jf-library/            SQLite catalogue, scan, watch, FileSystem trait
│   ├── jf-lyrics/             LRC, enhanced LRC, TTML, WebVTT → LyricDoc
│   │   └── tests/golden/      input files + expected LyricDoc JSON (golden tests)
│   ├── jf-palette/            artwork hue, Misty theme resolution, backdrop bake
│   ├── jf-plugin-host/        wasm runtime, host imports, limits, auth flows, search merge
│   ├── jf-plugin-model/       the plugin host's wasmtime-free parts (manifest, packages, trust,
│   │                          schema, merge, request rules, auth state machine); browser-safe
│   ├── jf-web/                Jellyfesh Web's logic in WebAssembly (queue, rules, sign-in, lyrics,
│   │                          themes); media/ (artwork, tags), sandbox/ (plugin checks, crypto)
│   ├── jf-session/            queue, history, persistence, media-session state
│   ├── jf-core/               actor facade: Command in, Snapshot/Event out; settings; secrets trait
│   ├── jf-tokens/             generated design tokens for Rust
│   └── jf-ffi/                UniFFI scaffolding (Swift, Kotlin) + C# bindings (ADR-0002)
│
├── apps/
│   └── cli/                   jf: command-line player over jf-core (Milestone 1)
│
├── platforms/                 thin native shells: views + platform adapters only
│   ├── macos/                 Xcode project, SwiftUI app, Swift package wrapping the XCFramework
│   │   ├── App/               views, view models that mirror snapshots
│   │   ├── Adapters/          NowPlaying, RemoteCommands, Keychain SecretStore, MusicKit player
│   │   └── Generated/         Swift bindings + tokens (git-ignored, built by xtask)
│   ├── windows/               WinUI 3 solution (C#)
│   │   ├── Jellyfesh.App/     views (XAML), view models that mirror snapshots
│   │   ├── Jellyfesh.Adapters/ SMTC, taskbar, Credential Manager, MusicKit-JS host
│   │   └── Generated/         C# bindings + tokens (git-ignored)
│   ├── linux/                 GTK4 + libadwaita app, a Rust crate in the workspace
│   │   ├── src/               widgets, MPRIS adapter, Secret Service adapter
│   │   └── data/              .desktop, metainfo, icons, GResource, tokens.css (generated)
│   ├── android/               Gradle project (Kotlin, Compose)
│   │   ├── app/               activities, Compose screens, GlassSurface (Cloudy behind it)
│   │   ├── adapters/          Media3 session + SimpleBasePlayer facade, SAF FileSystem,
│   │   │                      Keystore SecretStore, MusicKit for Android player
│   │   └── core-bindings/     UniFFI Kotlin + jniLibs (generated)
│   └── web/                   Jellyfesh Web: Svelte 5 + Vite views, Web Audio playback, plugins
│                              in Web Workers (jco), service worker; tests/ with mock servers
│
├── plugins/
│   ├── wit/                   jellyfesh:plugin WIT package (versioned, semver)
│   ├── sdk/                   jf-plugin-sdk: guest-side Rust crate (bindings, helpers, getrandom/clock shims)
│   ├── testkit/               runs a plugin in the real host against a mock HTTP server (tests)
│   ├── subsonic/              Subsonic / OpenSubsonic (Navidrome and others) — Stream mode
│   ├── jellyfin/              Jellyfin 10.9+ — Stream mode
│   ├── community/             unofficial NetEase Cloud Music and QQ Music community plugins,
│   │                          bundled unsigned (ADR-0012, amended for v0.0.6)
│   ├── test-plugins/          small plugins for the host's happy-path and hostile tests
│   └── release.sh             builds and packs the bundled plugins into dist/plugins/,
│                              signing only the official ones
│
├── tools/jf-plugin/           jf-plugin: new | build | pack | keygen | sign | verify | inspect
│
├── community-plugins/         Other community plugins: NOT in this repo. Separate repository,
│                              separate CI, separate releases (ADR-0012)
│
├── design/
│   ├── tokens.json            single token source (Misty 1.1 base + Jellyfesh groups)
│   ├── tools/                 Misty import; codegen lives in xtask
│   └── icons/                 app icon sources (concepts now, production layers later)
│
├── tests/
│   ├── contract/              FFI contract scenarios (JSON) replayed by every shell's tests
│   ├── fixtures/audio/        short CC0/self-made audio files: gapless pairs, codecs, tags
│   └── perf/                  scripted runs for cold start, idle CPU, 8-hour soak
│
├── xtask/                     cargo xtask: tokens codegen, bindings, XCFramework, AAR, MSIX, Flatpak
├── docs/
│   ├── step-0/                this review package
│   ├── adr/                   decision records
│   └── *.md                   architecture, risks, milestones, sources report, tokens
└── .github/workflows/         CI per target (see below)
```

## Rules that keep the shells thin

* `platforms/*` may depend only on: the generated bindings, the platform UI
  toolkit, the media-session and secrets APIs, and the vendor SDK for its
  vendor adapter. A CI check fails on any other dependency.
* Shell view models hold a mirror of core snapshots and forward intents.
  No sorting, filtering, matching, persistence or networking in a shell.
* Generated bindings are never committed; `cargo xtask bindings`
  regenerates them. The Rust token module (`core/jf-tokens/src/generated.rs`)
  is the one exception: it is committed so the workspace builds without a
  generation step, and CI runs `cargo xtask tokens --check` to prove it
  matches `design/tokens.json`.
* `apps/cli` follows the same rule as the shells: it depends on `jf-core`
  only.

## CI (GitHub Actions, proposed)

| Job | Runner | Runs |
| --- | --- | --- |
| core | ubuntu, macos, windows | fmt, clippy, unit, golden, property tests, cargo-deny |
| plugin-host | ubuntu | sandbox and limit tests against hostile fixture plugins |
| contract-swift | macos | build XCFramework, Swift contract tests |
| contract-kotlin | ubuntu + Android emulator | build AAR, instrumented contract tests |
| contract-cs | windows | build DLL + C# bindings, .NET contract tests |
| linux-shell | ubuntu (Flatpak) | build, widget smoke test under Xvfb and headless Wayland |
| web | ubuntu | jf-web crates, Jellyfesh Web build, vitest, Playwright against mock servers; GitHub Pages deploy (`web.yml`) |
| perf | self-hosted per OS (later) | cold start and idle-CPU checks against budgets |

Store-build jobs never check out `community-plugins/`.
