//! Audio engine: decode (files and HTTP streams), resample, ReplayGain, EQ, limiter, gapless
//! joins, crossfades and output sinks.
//!
//! # Threads
//!
//! [`Engine`] is a handle. Its methods queue commands to one engine thread and return at
//! once; the engine thread owns the decoders, the processing chain and the output [`Sink`],
//! and reports back through [`EngineEvent`]s and the shared [`PlaybackAnchor`]. With a
//! device output, a cpal real-time callback pulls from a lock-free SPSC ring buffer and
//! applies the volume (and the short fade-out of a manual cut); it never locks, allocates or
//! logs (architecture §4).
//!
//! Local files decode on the engine thread. An HTTP item ([`MediaSource::Http`]) downloads on
//! its own thread with range requests (read-ahead of about 10 s of audio, spilled to a capped
//! temporary file under [`EngineConfig::cache_dir`], reconnects with backoff, URLs resolved
//! again through [`EngineConfig::resolver`] before they expire or after 401/403/410), and
//! opens and decodes on another thread into a small queue, so the engine thread never waits on
//! the network. At most two decoders are live: the current one and either the prepared next
//! one or, during a crossfade, the outgoing one.
//!
//! # Pipeline
//!
//! ```text
//! Symphonia probe + decode (gapless on; Opus through libopus)
//!   → gapless trim (decoder-applied for MP3/Vorbis/Opus end padding, packet trims otherwise;
//!     plus MP4 iTunSMPB/elst priming and padding and the Opus pre-skip, read here)
//!   → interleaved f32, folded to stereo (mono copied to both sides); Opus output gain
//!   → ReplayGain scale (per track; a scalar, so it commutes with resampling)
//!   → lane: resample to the output rate with rubato (only when the rates differ)
//!   → mixer: two lanes during a crossfade, equal-power (sin/cos) fades
//!   → parametric EQ (up to 10 RBJ biquads)
//!   → limiter (-1 dBFS look-ahead; engaged when gain could pass 0 dBFS, and during overlaps)
//!   → volume → sink (device, WAV file or null)
//! ```
//!
//! # Gapless playback
//!
//! When a track's decoder runs out, the next item's decoder feeds the same lane without
//! flushing anything: a track at the same sample rate continues the same resampler run, and
//! the EQ and limiter keep their state. Joins are therefore sample-contiguous whenever the
//! codec delivers exact track boundaries: FLAC, WAV/AIFF/PCM, ALAC, Vorbis, Opus, MP3 with a
//! LAME/Xing header, and AAC in MP4 whose `iTunSMPB` tag or edit list states its priming and
//! padding (ADR-0008, spike S1). [`TrackInfo::gapless`] reports this per track. HTTP items
//! join the same way; the next one is opened about 10 s ahead so its join does not wait.
//!
//! [`Engine::set_upcoming`] can change what follows the current track at any time. If audio
//! of the old next track has already been written to the output buffer, the engine discards
//! it and re-renders from the first sample not yet played, so the join stays gapless.
//!
//! # Crossfades
//!
//! With [`Engine::set_crossfade`] automatic transitions overlap: the outgoing track fades out
//! with `cos`, the incoming one fades in with `sin`, so the summed power stays constant. The
//! overlap is the configured length but never more than half of either track. With album
//! continuity, consecutive items with the same [`PlayItem::album`] key join gaplessly instead.
//! Manual transitions (next, previous, `set_queue`, seek, stop, output changes) never
//! crossfade; they fade out over 10 ms and the new audio fades in, so they do not click.
//!
//! # Outputs
//!
//! [`Engine::set_output`] moves playback to another output at its own sample rate and
//! continues from the audible position. `OutputKind::Device { name: None }` follows the system
//! default: [`Engine::devices_changed`] (called by shells on OS device notifications; the
//! engine never polls) moves to a new default, and a default output that fails is reopened a
//! few times before the failure is reported.
//!
//! # Position
//!
//! The audible position is the number of frames delivered to the sink minus the sink's
//! reported latency (buffered audio plus the device's own latency). Anchors are published on
//! play, pause, seek, stop, track change and buffering, and at most once a second otherwise,
//! when the audio clock drifts more than 15 ms from the last anchor. `rate` is 0.0 when not
//! playing and while a stream buffers ([`EngineEvent::Buffering`]). Track events fire when a
//! boundary is heard, not when it is decoded; during a crossfade the incoming track starts
//! (and the anchor follows it) before the outgoing one ends.

mod anchor;
mod decode;
pub mod dsp;
mod engine;
mod http;
mod mono;
mod mp4;
mod pipeline;
mod resample;
mod sink;
mod types;
mod worker;

pub use anchor::{PlaybackAnchor, monotonic_now_ns};
pub use dsp::{BandKind, EqBand, EqSettings};
pub use engine::Engine;
pub use mono::{AudioInput, MonoAudio, SPEECH_RATE, decode_mono};
pub use sink::{DeviceInfo, NullSink, Sink, WavSink};
#[cfg(feature = "device")]
pub use sink::{DeviceSink, list_devices};
pub use types::{
    CrossfadeSettings, EngineConfig, EngineError, EngineEvent, HttpSource, MediaSource, OutputKind, PlayItem,
    ReplayGainInfo, ReplayGainMode, StreamResolver, TrackInfo,
};
