//! Songs as mono audio at a recogniser's rate, for AI Lyrics (ADR-0019 §5).
//!
//! [`decode_mono`] decodes a file or a stream with the engine's decoders
//! (the same formats, gapless trims and Opus gain as playback), mixes it to
//! mono and resamples it with the engine's resampler. [`MonoAudio::wav`]
//! cuts a stretch out as 16-bit PCM WAV, which is what the plugins' `audio`
//! import hands out.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::decode::{Input, Source};
use crate::http::{HttpStream, StreamOptions};
use crate::resample::{FftResampler, Resample};
use crate::types::{EngineError, HttpSource};

/// The sample rate speech recognisers take (Whisper's).
pub const SPEECH_RATE: u32 = 16_000;

/// What [`decode_mono`] reads.
#[derive(Debug, Clone)]
pub enum AudioInput {
    /// A local file.
    File(PathBuf),
    /// A stream, fetched from its start with its headers.
    Http(HttpSource),
}

/// Mono audio, samples in [-1, 1].
#[derive(Debug, Clone, PartialEq)]
pub struct MonoAudio {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl MonoAudio {
    /// Its length in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        if self.rate == 0 {
            return 0;
        }
        self.samples.len() as u64 * 1000 / u64::from(self.rate)
    }

    /// The sample index at `ms`, at most the end.
    fn index(&self, ms: u64) -> usize {
        let at = ms.saturating_mul(u64::from(self.rate)) / 1000;
        usize::try_from(at).unwrap_or(usize::MAX).min(self.samples.len())
    }

    /// `[start_ms, start_ms + length_ms)`, cut at the end, as 16-bit PCM
    /// WAV at this audio's rate. A start past the end gives a WAV without
    /// samples.
    pub fn wav(&self, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, EngineError> {
        let start = self.index(start_ms);
        let end = self.index(start_ms.saturating_add(length_ms)).max(start);
        let samples = &self.samples[start..end];
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
        let mut writer = hound::WavWriter::new(&mut out, spec)?;
        {
            let count = u32::try_from(samples.len()).map_err(|_| EngineError::Wav("too long".into()))?;
            let mut pcm = writer.get_i16_writer(count);
            for &sample in samples {
                pcm.write_sample(to_i16(sample));
            }
            pcm.flush()?;
        }
        writer.finalize()?;
        Ok(out.into_inner())
    }
}

/// A sample in [-1, 1] as 16-bit PCM.
fn to_i16(sample: f32) -> i16 {
    let sample = if sample.is_finite() { sample } else { 0.0 };
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

/// Decodes `input` from its start, at most `max_ms` of it, mixed to mono
/// and resampled to `rate`. Setting `cancel` stops it soon after with
/// [`EngineError::Cancelled`]. Blocks for as long as decoding takes (about
/// a second for a song from a file).
pub fn decode_mono(
    input: &AudioInput,
    rate: u32,
    max_ms: u64,
    cancel: &AtomicBool,
) -> Result<MonoAudio, EngineError> {
    if rate == 0 {
        return Err(EngineError::Resample("the rate must be above 0".into()));
    }
    let input = match input {
        AudioInput::File(path) => Input::File(path.clone()),
        AudioInput::Http(source) => Input::Http(HttpStream::open(
            source.clone(),
            StreamOptions {
                id: 0,
                cache_dir: None,
                resolver: None,
            },
        )?),
    };
    let mut source = Source::open(&input)?;
    let in_rate = source.sample_rate();
    let max_frames = max_ms.saturating_mul(u64::from(in_rate)) / 1000;
    let mut resampler = if in_rate == rate {
        None
    } else {
        Some(FftResampler::new(in_rate, rate, 1)?)
    };
    let expected = source
        .total_frames()
        .and_then(|f| u64::try_from(f).ok())
        .unwrap_or(0);
    let mut out = Vec::with_capacity(
        usize::try_from(expected.min(max_frames) * u64::from(rate) / u64::from(in_rate.max(1))).unwrap_or(0),
    );
    let (mut stereo, mut mono) = (Vec::new(), Vec::new());
    let mut frames = 0u64;
    while frames < max_frames {
        if cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Cancelled);
        }
        stereo.clear();
        if !source.read(&mut stereo)? {
            break;
        }
        mono.clear();
        mono.extend(stereo.chunks_exact(2).map(|frame| 0.5 * (frame[0] + frame[1])));
        let left = usize::try_from(max_frames - frames).unwrap_or(usize::MAX);
        mono.truncate(left);
        frames += mono.len() as u64;
        match &mut resampler {
            Some(resampler) => resampler.push(&mono, &mut out)?,
            None => out.extend_from_slice(&mono),
        }
    }
    if let Some(resampler) = &mut resampler {
        resampler.finish(&mut out)?;
    }
    Ok(MonoAudio { rate, samples: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generated test tone (not music): `seconds` of a sine in both channels, as a WAV file.
    fn tone(dir: &std::path::Path, rate: u32, channels: u16, seconds: f32) -> PathBuf {
        let path = dir.join(format!("tone-{rate}-{channels}.wav"));
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        let frames = (rate as f32 * seconds) as u32;
        for n in 0..frames {
            let s =
                (0.5 * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / rate as f32).sin() * 32767.0) as i16;
            for _ in 0..channels {
                writer.write_sample(s).unwrap();
            }
        }
        writer.finalize().unwrap();
        path
    }

    /// A fresh folder for one test's files.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jf-engine-mono-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn decodes_to_16_khz_mono_with_the_right_length() {
        let dir = scratch("rates");
        let cancel = AtomicBool::new(false);
        for (rate, channels) in [(44_100, 2), (48_000, 1), (16_000, 2)] {
            let path = tone(&dir, rate, channels, 2.5);
            let audio = decode_mono(&AudioInput::File(path), SPEECH_RATE, u64::MAX, &cancel).unwrap();
            assert_eq!(audio.rate, SPEECH_RATE);
            assert!(
                audio.samples.len().abs_diff(40_000) <= 2,
                "{rate} Hz × {channels}: {} samples",
                audio.samples.len()
            );
            assert_eq!(audio.duration_ms(), 2_500);
            // The tone survives: about 0.5 × √½ RMS in the middle.
            let middle = &audio.samples[10_000..30_000];
            let rms = (middle.iter().map(|s| s * s).sum::<f32>() / middle.len() as f32).sqrt();
            assert!((rms - 0.3535).abs() < 0.01, "{rate} Hz: rms {rms}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stops_at_the_cap_and_when_cancelled() {
        let dir = scratch("cap");
        let path = tone(&dir, 44_100, 2, 3.0);
        let audio = decode_mono(
            &AudioInput::File(path.clone()),
            SPEECH_RATE,
            1_000,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(
            audio.samples.len().abs_diff(16_000) <= 2,
            "{}",
            audio.samples.len()
        );
        let cancelled = decode_mono(
            &AudioInput::File(path),
            SPEECH_RATE,
            u64::MAX,
            &AtomicBool::new(true),
        );
        assert!(matches!(cancelled, Err(EngineError::Cancelled)));
        let missing = decode_mono(
            &AudioInput::File(dir.join("missing.flac")),
            SPEECH_RATE,
            u64::MAX,
            &AtomicBool::new(false),
        );
        assert!(missing.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cuts_wav_stretches() {
        let audio = MonoAudio {
            rate: 16_000,
            samples: (0..48_000).map(|n| if n % 2 == 0 { 0.5 } else { -1.5 }).collect(),
        };
        let wav = audio.wav(1_000, 500).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(&wav)).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples.len(), 8_000);
        assert_eq!(&samples[..2], &[16_384, -32_767]);
        // Cut at the end, and past it.
        let tail = audio.wav(2_900, 1_000).unwrap();
        assert_eq!(hound::WavReader::new(Cursor::new(&tail)).unwrap().len(), 1_600);
        let none = audio.wav(5_000, 1_000).unwrap();
        assert_eq!(hound::WavReader::new(Cursor::new(&none)).unwrap().len(), 0);
        assert_eq!(audio.duration_ms(), 3_000);
    }
}
