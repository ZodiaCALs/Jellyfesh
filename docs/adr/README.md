# Architecture decision records

Each record states the context, the options, the decision, what was
verified and where, and what would make us revisit it. Status values:
**Proposed** (awaiting your approval), **Accepted**, **Superseded**.
Facts carry a check date because library and platform facts drift; ADR-0014
holds the pinned versions.

| ADR | Title | Verify item from the brief | Status |
| --- | --- | --- | --- |
| [0001](0001-headless-core-and-data-flow.md) | Headless core, unidirectional data flow, playback anchor | | Proposed |
| [0002](0002-ffi-bindings.md) | FFI: UniFFI for Swift and Kotlin; C# over a C ABI | "Verify current maturity" (uniffi-bindgen-cs) | Accepted (updated) |
| [0003](0003-plugin-runtime.md) | Plugin runtime: wasmtime (wasmi cannot host WIT components) | "Benchmark both" | Proposed |
| [0004](0004-plugin-packaging-and-sandbox.md) | Plugin package, WIT world, permissions, signing | | Proposed |
| [0005](0005-android-glass.md) | Android glass: Cloudy behind `GlassSurface` | "Pin its version, backdrop API may be alpha"; Cloudy licence | Proposed |
| [0006](0006-linux-glass.md) | Linux glass: GTK blur render nodes | "GTK blur render nodes (verify)" | Proposed |
| [0007](0007-macos-windows-glass.md) | macOS and Windows materials | | Proposed |
| [0008](0008-codecs-and-gapless.md) | Codec coverage and gapless playback | "Check current docs and versions" (symphonia) | Proposed |
| [0009](0009-audio-output.md) | Audio output and device switching | AAudio/Oboe on Android (cpal) | Proposed |
| [0010](0010-apple-music.md) | Apple Music vendor adapters | Service API and terms | Deferred (commercial) |
| [0011](0011-spotify.md) | Spotify: Remote mode and Web API only | Service API and terms | Deferred (commercial) |
| [0012](0012-community-plugins.md) | Community plugins: distribution and limits | Wrapper licences; signing components | Proposed |
| [0013](0013-design-language.md) | Misty design language, tokens and deviations | | Proposed |
| [0014](0014-dependency-baseline.md) | Dependency baseline (checked 2026-11-05) | "Check current docs and versions" | Proposed |
| [0015](0015-secrets-and-credentials.md) | Secrets and credentials | OS keystores; no stored passwords | Proposed |
| [0016](0016-distribution-licence-versions.md) | Distribution, licence and versions | Your answers 2, 3, 9, 14 | Accepted |
| [0017](0017-app-icon-defaults-and-picker.md) | App icon defaults and the icon picker | Your icon decision | Accepted |
| [0018](0018-plugin-api-1-1.md) | Plugin API 1.1: links and remote sessions | Service APIs (v0.0.6) | Accepted |
| [0019](0019-ai-lyrics.md) | AI Lyrics: plugin API 1.2 audio and speech, Whisper on the device | Your AI lyrics answers (v0.0.7) | Accepted |
| [0020](0020-customize.md) | Customize: appearance plugins and the Look slice | Your Customize answer (v0.0.7) | Accepted |
| [0021](0021-jellyfesh-pro.md) | Jellyfesh Pro, editions and commercial readiness | Your Free + Pro answer (v0.0.7) | Accepted |
| [0022](0022-discover-details-comments.md) | Discover, details, comments, covers and the lyrics bar | Your v0.0.7 additions | Accepted |
| [0023](0023-one-layout-web-reference.md) | One layout in every app: Jellyfesh Web's | Your v0.0.8 UI answer | Accepted |
| [0024](0024-source-library-search-by-source.md) | Sources' likes and playlists, and search by source (API 1.3) | Your v0.0.8 import answer | Accepted |
| [0025](0025-downloads.md) | Downloads from your own servers | Your v0.0.8 download request | Accepted |
| [0026](0026-quality-menu-mini-player.md) | Streaming quality in the bar, and the mini player window | Your v0.0.8 bar and window requests | Accepted |
| [0027](0027-source-rows-ready-search-one-keychain-item.md) | Source songs keep their album, artist and link; search asks ready sources; one Keychain item on macOS | Your v0.0.8 screenshots | Accepted |

The service-by-service findings behind 0010–0012 are in
[`../sources-report.md`](../sources-report.md).
