# Milestone plan

Status: **in progress.** v0.0.1 and v0.0.2 delivered Milestone 1; v0.0.3
(2026-11-10) brought the first build of Milestones 2–4 together: the plugin
host, SDK and the Subsonic and Jellyfin plugins, the Linux app built and run,
and the macOS, Windows and Android apps written but not yet compiled (their
gates need the CI builds and real devices). v0.0.4 (2026-11-13) added
Jellyfesh Web, the player in the browser for Subsonic, Navidrome and Jellyfin
servers (`docs/web.md`), outside this plan, and a design pass on every app
(`docs/design-controls.md`, "v0.0.4"). Each milestone ends at a gate: its
acceptance checks pass, the budgets in scope are **measured and reported**,
and you approve before the next one starts. Every milestone is delivered as
small commits with tests.

Spikes (time-boxed experiments) come first in the milestone that depends on
them, so a bad finding changes the plan before code piles up on it.

## Budgets and how each is measured

| Budget | Definition | Tool per platform | First measured |
| --- | --- | --- | --- |
| Cold start < 1 s (desktop) | Process launch to first interactive frame with the library view filled from cache, on the reference hardware in the Step 0 index | macOS: Instruments App Launch + `os_signpost`; Windows: ETW + WPA; Linux: `perf trace` + signpost log line; Android (reported, no target yet): Macrobenchmark `StartupTimingMetric` | M2 (macOS), M4 (others) |
| Idle playback CPU ≈ 1% or less in Still mode | Mean CPU, as % of one logical core, over 10 min playing a local 44.1 kHz FLAC in **Still mode**: nothing is drawn per frame, because no window is visible (minimised, hidden, covered, screen off) or the visible window has no time-driven animation (lyrics closed, no visualiser, progress text once a second) | macOS: `powermetrics`, Instruments CPU Profiler; Windows: `typeperf` per-process, WPA; Linux: `pidstat`, `perf`; Android: Perfetto | M1 (CLI), M2, M4 |
| Memory baseline, no growth over 8 h | Baseline = steady-state footprint after M1 (CLI) and after each shell lands. Soak: 8 h mixed local + stream playlist with scripted UI actions; footprint sampled each minute; pass if the slope after the first hour is < 1 MB/h and no single step > 5 MB persists | macOS: `footprint`, Instruments Leaks/Allocations; Windows: WPA + VMMap; Linux: heaptrack, `smem`; Android: Perfetto heapprofd, `dumpsys meminfo` | M1, then every milestone |
| 60 fps scrolling | Fling through a 50 000-track library (a generated synthetic fixture, labelled as such) and the album grid: ≥ 99% of frames within 16.7 ms, no hitch over 50 ms | macOS: Instruments Animation Hitches; Windows: PresentMon/WPA; Linux: GTK frame-clock stats + Sysprof; Android: Macrobenchmark `FrameTimingMetric`, JankStats | M2, M4 |
| Android battery comparable to a standard native player | 1 h of local playback with the screen off, then 1 h with Now Playing visible, against the Media3 demo player on the same device and file; pass within +10% | Battery Historian, `dumpsys batterystats`, Perfetto power rails where the device has them | M4 |
| Glass frame cost | Live blur layers ≤ budget per screen; GPU frame time on Now Playing ≤ 4 ms at 60 Hz on reference hardware | Same GPU tools as above | M2, M4 |

Reference hardware is listed in [`step-0/README.md`](step-0/README.md);
budgets must pass on the entry-level device of each platform. When two goals
conflict, the order is: performance, animation and design effects first;
resource use and app size second (your decision, 2026-11-05).

## Step 0: architecture plan (now)

ADRs for every "verify" item, repo layout, risks, milestones, sources
report, token proposal, icon concepts. **Gate:** your approval and answers to
the open questions.

## Milestone 1: core engine, library and CLI on desktop

**Spikes first**

* S1: sample-exact gapless for M4A/AAC on Symphonia 0.6 (it parses `elst` but
  does not apply it, and ignores iTunSMPB) — ADR-0008.
* S2: codec gaps — Opus through libopus, HE-AAC through each desktop's platform
  decoder versus a pure-Rust decoder — ADR-0008.
* S3: output-device switching on CoreAudio, WASAPI and PipeWire/ALSA, with
  cpal's lack of hot-plug events — ADR-0009.

**Build**

* Workspace, CI, `cargo-deny`, `xtask tokens` (Rust target).
* `jf-engine`: local playback, gapless, crossfade, ReplayGain, 10-band
  parametric EQ, limiter, device switching, HTTP range streaming with
  read-ahead (tested against `jf-mock-server`).
* `jf-library`: scan, tags, artwork cache, incremental rescan, watcher,
  FTS5 search (word and trigram).
* `jf-lyrics`: LRC, enhanced LRC, TTML, WebVTT → `LyricDoc`, timeline.
* `jf-palette`: hue extraction, Misty theme resolution, backdrop bake.
* `jf-session`: queue, shuffle, repeat, history, persistence and restore.
* `jf-core` facade with snapshots, events and the playback anchor.
* `jf-cli`: `scan`, `play`, `queue`, `eq`, `devices`, `lyrics --follow`
  (prints the active line from the anchor), `theme <file>` (writes the
  backdrop PNG and theme JSON).

**Tests**

* Unit tests in every crate; property tests for the queue, EQ coefficients,
  colour conversion and backdrop contrast.
* Golden tests for every lyric format: fixture → `LyricDoc` JSON, including
  CJK, RTL, duets/agents, background vocals, offsets, malformed input.
* Engine tests with generated tones: gapless joins measured to the sample,
  crossfade gain curves, ReplayGain levels, EQ frequency response.

**Gate:** CLI plays a gapless album without a click on all three desktops;
idle CPU and memory baseline measured and reported; golden tests green.

**Progress:** v0.0.2 (2026-11-06) fixed the queue-end cut and output
start-up timing, made stream errors non-fatal, and added the drawing-time
helpers (`DisplayClock`, `next_change`, contract vectors) and a lyric-sync
check. v0.0.1 (2026-11-06) delivered the workspace, CI, `cargo-deny`,
`xtask tokens`, and the engine (without crossfade, device switching and HTTP
streaming), library (without the watcher), lyrics, palette, session, core
facade and CLI, tested on Linux. Still to come for the gate: spikes S1–S3,
crossfade, device switching, HTTP streaming with `jf-mock-server`, the
watcher, runs on macOS and Windows, and the budget measurements.

## Milestone 2: macOS UI end to end

**Spikes first:** S4 FFI surface with UniFFI (Swift) and the contract-test
harness; S5 glass cost on macOS 14, 15 and 26 (materials, masked edge blur,
`glassEffect` on 26) — ADR-0007.

**Build:** Swift package around the XCFramework; app shell with sidebar,
menu bar, shortcuts; Now Playing with backdrop crossfade and lyrics
(word-level sweep, translation and romanization tracks); Library (albums,
artists, tracks, playlists); local Search; Queue; Playback settings (EQ,
ReplayGain, crossfade, output device); Appearance (hue source, appearance,
reduce effects, the **Style** switch between Artwork and Misty backdrops for your comparison, and the **App icon** picker, ADR-0017); persistent mini-player; media keys, Now Playing info
centre, remote commands; VoiceOver labels and keyboard navigation.

**Tests:** Swift FFI contract tests; snapshot tests of key views in light,
dark, reduce transparency, increased contrast, CJK and RTL; lyrics-cursor
test vectors.

**Gate:** cold start, idle CPU, 60 fps scrolling and 8-hour soak measured on
macOS against budgets.

## Milestone 3: plugin host, SDK and Subsonic plugin

**Spike first:** S6 plugin runtime benchmark on desktop and on two
Android devices: wasmtime with Winch, Cranelift and Pulley (compile time,
cached load, instantiate, call latency, peak memory, binary size), with
wasmi on equivalent core modules as a reference — ADR-0003 picks the
compiler on these numbers.

**Build:** WIT package `jellyfesh:plugin@1`; `jf-plugin-host` with http
(allowlist, cookie jar, rate limits, timeouts), kv, crypto, clock, log;
fuel/epoch and memory limits; package verification and signatures; auth flow
state machines (QR, OAuth PKCE, cookie import, password exchange); settings
schema profile; unified search and dedupe; `jf-plugin-sdk`; `jf-plugin` CLI
(`new`, `build`, `test`, `pack`, `sign`, `verify`); `jf-mock-server`;
Subsonic/OpenSubsonic plugin (Stream mode); Sources settings page on macOS
with badges and the unofficial-plugin disclosure. Jellyfin follows right
after Subsonic, here if time allows, else early in M4.

**Tests:** sandbox and limit tests with hostile fixture plugins (infinite
loop, memory bomb, oversized responses, allowlist escape by redirect and
DNS rebinding, private-address access, secret exfiltration attempts, panics,
malformed components, API version mismatch); Subsonic plugin against
recorded fixtures and a Navidrome container.

**Gate:** limits hold under the hostile suite; plugin search adds < 50 ms
host overhead per source over network time.

## Milestone 4: Linux, Windows and Android UIs

**Spikes first:** S7 GTK progressive edge blur on `backdrop-filter`
render nodes (ADR-0006); S8 C# bindings against the fallback (ADR-0002); S9
Cloudy vs Haze behind `GlassSurface` on low- and mid-range devices
(ADR-0005); S10 AAudio power-saving path and buffer sizes (ADR-0009); S11
Android keystore-backed secret store (ADR-0015).

**Build:** each shell at parity with macOS. Linux: libadwaita adaptive
layout, system accent and dark style, MPRIS, Wayland and X11, Flatpak.
Windows: Mica/Acrylic, SMTC, taskbar buttons, MSIX. Android: Compose screens,
`GlassSurface`, SAF library access, Media3 session and notification,
foreground service, audio focus, headset and Bluetooth controls, lock screen.

**Tests:** C# and Kotlin FFI contract tests; UI smoke tests per shell;
accessibility checks (Narrator, Orca, TalkBack).

**Gate:** every budget measured on every platform; Android battery
comparison reported.

## Milestone 5: vendor and remote sources (deferred to the commercial phase)

Deferred on 2026-11-05: Apple Music and Spotify need developer accounts,
which wait for the commercial version (ADR-0016). The plan below stays for
then.


**Spikes first:** S12 current MusicKit for Android SDK, licence, sign-in
and playback; S13 MusicKit JS full-track playback in WebView2
(`requestMediaKeySystemAccess`, then a full track) — both ADR-0010.

Scope depends on ADR-0010 and ADR-0011 findings, which you approve first.
Apple Music: native MusicKit on macOS; Android, Windows and Linux only where
the findings show full-track playback is actually possible. Spotify:
Remote mode over the Web API (Connect devices), inside the official API
terms. UI marks vendor tracks as having no EQ, crossfade or visualization.

**Gate:** each adapter passes the vendor's own terms checklist in
`sources-report.md`.

## Milestone 6: community plugin template and opt-in unofficial examples

Template, docs, CI recipe and signing guide in the separate
`community-plugins` repository; reference plugins limited to what
ADR-0012 allows (no binaries or secrets extracted from official apps);
one-time disclosure flow; official release builds proven not to bundle them.

## Milestone 7: polish, performance, accessibility, packaging, updates

Final budget runs and 8-hour soaks on all platforms; accessibility audits
with VoiceOver, Narrator, Orca and TalkBack; CJK and RTL review;
pseudo-localisation in English, Simplified Chinese, Traditional Chinese and
Japanese; contrast audit over baked backdrops; the website with its home-page hero and download page; packaging per ADR-0016 (DMG,
installer and zip, Flatpak bundle, APKs on GitHub Releases) and the update
feed; licence and NOTICE audit (`cargo-deny`, Gradle and NuGet licence
reports). Signing, notarisation and store packaging come with the commercial
version.
