# Risk register

Status: **Step 0**, 2026-11-05. L = likelihood, I = impact (H/M/L).
Each risk names its mitigation and where it is retired (an ADR, a spike, or
a milestone gate). Spikes are defined in [`milestones.md`](milestones.md).

## Top ten

| # | Risk | L | I | Mitigation | Retired by |
| --- | --- | --- | --- | --- | --- |
| R1 | **Scope**: a Rust core, four native shells, an SDK and plugins is a large build; shells drift apart in behaviour and polish | H | H | Shells hold no logic; one token source; shared contract and lyrics-cursor test vectors; one milestone gate at a time; macOS first as the reference shell | Every gate |
| R2 | **Codec gaps**: Symphonia 0.6.1 has no Opus, HE-AAC, WavPack or APE decoder, and its AAC decoder is LC/stereo only | H | M | libopus adapter; HE-AAC through each OS's decoder behind `PlatformDecoder`; `rusty_aac` evaluated for Linux | ADR-0008, S2 |
| R3 | **Gapless M4A**: Symphonia ignores `elst` and `iTunSMPB`, so AAC albums click at track joins | H | M | Our own `GaplessTrim` stage, sample-exact tests | ADR-0008, S1 |
| R4 | **Store policy**: App Review 2.5.2 and 4.7 (plug-ins allowed only with an index, reporting, age gating and no native-API exposure), 5.2.2/5.2.3 (streaming third-party content needs the service's permission); Google Play's executable-code policy could not be read | M | H | Dormant: no store builds until the commercial phase (ADR-0016); then store builds load only release-signed plugins | ADR-0012, commercial phase |
| R5 | **Spotify viability**: about five users in development mode, Extended Quota only for 250 k-MAU organisations, no mixing with other services | H | M | Deferred to the commercial phase; then a Remote-only plugin with enforced `content_policy` | ADR-0011 |
| R6 | **Apple Music beyond macOS**: Android SDK docs are a 404; WebView2 DRM undocumented; WebKitGTK lacks EME; the ADPLA defines apps as Apple-platform software; MusicKit JS needs a token service | H | M | Deferred to the commercial phase; then macOS first, spikes and a legal read; no Linux support | ADR-0010 |
| R7 | **Android battery**: cpal's AAudio stream may run on the low-latency path, missing the "comparable to a native player" budget | M | H | S10: power-saving performance mode and large buffers, upstream patch or a thin AAudio output | ADR-0009, M4 gate |
| R8 | **C# bindings lag and AOT**: uniffi-bindgen-cs targets UniFFI 0.31 (one minor behind), is mostly one outside contributor's work, and its NativeAOT behaviour is undocumented | M | H | Pin UniFFI 0.31 for all bindings; C# contract tests from M2; S8 NativeAOT proof; JSON-over-C-ABI fallback defined | ADR-0002, S8 |
| R9 | **Plugin runtime on Android**: wasmi cannot host WIT components; wasmtime's Android targets are Tier 3 (untested upstream), it adds ~2–7 MB, and it had ~60 advisories in 2026 | M | H | wasmtime LTS with minimal features (no WASI, no async); compile-once cache; S6 on two devices; plugin-host suite on an Android emulator in CI | ADR-0003, S6 |
| R10 | **Unofficial plugins legal exposure**: Bilibili's January 2026 lawyer's letter; service terms; takedowns | M | H | Separate repo, opt-in, no app-extracted secrets or vendor code, reduced reference set, legal review | ADR-0012, M6 |

## Technical

| # | Risk | L | I | Mitigation | Retired by |
| --- | --- | --- | --- | --- | --- |
| R11 | Symphonia opens MP4 slowly over HTTP (issue #175) and fails on moov-at-end without seeking | M | M | Our seekable range reader with large read-ahead and coalesced reads | ADR-0008, M1 |
| R12 | cpal has no hot-plug events, WASAPI shared mode only, open Windows bug #1402, macOS 14.2 minimum | H | M | Per-OS `DeviceWatcher`; engine thread with its own COM apartment; track PR #1413 | ADR-0009, S3 |
| R13 | **Short CJK searches**: the FTS5 trigram index ignores 1–2 character queries (many Chinese titles are two characters) | H | M | Add a CJK bigram/unigram side index or a bounded scan path; measure on the 50 k fixture | M1 |
| R14 | GTK `backdrop-filter` is new (4.22) and distro GTK is older on Ubuntu 24.04 and Debian 13 | M | M | Flatpak on GNOME 51; scrim or Cloud fallback; S7 measures cost | ADR-0006 |
| R15 | Android blur: scrim below API 31, progressive only from 33; recent Cloudy crash fixes | M | M | `GlassSurface` tiers; pin 0.7.1; Haze 2.x as swap target | ADR-0005, S9 |
| R16 | Android 17 silently drops background audio unless a foreground service started while visible | M | H | Media3 `MediaSessionService`; test matrix includes Android 17 | M4 |
| R17 | Dependency churn: rubato (four majors in 2026), wasmtime monthly majors, Windows App SDK regressions, UniFFI breaking releases | H | L | Pins, thin wrappers, a scheduled update day per month, cargo-deny duplicate check | ADR-0014 |
| R18 | Two zbus majors (mpris-server 0.11 needs zbus 6; keyring's Secret Service uses zbus 5) | M | L | Unify versions or pin mpris-server 0.10 | M4 |
| R19 | Android needs two native initialisations (ndk-context; rustls-platform-verifier with Gradle, Maven and ProGuard setup) | M | L | One Android bootstrap module with an instrumented test | M4 |
| R20 | Library scanning through SAF is slow for large collections | M | M | File descriptors passed to Rust; MediaStore index if you choose option C | M4 |
| R21 | Playback-anchor clock skew between core and a shell | L | M | Per-platform clock table and a 1 ms contract test | ADR-0001 |
| R22 | Plugin sandbox escape through host imports (SSRF, DNS rebinding, cookie exfiltration) | L | H | Allowlist after redirects, private-range block, rebinding check, hostile plugin suite | ADR-0004, M3 |
| R23 | HLS needed for SoundCloud and Jellyfin's universal endpoint grows engine scope | M | L | Prefer progressive streams; add HLS only when a plugin needs it | M3+ |
| R24 | Budgets (cold start under 1 s, ≈1% idle CPU, no memory growth) slip as features land | M | M | Measured at every gate on reference hardware; regressions block the gate | Milestones |

## Product, legal and brand

| # | Risk | L | I | Mitigation | Retired by |
| --- | --- | --- | --- | --- | --- |
| R25 | "Jellyfesh" is close to "Jellyfin", a media server we integrate, whose mark is a purple-to-cyan jellyfish-like arch | M | M | Name kept (2026-11-05); trademark search before the commercial version; icon avoids Jellyfin's purple-to-cyan arch | Commercial phase |
| R26 | Misty: deviations D1 (artwork backdrop) and D2 (liquid accent) break MUSTs, so no conformance claim; "Misty" cannot appear in our names | H | L | Both styles built and compared; factual reference only | ADR-0013, M2 |
| R27 | Lyrics rights: LRCLIB data licence unknown; AMLL data partly under upstream terms; songwriters' copyright in lyric text | M | M | Lyrics only through plugins with source attribution; legal review before shipping lyric plugins | ADR-0012 |
| R28 | Apple Music items cannot show third-party synced lyrics (ADPLA "no synchronisation"), which people may expect | H | L | Explain in the UI; follow the conservative reading until legal review | ADR-0010 |
| R29 | Subsonic auth: servers without API keys need a password-equivalent token to be stored | H | L | Decided: API key first, else a keystore-only token with a clear note | ADR-0015 |
| R30 | Verification gaps: several official sites were unreachable from this environment, so some facts are secondary | H | M | Re-verify before the milestone that depends on them (allow the hosts, or verify manually) | M3, M5, M6 |
| R31 | MusicKit in Developer ID (non-App Store) macOS builds: reports conflict | M | M | Test in S12/S13 prep on a Developer ID build | M5 |
| R32 | Windows 10 still in use (consumer ESU reportedly to 2027-10) but Mica is Windows 11 only | L | L | Solid fallback already designed | ADR-0007 |
| R33 | **Unsigned builds** (no developer accounts until the commercial phase): macOS blocks the first launch, Windows SmartScreen warns, some people will not get past it | H | M | Clear install steps on the website and in release notes; ad-hoc signing on macOS; signing arrives with the commercial phase | ADR-0016 |
| R34 | Losing the Android release key would strand every installed copy (Android refuses updates signed with a new key) | L | H | Generate once, store offline in two places, never in CI logs | ADR-0016 |
