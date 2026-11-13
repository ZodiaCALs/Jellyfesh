# Step 0 review package

Status: **approved; implementation started.** Revision 5 (tag `step-0.5`)
records your answers of 2026-11-05; on 2026-11-06 you started the v0.0.1
release ([notes](../releases/v0.0.1.md)). Still open, none of them blocking
Milestone 1: the home-page hero, the control style, the backdrop comparison,
codec scope and Android library access.

## What to review, in order

1. [Architecture overview](../architecture.md): how the core, shells and
   plugins fit together, the data flow and the playback anchor.
2. [Decision records](../adr/README.md): one per "verify" item, plus the
   foundations and ADR-0016 (distribution, licence, versions).
3. [Online sources report](../sources-report.md): official APIs, terms and
   the playback mode each service actually allows.
4. [Risk register](../risks.md).
5. [Milestone plan](../milestones.md): spikes first, budgets and how each is
   measured, one gate per milestone.
6. [Repository layout](../repo-layout.md).
7. [Design tokens](../design-tokens.md): defaults on top of Misty 1.1.
8. [Lyrics model](../lyrics-model.md): `LyricDoc` and format coverage.
9. The design canvas (link in the session): round 3 icons, four control
   styles, round 2 icons, the two Now Playing styles and round 1 concepts.
   Control styles are written up in [`design-controls.md`](../design-controls.md). Icon sources are in
   [`design/icons/`](../../design/icons/).

## Your decisions (2026-11-05)

| # | Question | Decision | Where it lands |
| --- | --- | --- | --- |
| 1 | Still mode | As proposed: audio is playing and nothing is drawn per frame, either because no window is visible (minimised, hidden, fully covered, phone screen off) or because the visible window has no time-driven animation (lyrics closed, no visualiser; progress text updates once a second). Idle-CPU budget ≈ 1% of one core in this state | [milestones](../milestones.md) |
| 2 | Distribution | Own website and GitHub Releases first; commercial version later, when you say | [ADR-0016](../adr/0016-distribution-licence-versions.md) |
| 3 | Licence | None for now (all rights reserved); dependencies limited to permissive, MPL-2.0 and dynamically linked LGPL | ADR-0016 |
| 4 | Minimum OS | macOS 14.2; Windows 10 1809 (Mica on 11); Android 8.0 / API 26 (glass from 12, progressive blur from 13); Linux via Flatpak on the GNOME 51 runtime | ADR-0005/0006/0007/0009 |
| 5 | Misty deviations | Build both styles and compare: **Artwork** (your brief: blurred-artwork backdrop, one liquid accent) and **Misty** (conformant field at the artwork hue). Mock-ups on the canvas now; a live switch in Appearance from Milestone 2 | [ADR-0013](../adr/0013-design-language.md) |
| 6 | Priorities and hardware | Trade-off order: performance, animation and design effects first; resource use and app size second. Reference hardware proposed below | [architecture](../architecture.md), milestones |
| 7 | Subsonic without API keys | Store the token (salt + MD5 token pair) in the OS keystore with a clear note; switch to API keys when the server supports them | [ADR-0015](../adr/0015-secrets-and-credentials.md) |
| 8 | Codec scope | Explained in the session; awaiting your choice (below) | [ADR-0008](../adr/0008-codecs-and-gapless.md) |
| 9 | Developer accounts | Deferred to the commercial version, so Apple Music, Spotify, notarisation and code signing move there too | ADR-0016, milestones |
| 10 | Name | Keep "Jellyfesh"; trademark search before the commercial version | [risks](../risks.md) R25 |
| 11 | Android library access | Options explained (below); awaiting your choice | — |
| 12 | Launch languages | English, Simplified Chinese, Traditional Chinese, Japanese | architecture |
| 13 | WIT components | Keep; wasmtime is the runtime | [ADR-0003](../adr/0003-plugin-runtime.md) |
| 14 | Repository | Every version is tagged and delivered as a `.zip` snapshot and a `.bundle` with full history | ADR-0016, [CHANGELOG](../../CHANGELOG.md) |
| — | App icon | Variant B; **Ice** default in light, **Steel** in dark; all ten colours selectable in Settings → Appearance → App icon | [ADR-0017](../adr/0017-app-icon-defaults-and-picker.md) |
| — | Wordmark | **Spaced capitals** for formal use; a fancier wordmark for the home page (three options) | [`design/brand/`](../../design/brand/) |

### Reference hardware (proposed, since budgets need fixed devices)

| Platform | Performance reference | Also checked (correctness only) |
| --- | --- | --- |
| macOS | MacBook Air M1, 8 GB (the slowest Apple Silicon Mac) | One Intel Mac on macOS 14.2+ for the x86_64 half of the universal build |
| Windows | Laptop with a 12th-gen Intel Core i5 U-series and Iris Xe graphics, 8 GB, Windows 11 | Windows 10 22H2 virtual machine |
| Linux | The same laptop on Fedora Workstation (GNOME), running the Flatpak | Ubuntu 26.04 |
| Android | Entry level: a 2024–25 phone with 4 GB RAM (Galaxy A16 class). Mid range: Pixel 8a | Android 8.0 emulator, Android 17 device |

Budgets must pass on the entry-level device of each row, so smoothness holds
where it is hardest to keep.

## Still open

1. **Home-page hero**: A bioluminescent glass, B tentacle flourish or C aurora
   glass capitals (my pick: A, or C to match the formal mark).
2. **Control style** for bars and buttons: A (after Apple), B (after
   Microsoft), C (after Google), D (a blend, my pick), or native flavours
   per platform. Details in [`design-controls.md`](../design-controls.md).
3. **Backdrop style**: compare the two Now Playing mock-ups; the live
   comparison follows in Milestone 2.
4. **Codec scope** (decide before the Milestone 1 spikes):

   | Option | What it adds | Cost |
   | --- | --- | --- |
   | A. As planned | MP3, AAC (LC via Symphonia, HE via the OS), ALAC, FLAC, Vorbis, Opus, WAV/AIFF | — |
   | B. + CUE sheets | Albums ripped as one big file plus a `.cue` index show up as separate tracks, gapless by nature; CUE text in GBK, Big5 or Shift-JIS is detected | Library and engine work, no new decoder |
   | C. + APE (Monkey's Audio) | A lossless format common in Chinese and Japanese CD rips, often paired with CUE | A C/C++ decoder to build for four platforms; licence to confirm |
   | D. + WavPack | A niche lossless/hybrid format | libwavpack (BSD-3), modest |
   | E. Bit-perfect output | Samples reach the DAC untouched: exclusive device access, the device switched to the file's sample rate, and no EQ, ReplayGain, crossfade or software volume while it is on | Own output backends per OS (cpal has none); large |

   My recommendation: B and C after Milestone 1 (they matter for CJK
   libraries), D later, E as a commercial-version "Hi-Fi mode".
5. **Android library access**:

   | Option | How it works | Good | Weak |
   | --- | --- | --- | --- |
   | A. SAF only (the brief) | The person picks folders in the system picker | No broad permission; any folder, SD card or USB drive | Slow first scan of large libraries; nothing until folders are picked |
   | B. MediaStore only | `READ_MEDIA_AUDIO` permission; the OS index lists every audio file | Instant first library | OS tags are basic (no ReplayGain, lyrics), may garble CJK tags, skips folders with `.nomedia` |
   | C. Both (recommended) | MediaStore for an instant first list, then our own tag reading in the background through file descriptors; SAF to add folders the OS doesn't index, or instead of the permission | Fast start and full metadata; still works if the person declines the permission | Two code paths to test |

## Verification gaps

Some official sites were blocked by this environment's network policy, so
a few facts are marked **U** (secondary). They are listed at the top of the
sources report, and each is re-checked before the milestone that depends on
it.
