# ADR-0014: Dependency baseline

* Status: Proposed
* Checked: 2026-11-05, against crates.io's API, Maven Central, NuGet,
  GitHub releases and vendor release notes (sources in each ADR).

## Decision

Start Milestone 1 on the versions below, pinned in lock files. Update on a
fixed day each month, one dependency family per commit, with the relevant
tests and budgets re-run. A dependency that ships breaking majors often
(rubato, wasmtime, Windows App SDK) sits behind a thin wrapper of ours.

### Rust core

| Crate | Version (date) | Licence | Notes |
| --- | --- | --- | --- |
| Rust toolchain | 1.97 stable; workspace MSRV ≥ 1.92 | MIT/Apache | gtk4 0.11 needs 1.92; lofty and others 1.89 |
| symphonia | 0.6.1 (2026-08-13) | MPL-2.0 | Gapless via `AudioDecoderOptions::gapless`; gaps in ADR-0008 |
| symphonia-adapter-libopus | 0.3.0 (2026-05-16) | MIT/Apache | libopus 1.6.1 through `opusic-sys` (BSD-3), CMake build |
| cpal | 0.18.2 (2026-08-16) | Apache-2.0 | AAudio on Android (API 26+); WASAPI shared; macOS 14.2+ |
| rubato | 5.0.1 (2026-10-01) | MIT/Apache | Behind our `Resampler` trait |
| lofty | 0.25.5 (2026-10-25) | MIT/Apache | ReplayGain, ISRC, pictures, USLT/SYLT/©lyr |
| rusqlite | 0.41.1 (2026-10-28), bundled SQLite 3.54.1 | MIT | FTS5 with `trigram` |
| notify | 8.2.1 (2026-10-25) | CC0-1.0 | 9.0 is in release candidates |
| keyring | 4.2.0 (2026-08-29) | MIT/Apache | + `android-native-keyring-store` 1.0.0 |
| reqwest | 0.13.5 (2026-09-08) | MIT/Apache | rustls + aws-lc-rs + platform verifier by default |
| rustls / rustls-platform-verifier | 0.23.46 / 0.7.1 | Apache/MIT/ISC | Android needs the Kotlin verifier component |
| stream-download | 0.24.5 (2026-10-28) | MIT/Apache | Candidate for the seekable HTTP reader (MSRV 1.95) |
| image | 0.25.10 (2026-03-10) | MIT/Apache | JPEG via zune-jpeg 0.5.15 |
| fast_image_resize | 6.1.0 (2026-07-21) | MIT/Apache | Downsampling before the backdrop blur |
| libblur | 0.24.0 (2026-05-09) | Apache-2.0 OR BSD-3 | Candidate Gaussian blur for the bake |
| biquad | 0.6.0 (2026-03-22) | MIT/Apache | Or hand-written RBJ biquads |
| mpris-server / zbus | 0.11.0 / 6.2.1 | MPL-2.0 / MIT | Linux shell; zbus 5 vs 6 split noted in R18 |
| blocking (zbus's thread pool) | **1.6.2 held** (1.7.0 is current, 2026-08-23) | MIT/Apache | Checked 2026-11-26: 1.7.0 never retires its last idle thread, which then wakes every 500 ms for the life of the process; zbus connects every Unix socket on it (the core's keystore check, MPRIS). Idle Linux app: 2.2 wake-ups/s on 1.7.0, 0.2 on 1.6.2. `deny.toml` bans ≥ 1.7.0 until a release retires idle threads again |
| gtk4 / libadwaita (Rust) | 0.11.5 (`v4_24`) / 0.9.2 (`v1_10`) | MIT | Linux shell |

### Platforms

| Platform | Baseline |
| --- | --- |
| macOS | Xcode 27.2, Swift 6.4; deployment target macOS 14.2; universal binary |
| Windows | Windows App SDK 2.6.x (2.6.5, 2026-11-03); .NET 10 LTS (.NET 8 and 9 end support 2026-11-10); deployment Windows 10 1809+; self-contained |
| Linux | Flatpak, GNOME 51 runtime (GTK 4.24, libadwaita 1.10) |
| Android | Compose BOM 2026.09.00 (Compose 1.12.1); Media3 1.11.1; Cloudy 0.7.1 (Haze 2.1.0 as alternative); minSdk 26, targetSdk current |

### FFI and WebAssembly

| Component | Version (date) | Notes |
| --- | --- | --- |
| uniffi | **0.31.x pinned** (0.32.2 is current, 2026-09-23) | Held at the minor uniffi-bindgen-cs supports (ADR-0002) |
| uniffi-bindgen-cs | v0.11.1+v0.31.0 (2026-10-09), from a git tag | Not on crates.io |
| cbindgen / csbindgen | 0.29.4 / 1.9.8 | Fallback C ABI path only |
| wasmtime | **48.x LTS** (50.0.1 current, 2026-10-29) | `component-model`, no WASI, no component-model-async (ADR-0003) |
| wit-bindgen / wasm-tools | 0.63.0 (2026-10-14) / 1.263.0 | Guest builds for `wasm32-unknown-unknown` |
| wasmi | 2.0.0 (2026-09-01) | Reference in spike S6 only |
| .NET | 10 LTS | NativeAOT checked in S8 |

## Consequences

* Several pins are younger than six months; the monthly update day and the
  contract tests catch regressions early.
* `cargo-deny` enforces ADR-0016's licence policy (allow MIT, Apache-2.0,
  BSD, ISC, Zlib, CC0, Unicode-3.0, MPL-2.0, dynamically linked LGPL; deny
  GPL, AGPL, SSPL and unlicensed code) and flags duplicate majors.
