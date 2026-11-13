//! Demux and decode with Symphonia into interleaved stereo `f32`.
//!
//! # Gapless trimming
//!
//! Every source has a playable range of "track frames" starting at 0. Decoders that trim encoder
//! delay and padding themselves (MP3, Vorbis, and Opus for its end padding) return buffers that
//! are already trimmed; for the others the packet trims a demuxer sets are applied here. On top
//! of that the source drops what the container states but Symphonia does not apply:
//!
//! * AAC in MP4: the priming and padding from `iTunSMPB` or the edit list ([`crate::mp4`]).
//! * Opus: the `OpusHead` pre-skip (the libopus adapter would apply it only to the first packet
//!   it ever decodes, which is wrong after a seek). The `OpusHead` output gain is applied too.

use std::fmt;
use std::fs::File;
use std::path::PathBuf;
use std::sync::OnceLock;

use symphonia::core::audio::{Channels, Position};
use symphonia::core::codecs::audio::well_known::{
    CODEC_ID_AAC, CODEC_ID_ALAC, CODEC_ID_FLAC, CODEC_ID_MP1, CODEC_ID_MP2, CODEC_ID_MP3, CODEC_ID_OPUS,
    CODEC_ID_VORBIS,
};
use symphonia::core::codecs::audio::{AudioCodecId, AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::errors::{Error as SymError, SeekErrorKind};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{TimeBase, Timestamp};
use symphonia_adapter_libopus::OpusDecoder;

use crate::http::HttpStream;
use crate::mp4;
use crate::types::{EngineError, TrackInfo};

/// Consecutive undecodable packets after which a track is abandoned.
const MAX_BAD_PACKETS: u32 = 32;
/// Frames decoded and dropped before a seek target so the decoder state settles. RFC 7845
/// (section 4.6) asks for at least 80 ms of Opus; with 80 ms a steady tone still came out 5 %
/// low right after the target (the band energies are predicted from frame to frame), with
/// 200 ms it is exact. AAC needs two frames for its overlapping transforms.
const OPUS_PREROLL: i64 = 9_600;
const AAC_PREROLL: i64 = 2_048;

/// Where a [`Source`] reads from.
#[derive(Debug, Clone)]
pub(crate) enum Input {
    File(PathBuf),
    Http(HttpStream),
}

impl Input {
    fn reader(&self, primary: bool) -> std::io::Result<Box<dyn MediaSource>> {
        Ok(match self {
            Input::File(path) => Box::new(File::open(path)?),
            Input::Http(stream) => Box::new(stream.reader(primary)),
        })
    }

    fn byte_len(&self) -> Option<u64> {
        match self {
            Input::File(path) => std::fs::metadata(path).ok().map(|m| m.len()),
            Input::Http(stream) => stream.total(),
        }
    }

    fn hint(&self) -> Hint {
        let mut hint = Hint::new();
        match self {
            Input::File(path) => {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    hint.with_extension(ext);
                }
            }
            Input::Http(stream) => {
                if let Some(ext) = stream.extension() {
                    hint.with_extension(&ext);
                }
                if let Some(mime) = stream.mime() {
                    hint.mime_type(&mime);
                }
            }
        }
        hint
    }

    fn open_error(&self, message: String) -> EngineError {
        match self {
            Input::File(path) => EngineError::Open {
                path: path.clone(),
                message,
            },
            Input::Http(stream) => EngineError::Stream {
                url: stream.describe(),
                message,
            },
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Input::File(path) => path.display().to_string(),
            Input::Http(stream) => stream.describe(),
        }
    }

    /// Why a stream failed, if it did.
    fn failure(&self) -> Option<String> {
        match self {
            Input::File(_) => None,
            Input::Http(stream) => stream.error(),
        }
    }
}

/// Symphonia's decoders plus Opus through libopus (ADR-0008).
fn codecs() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(|| {
        let mut registry = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        registry.register_audio_decoder::<OpusDecoder>();
        registry
    })
}

/// An open audio source: demuxer, decoder and the per-track state the engine needs.
pub(crate) struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    rate: u32,
    info: TrackInfo,
    /// The decoder trims encoder delay and padding itself (MP3, Vorbis, Opus); for the others
    /// the packet trims, when a demuxer sets any, are applied here.
    decoder_trims: bool,
    /// Frames of the decoded timeline before track frame 0 (MP4 priming, Opus pre-skip).
    delay: i64,
    /// Track frames after the last playable one are dropped (MP4 padding).
    valid_frames: Option<i64>,
    /// Frames decoded before a seek target and dropped.
    preroll: i64,
    /// Linear gain applied to every sample (the Opus header's output gain).
    gain: f32,
    /// Playable frames of the whole track, when the container states them.
    total_frames: Option<i64>,
    /// After a seek: drop decoded frames before this track frame.
    discard_until: Option<i64>,
    /// Track frame of the next frame `read` returns.
    next_frame: i64,
    /// Set by `open` and `seek`; the first frame `read` actually returned afterwards.
    first_frame: Option<i64>,
    report_first_frame: bool,
    mixer: StereoMixer,
    scratch: Vec<f32>,
    bad_packets: u32,
    eof: bool,
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source")
            .field("info", &self.info)
            .field("next_frame", &self.next_frame)
            .finish()
    }
}

/// The parts of an `OpusHead` the engine applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpusHead {
    pre_skip: u16,
    /// Q7.8 dB.
    output_gain: i16,
}

/// Reads `OpusHead` (RFC 7845, section 5.1) from a codec's extra data.
fn parse_opus_head(data: &[u8]) -> Option<OpusHead> {
    if data.len() < 19 || &data[..8] != b"OpusHead" {
        return None;
    }
    Some(OpusHead {
        pre_skip: u16::from_le_bytes([data[10], data[11]]),
        output_gain: i16::from_le_bytes([data[16], data[17]]),
    })
}

impl Source {
    /// Opens and probes `input`, selects its default audio track and creates a gapless decoder.
    pub(crate) fn open(input: &Input) -> Result<Self, EngineError> {
        let open_err = |message: String| input.open_error(message);
        let media = input.reader(true).map_err(|e| open_err(e.to_string()))?;
        let stream = MediaSourceStream::new(media, MediaSourceStreamOptions::default());
        let format = symphonia::default::get_probe()
            .probe(
                &input.hint(),
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| match input.failure() {
                // The probe saw an I/O error from a failed stream (404, refused URL, …).
                Some(failure) => open_err(failure),
                None => open_err(format!("unrecognised format ({e})")),
            })?;
        let is_mp4 = format.format_info().short_name == "isomp4";
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| open_err("no audio track".into()))?;
        let mut params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| open_err("no audio codec parameters".into()))?
            .clone();
        let rate = params
            .sample_rate
            .filter(|&r| r > 0)
            .ok_or_else(|| open_err("unknown sample rate".into()))?;
        let codec = params.codec;

        // Opus: the pre-skip is applied here, so the adapter must not apply it again.
        let opus = if codec == CODEC_ID_OPUS {
            let head = params.extra_data.as_deref().and_then(parse_opus_head);
            if let Some(extra) = params.extra_data.as_mut()
                && head.is_some()
            {
                extra[10] = 0;
                extra[11] = 0;
            }
            head
        } else {
            None
        };
        let decoder = codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default().gapless(true))
            .map_err(|e| open_err(format!("unsupported codec ({e})")))?;

        // MP4: priming and padding from iTunSMPB or the edit list, read from the file itself.
        let mp4_info = if is_mp4 {
            match input.reader(false).and_then(|mut r| mp4::read_info(&mut r, rate)) {
                Ok(info) => info,
                Err(err) => {
                    tracing::debug!("{}: cannot read MP4 gapless information: {err}", input.describe());
                    mp4::Mp4Info::default()
                }
            }
        } else {
            mp4::Mp4Info::default()
        };
        let mp4_trim = mp4_info
            .trim
            .filter(|_| codec == CODEC_ID_AAC || codec == CODEC_ID_ALAC);

        let channels = params.channels.as_ref().map_or(0, Channels::count);
        let (delay, valid_frames) = match (mp4_trim, opus) {
            (Some(trim), _) => (trim.delay as i64, Some(trim.valid as i64)),
            (None, Some(head)) => (i64::from(head.pre_skip), None),
            (None, None) => (0, None),
        };
        let container_frames = match track.num_frames {
            Some(frames) => Some(frames as i64),
            None => track
                .time_base
                .zip(track.duration)
                .and_then(|(tb, d)| tb.calc_duration(d))
                .map(|t| (t.as_secs_f64() * f64::from(rate)).round() as i64),
        };
        let playable_frames = match (valid_frames, opus) {
            (Some(valid), _) => Some(valid),
            // Ogg reports the final granule position, which counts the pre-skip.
            (None, Some(_)) => container_frames.map(|frames| (frames - delay).max(0)),
            (None, None) => container_frames,
        };
        let duration_us = playable_frames.map(|frames| frames_to_us(frames.max(0) as u64, rate));
        let has_trim_info = track.delay.is_some() || track.padding.is_some();
        let codec_name = decoder.codec_info().short_name.to_string();
        let bitrate = bitrate(
            &codec_name,
            rate,
            channels,
            params.bits_per_coded_sample.or(params.bits_per_sample),
            mp4_info.mdat_bytes.or_else(|| input.byte_len()),
            duration_us,
        );
        let info = TrackInfo {
            gapless: is_gapless(codec, &codec_name, has_trim_info, mp4_trim.is_some()),
            codec: codec_name,
            sample_rate: rate,
            channels: u16::try_from(channels).unwrap_or(u16::MAX),
            duration_us,
            bitrate,
        };
        let decoder_trims = [
            CODEC_ID_MP1,
            CODEC_ID_MP2,
            CODEC_ID_MP3,
            CODEC_ID_VORBIS,
            CODEC_ID_OPUS,
        ]
        .contains(&codec);
        let preroll = if codec == CODEC_ID_OPUS {
            OPUS_PREROLL
        } else if codec == CODEC_ID_AAC {
            AAC_PREROLL
        } else {
            0
        };
        let gain = opus.map_or(1.0, |head| {
            10f32.powf(f32::from(head.output_gain) / (256.0 * 20.0))
        });
        if let Some(trim) = mp4_trim {
            tracing::debug!(
                "{}: MP4 gapless from {:?}: {} frames of priming, {} playable",
                input.describe(),
                trim.source,
                trim.delay,
                trim.valid
            );
        }
        tracing::debug!("opened {}: {info:?}", input.describe());
        Ok(Self {
            track_id: track.id,
            time_base: track.time_base,
            format,
            decoder,
            rate,
            info,
            decoder_trims,
            delay,
            valid_frames,
            preroll,
            gain,
            total_frames: playable_frames,
            discard_until: None,
            next_frame: 0,
            first_frame: None,
            report_first_frame: true,
            mixer: StereoMixer::default(),
            scratch: Vec::new(),
            bad_packets: 0,
            eof: false,
        })
    }

    pub(crate) fn info(&self) -> &TrackInfo {
        &self.info
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Playable frames of the whole track, when the container states them.
    pub(crate) fn total_frames(&self) -> Option<i64> {
        self.total_frames
    }

    /// Track frame of the next frame `read` returns.
    pub(crate) fn next_frame(&self) -> i64 {
        self.next_frame
    }

    /// Seeks so that the next frame `read` returns is track frame `frame` (sample accurate:
    /// the demuxer lands on a packet at or before it, and the frames in between, plus a
    /// pre-roll for codecs that need one, are decoded and dropped). A position past the end
    /// leaves the source at its end.
    pub(crate) fn seek_frame(&mut self, frame: i64) -> Result<(), EngineError> {
        let frame = frame.max(0);
        let from = (frame + self.delay - self.preroll).max(0);
        let to = SeekTo::Timestamp {
            ts: self.frames_to_ts(from),
            track_id: self.track_id,
        };
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(seeked) => {
                self.decoder.reset();
                // A coarse time base rounds the timestamp up; start where it really points.
                let rounding = (self.ts_to_frames(seeked.required_ts) - from).max(0);
                let target = frame + rounding;
                self.discard_until = Some(target);
                self.next_frame = target;
                self.first_frame = None;
                self.report_first_frame = true;
                self.eof = false;
                Ok(())
            }
            Err(SymError::SeekError(SeekErrorKind::OutOfRange)) => {
                self.eof = true;
                Ok(())
            }
            Err(err) => Err(EngineError::Seek(err.to_string())),
        }
    }

    /// The frame at `position_us`, rounded down.
    pub(crate) fn frame_at(&self, position_us: i64) -> i64 {
        let frames = i128::from(position_us) * i128::from(self.rate) / 1_000_000;
        i64::try_from(frames).unwrap_or(i64::MAX)
    }

    /// The track frame of the first frame returned since `open` or the last `seek`, once.
    pub(crate) fn take_first_frame(&mut self) -> Option<i64> {
        if self.report_first_frame && self.first_frame.is_some() {
            self.report_first_frame = false;
            return self.first_frame;
        }
        None
    }

    /// Decodes until at least one frame is available and appends it to `out` as interleaved
    /// stereo. Returns `Ok(false)` at the end of the track.
    pub(crate) fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, EngineError> {
        loop {
            if self.eof {
                return Ok(false);
            }
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    self.eof = true;
                    continue;
                }
                Err(SymError::ResetRequired) => {
                    tracing::warn!("track layout changed mid-stream; ending the track");
                    self.eof = true;
                    continue;
                }
                Err(SymError::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    continue;
                }
                Err(err) => {
                    self.eof = true;
                    return Err(EngineError::Decode(err.to_string()));
                }
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let trim_start = packet.trim_start.get();
            let trim_end = packet.trim_end.get();
            let pts_frame = self.ts_to_frames(packet.pts);
            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                Err(SymError::DecodeError(msg)) => {
                    self.bad_packet(msg)?;
                    continue;
                }
                Err(SymError::IoError(err)) => {
                    self.bad_packet(&err.to_string())?;
                    continue;
                }
                Err(err) => {
                    self.eof = true;
                    return Err(EngineError::Decode(err.to_string()));
                }
            };
            self.bad_packets = 0;
            if buffer.spec().rate() != self.rate {
                self.eof = true;
                return Err(EngineError::Decode("sample rate changed mid-stream".into()));
            }
            let channels = buffer.spec().channels().clone();
            buffer.copy_to_vec_interleaved(&mut self.scratch);
            let width = channels.count().max(1);
            let decoded = self.scratch.len() / width;

            // Track frame of the buffer's first frame: decoders that trim return the buffer
            // already trimmed, the others return the encoder delay too.
            let base = if self.decoder_trims {
                pts_frame.saturating_add(trim_start as i64)
            } else {
                pts_frame
            } - self.delay;
            let (mut start, mut end) = (0, decoded);
            if !self.decoder_trims {
                // Gapless trim for decoders that leave it to the caller.
                start = (trim_start as usize).min(decoded);
                end = decoded - (trim_end as usize).min(decoded - start);
            }
            // Priming before track frame 0, padding after the last playable frame.
            if base < 0 {
                start = start.max(usize::try_from(-base).unwrap_or(usize::MAX).min(end));
            }
            if let Some(valid) = self.valid_frames {
                let room = usize::try_from(valid.saturating_sub(base).max(0)).unwrap_or(usize::MAX);
                end = end.min(room).max(start);
            }
            if let Some(target) = self.discard_until {
                // Sample-accurate seek: drop the frames before the target.
                let skip = usize::try_from(target.saturating_sub(base + start as i64)).unwrap_or(0);
                if skip >= end - start {
                    if self.valid_frames.is_some_and(|valid| base + end as i64 >= valid) {
                        self.eof = true;
                    }
                    continue;
                }
                start += skip;
                self.discard_until = None;
            }
            if start >= end {
                if self
                    .valid_frames
                    .is_some_and(|valid| base + start as i64 >= valid)
                {
                    // Only padding is left.
                    self.eof = true;
                }
                continue;
            }
            let frame = base + start as i64;
            if self.first_frame.is_none() {
                self.first_frame = Some(frame);
            }
            self.mixer.configure(&channels, width);
            let at = out.len();
            self.mixer.mix(&self.scratch[start * width..end * width], out);
            if self.gain != 1.0 {
                out[at..].iter_mut().for_each(|s| *s *= self.gain);
            }
            self.next_frame = frame + (end - start) as i64;
            return Ok(true);
        }
    }

    fn bad_packet(&mut self, message: &str) -> Result<(), EngineError> {
        self.bad_packets += 1;
        tracing::debug!("skipping undecodable packet: {message}");
        if self.bad_packets >= MAX_BAD_PACKETS {
            self.eof = true;
            return Err(EngineError::Decode(format!(
                "too many undecodable packets ({message})"
            )));
        }
        Ok(())
    }

    /// Converts frames at the track's sample rate to a timestamp in the track's time base,
    /// rounding up so that converting back gives at least `frames`.
    fn frames_to_ts(&self, frames: i64) -> Timestamp {
        match self.time_base {
            Some(tb) => {
                let numer = i128::from(tb.numer.get()) * i128::from(self.rate);
                let denom = i128::from(tb.denom.get());
                let ts = (i128::from(frames) * denom + numer - 1).div_euclid(numer);
                Timestamp::new(i64::try_from(ts).unwrap_or(i64::MAX))
            }
            None => Timestamp::new(frames),
        }
    }

    /// Converts a timestamp in the track's time base to frames at the track's sample rate.
    fn ts_to_frames(&self, ts: Timestamp) -> i64 {
        match self.time_base {
            Some(tb) => {
                let numer = i128::from(tb.numer.get()) * i128::from(self.rate);
                let denom = i128::from(tb.denom.get());
                let frames = (i128::from(ts.get()) * numer).div_euclid(denom);
                i64::try_from(frames).unwrap_or(if frames < 0 { i64::MIN } else { i64::MAX })
            }
            None => ts.get(),
        }
    }
}

fn frames_to_us(frames: u64, rate: u32) -> i64 {
    i64::try_from(u128::from(frames) * 1_000_000 / u128::from(rate.max(1))).unwrap_or(i64::MAX)
}

/// Bits per second: exact for PCM, else the average over `bytes` (the file, or the MP4 media
/// data) and the playable duration.
fn bitrate(
    codec: &str,
    rate: u32,
    channels: usize,
    bits_per_sample: Option<u32>,
    bytes: Option<u64>,
    duration_us: Option<i64>,
) -> Option<u32> {
    if codec.starts_with("pcm")
        && let Some(bits) = bits_per_sample
    {
        return u32::try_from(u64::from(rate) * channels as u64 * u64::from(bits)).ok();
    }
    let duration_us = u64::try_from(duration_us?).ok().filter(|&d| d > 0)?;
    let bits = u128::from(bytes?) * 8 * 1_000_000 / u128::from(duration_us);
    u32::try_from(bits).ok().filter(|&b| b > 0)
}

/// Joins are sample exact when the codec has no encoder delay (FLAC, PCM, ALAC), the decoder
/// applies the trims the demuxer reports (Vorbis and Opus always; MP3 when a LAME/Xing header
/// gives the delay and padding), or the MP4 container states them (AAC with `iTunSMPB` or an
/// edit list).
fn is_gapless(codec: AudioCodecId, short_name: &str, has_trim_info: bool, mp4_trim: bool) -> bool {
    if codec == CODEC_ID_FLAC || codec == CODEC_ID_ALAC || codec == CODEC_ID_VORBIS || codec == CODEC_ID_OPUS
    {
        true
    } else if codec == CODEC_ID_MP3 || codec == CODEC_ID_MP2 || codec == CODEC_ID_MP1 {
        has_trim_info
    } else if codec == CODEC_ID_AAC {
        mp4_trim
    } else {
        short_name.starts_with("pcm")
    }
}

/// Mixes any channel layout to stereo: mono is copied to both sides, stereo passes through,
/// more channels are folded down with ITU-style coefficients (centre and surrounds at -3 dB,
/// LFE dropped), normalised so the fold-down cannot clip.
#[derive(Debug, Default)]
pub(crate) struct StereoMixer {
    layout: Option<(Channels, usize)>,
    /// Per input channel: (gain into left, gain into right).
    gains: Vec<(f32, f32)>,
}

impl StereoMixer {
    pub(crate) fn configure(&mut self, channels: &Channels, width: usize) {
        if self
            .layout
            .as_ref()
            .is_some_and(|(c, w)| c == channels && *w == width)
        {
            return;
        }
        self.gains = downmix_gains(channels, width);
        self.layout = Some((channels.clone(), width));
    }

    pub(crate) fn mix(&self, input: &[f32], out: &mut Vec<f32>) {
        let width = self.gains.len();
        match width {
            1 => {
                out.reserve(input.len() * 2);
                for &s in input {
                    out.push(s);
                    out.push(s);
                }
            }
            2 => out.extend_from_slice(input),
            _ => {
                out.reserve(input.len() / width * 2);
                for frame in input.chunks_exact(width) {
                    let (mut l, mut r) = (0.0f32, 0.0f32);
                    for (&s, &(gl, gr)) in frame.iter().zip(&self.gains) {
                        l += s * gl;
                        r += s * gr;
                    }
                    out.push(l);
                    out.push(r);
                }
            }
        }
    }
}

fn downmix_gains(channels: &Channels, width: usize) -> Vec<(f32, f32)> {
    const HALF_POWER: f32 = std::f32::consts::FRAC_1_SQRT_2;
    if width <= 2 {
        return vec![(1.0, 1.0); width.max(1)];
    }
    let mut gains: Vec<(f32, f32)> = match channels {
        Channels::Positioned(mask) if mask.bits().count_ones() as usize == width => (0..64)
            .map(|bit| Position::from_bits_truncate(1u64 << bit))
            .filter(|p| !p.is_empty() && mask.contains(*p))
            .map(|p| {
                if p == Position::FRONT_LEFT {
                    (1.0, 0.0)
                } else if p == Position::FRONT_RIGHT {
                    (0.0, 1.0)
                } else if p == Position::FRONT_CENTER {
                    (HALF_POWER, HALF_POWER)
                } else if p == Position::LFE1 || p.bits() & LFE_MASK != 0 {
                    (0.0, 0.0)
                } else if p.bits() & LEFT_MASK != 0 {
                    (HALF_POWER, 0.0)
                } else if p.bits() & RIGHT_MASK != 0 {
                    (0.0, HALF_POWER)
                } else {
                    (0.5, 0.5)
                }
            })
            .collect(),
        // Unlabelled channels: first two are left and right, the rest alternate.
        _ => (0..width)
            .map(|i| match i {
                0 => (1.0, 0.0),
                1 => (0.0, 1.0),
                i if i % 2 == 0 => (HALF_POWER, 0.0),
                _ => (0.0, HALF_POWER),
            })
            .collect(),
    };
    let left: f32 = gains.iter().map(|g| g.0).sum();
    let right: f32 = gains.iter().map(|g| g.1).sum();
    let norm = left.max(right);
    if norm > 1.0 {
        for g in &mut gains {
            g.0 /= norm;
            g.1 /= norm;
        }
    }
    gains
}

/// Positions that belong on the left (rear, side, left-of-centre and top-left speakers).
const LEFT_MASK: u64 = Position::REAR_LEFT.bits()
    | Position::FRONT_LEFT_CENTER.bits()
    | Position::SIDE_LEFT.bits()
    | Position::TOP_FRONT_LEFT.bits()
    | Position::TOP_REAR_LEFT.bits()
    | Position::TOP_SIDE_LEFT.bits()
    | Position::BOTTOM_FRONT_LEFT.bits()
    | Position::FRONT_LEFT_WIDE.bits();
const RIGHT_MASK: u64 = Position::REAR_RIGHT.bits()
    | Position::FRONT_RIGHT_CENTER.bits()
    | Position::SIDE_RIGHT.bits()
    | Position::TOP_FRONT_RIGHT.bits()
    | Position::TOP_REAR_RIGHT.bits()
    | Position::TOP_SIDE_RIGHT.bits()
    | Position::BOTTOM_FRONT_RIGHT.bits()
    | Position::FRONT_RIGHT_WIDE.bits();
const LFE_MASK: u64 = Position::LFE1.bits() | Position::LFE2.bits();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_copied_to_both_sides() {
        let mut mixer = StereoMixer::default();
        mixer.configure(&Channels::Positioned(Position::FRONT_CENTER), 1);
        let mut out = Vec::new();
        mixer.mix(&[0.25, -0.5], &mut out);
        assert_eq!(out, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn surround_folds_down_without_clipping() {
        let layout = Position::FRONT_LEFT
            | Position::FRONT_RIGHT
            | Position::FRONT_CENTER
            | Position::LFE1
            | Position::SIDE_LEFT
            | Position::SIDE_RIGHT;
        let mut mixer = StereoMixer::default();
        mixer.configure(&Channels::Positioned(layout), 6);
        // All channels at full scale must not exceed full scale after the fold-down.
        let mut out = Vec::new();
        mixer.mix(&[1.0; 6], &mut out);
        assert!(out.iter().all(|s| s.abs() <= 1.0 + 1e-6), "{out:?}");
        // Left-only content stays on the left.
        out.clear();
        mixer.mix(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0], &mut out);
        assert!(out[0] > 0.0 && out[1] == 0.0, "{out:?}");
        // The LFE is dropped.
        out.clear();
        mixer.mix(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut out);
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn opus_head_fields() {
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, 2]); // version, channels
        head.extend_from_slice(&312u16.to_le_bytes());
        head.extend_from_slice(&48_000u32.to_le_bytes());
        head.extend_from_slice(&(-1536i16).to_le_bytes()); // -6 dB
        head.push(0);
        assert_eq!(
            parse_opus_head(&head),
            Some(OpusHead {
                pre_skip: 312,
                output_gain: -1536
            })
        );
        assert_eq!(parse_opus_head(&head[..18]), None);
        assert_eq!(parse_opus_head(b"OpusTags........................"), None);
    }

    #[test]
    fn bitrates() {
        assert_eq!(
            bitrate("pcm_s16le", 44_100, 2, Some(16), None, None),
            Some(1_411_200)
        );
        // 1 MB over 8 s is 1 Mbit/s.
        assert_eq!(
            bitrate("flac", 44_100, 2, Some(16), Some(1_000_000), Some(8_000_000)),
            Some(1_000_000)
        );
        assert_eq!(bitrate("flac", 44_100, 2, None, Some(1_000_000), None), None);
        assert_eq!(bitrate("mp3", 44_100, 2, None, None, Some(1_000)), None);
    }
}
