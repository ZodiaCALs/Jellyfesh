# ADR-0008: Codec coverage and gapless playback

* Status: Proposed
* Date: 2026-11-05 (facts checked against crate sources and READMEs that day)

## Context

The engine decodes with Symphonia so that EQ, ReplayGain, gapless and
crossfade apply to every source in Stream mode. Symphonia's current state
matters more than its reputation.

**Verified (crates.io, release notes, the 0.6.1 sources):**

* Symphonia **0.6.1** (2026-08-13), MPL-2.0, MSRV 1.85. 0.6 shipped on
  2026-05-15.
* Decoders: MP3, AAC-LC, ALAC, FLAC, Vorbis, PCM. **No decoder** for
  HE-AAC v1/v2, Opus, WavPack or APE (APE tags only).
* The AAC decoder rejects non-LC profiles, explicitly signalled SBR, more
  than two channels and 960-sample frames ("aac too complex"). Implicit SBR
  plays as the band-limited LC core.
* Gapless moved in 0.6: `FormatOptions::enable_gapless` is gone;
  `AudioDecoderOptions::gapless` is **on by default**. Demuxers fill
  `Track.delay/padding` and `Packet.trim_start/trim_end`, but only the MP3
  and Vorbis decoders apply the trims.
* **M4A is not gapless:** the MP4 demuxer parses `elst` but never applies
  it, and nothing reads `iTunSMPB`. MKV `CodecDelay`/`DiscardPadding` are
  parsed but unused. Ogg Opus pre-skip and the final granule are available.
* MP4 with `moov` after `mdat` fails on a non-seekable source; on a seekable
  one the demuxer seeks to the end once (an extra HTTP range request).
  Opening MP4 over HTTP is slow per skipped atom (issue #175, open).

Options for the gaps (all verified as published crates, 2026):

* Opus: `symphonia-adapter-libopus` 0.3.0 over libopus 1.6.1
  (`opusic-sys`, BSD-3, CMake build, Android needs the NDK). Pure-Rust
  decoders (`opus-decoder`, `opus-rs`, `opus-pure`, …) all first appeared in
  2026 with self-reported conformance.
* HE-AAC: FDK-AAC crates bundle the Fraunhofer licence, which grants **no
  patent licence** and requires offering the source. `rusty_aac` 2.0.1
  (Apache-2.0, first release 2026-07) claims LC and HE-AAC v1/v2 from the
  ISO spec, without a Symphonia adapter. Platform decoders: Android
  guarantees HE-AAC v1/v2 on all versions (verified in Android's supported
  formats page); AudioToolbox, Media Foundation and GStreamer are expected
  to, but I could not open their documentation from this environment.

## Decision

1. **Symphonia 0.6.x** demuxes everything it supports and decodes MP3,
   AAC-LC, ALAC, FLAC, Vorbis and PCM.
2. **Opus through libopus** (`symphonia-adapter-libopus`) on every platform.
   One proven decoder beats young pure-Rust ones for now; revisit yearly.
3. **HE-AAC and multichannel AAC through a `PlatformDecoder` trait** in
   `jf-engine`, implemented in Rust per OS: AudioToolbox (macOS),
   Media Foundation (Windows), `AMediaCodec` through the NDK (Android), and
   GStreamer on Linux if the Flatpak runtime provides the codec. The OS
   vendor carries the codec licence. Spike S2 also measures `rusty_aac`
   against conformance vectors as the Linux fallback; shipping our own
   HE-AAC decoder needs a patent review first.
4. **We own gapless trimming.** A `GaplessTrim` stage after decode applies
   delay and padding from, in order: decoder-applied trims (MP3, Vorbis,
   Opus); MP4 `elst` and `iTunSMPB` parsed by us; Matroska `CodecDelay` and
   `DiscardPadding`. Tests check joins to the sample with generated fixtures.
5. **Streaming MP4** uses our seekable HTTP reader (range requests, large
   read-ahead, coalesced small reads), so moov-at-end files open with one
   extra request instead of failing or crawling.
6. **WavPack, APE and CUE sheets are not in v1** unless you want them (APE
   and single-file albums with CUE are common in CJK lossless collections).
   Options and costs are in the Step 0 index; awaiting your choice. Note `symphonia-codec-wavpack` on crates.io is not an
   official Symphonia crate and its licence metadata is inconsistent
   (GPL-3.0 in Cargo, MPL-2.0 in LICENSE); we would not use it.

## Consequences

* Builds need CMake and a C toolchain for libopus on all four targets.
* The `PlatformDecoder` path means a codec can play on one OS and not
  another; the UI reports "This format needs a system codec that isn't
  installed" instead of failing silently.
* Symphonia is MPL-2.0: file-level copyleft. Fine for any app licence, but
  changes we make to Symphonia files must be published.

## Verification sources

* https://crates.io/api/v1/crates/symphonia, release notes at
  https://github.com/pdeljanov/Symphonia/releases, the 0.6 migration guide
  (`docs/guides/migration/0p6.md`), and the published 0.6.1 crate sources.
* https://developer.android.com/media/platform/supported-formats
* Issues: https://github.com/pdeljanov/Symphonia/issues/175 and `/289`.
