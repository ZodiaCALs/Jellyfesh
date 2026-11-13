//! A sink that writes a 32-bit float WAV file.

use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;

use hound::{SampleFormat, WavSpec, WavWriter};

use super::pacer::Pacer;
use super::{Sink, frames_to_us};
use crate::types::EngineError;

/// Sample rate used until the first track tells the sink which rate to take.
const DEFAULT_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

/// Writes stereo 32-bit float WAV.
///
/// Offline (the default) every write goes straight to the file and the engine renders as fast
/// as it can. Real-time mode behaves like a device with a `buffer_ms` buffer: samples reach
/// the file when the virtual device "plays" them, and a flush (seek, skip) drops the ones that
/// have not been played, so the file holds exactly what a listener would have heard.
///
/// With no fixed rate the sink adopts the sample rate of the first track (so a 44.1 kHz file
/// renders bit-transparently at unity gain); later tracks at other rates are resampled.
pub struct WavSink {
    path: PathBuf,
    writer: Option<WavWriter<BufWriter<File>>>,
    rate: u32,
    rate_fixed: bool,
    started: bool,
    /// Real-time mode: the virtual device clock and the samples it has not played yet.
    paced: Option<(Pacer, VecDeque<f32>)>,
}

impl fmt::Debug for WavSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WavSink")
            .field("path", &self.path)
            .field("rate", &self.rate)
            .field("realtime", &self.paced.is_some())
            .finish_non_exhaustive()
    }
}

impl WavSink {
    /// An offline sink. `sample_rate: None` adopts the first track's rate.
    pub fn new(path: impl AsRef<Path>, sample_rate: Option<u32>) -> Result<Self, EngineError> {
        Self::create(path.as_ref(), sample_rate, None)
    }

    /// A sink paced like a device with a `buffer_ms` buffer.
    pub fn realtime(
        path: impl AsRef<Path>,
        sample_rate: Option<u32>,
        buffer_ms: u32,
    ) -> Result<Self, EngineError> {
        Self::create(path.as_ref(), sample_rate, Some(buffer_ms))
    }

    fn create(path: &Path, sample_rate: Option<u32>, buffer_ms: Option<u32>) -> Result<Self, EngineError> {
        let rate = sample_rate.unwrap_or(DEFAULT_RATE);
        if rate == 0 {
            return Err(EngineError::Wav("sample rate must be positive".into()));
        }
        let writer = WavWriter::create(path, spec(rate))?;
        Ok(Self {
            path: path.to_path_buf(),
            writer: Some(writer),
            rate,
            rate_fixed: sample_rate.is_some(),
            started: false,
            paced: buffer_ms.map(|ms| (Pacer::new(rate, ms), VecDeque::new())),
        })
    }

    /// The file being written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write_to_file(
        writer: &mut Option<WavWriter<BufWriter<File>>>,
        samples: impl Iterator<Item = f32>,
    ) -> Result<(), EngineError> {
        let writer = writer
            .as_mut()
            .ok_or_else(|| EngineError::Wav("WAV writer is closed".into()))?;
        for sample in samples {
            writer.write_sample(sample)?;
        }
        Ok(())
    }

    /// Real-time mode: moves samples the virtual device has played into the file.
    fn commit_played(&mut self) -> Result<(), EngineError> {
        let Some((pacer, queue)) = self.paced.as_mut() else {
            return Ok(());
        };
        let unplayed = pacer.pending() as usize * usize::from(CHANNELS);
        let played = queue.len().saturating_sub(unplayed);
        Self::write_to_file(&mut self.writer, queue.drain(..played))
    }
}

fn spec(rate: u32) -> WavSpec {
    WavSpec {
        channels: CHANNELS,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    }
}

impl Sink for WavSink {
    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn channels(&self) -> u16 {
        CHANNELS
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        if samples.is_empty() {
            return Ok(());
        }
        self.started = true;
        if self.paced.is_none() {
            return Self::write_to_file(&mut self.writer, samples.iter().copied());
        }
        self.commit_played()?;
        if let Some((pacer, queue)) = self.paced.as_mut() {
            queue.extend(samples.iter().copied());
            pacer.write((samples.len() / usize::from(CHANNELS)) as u64);
        }
        self.commit_played()
    }

    fn flush(&mut self) -> u64 {
        let Some((pacer, queue)) = self.paced.as_mut() else {
            return 0;
        };
        // One clock reading decides both what was played (kept) and what is dropped, so the
        // file and the engine's frame count agree.
        let dropped = pacer.flush();
        let played = queue
            .len()
            .saturating_sub(dropped as usize * usize::from(CHANNELS));
        let result = Self::write_to_file(&mut self.writer, queue.drain(..played));
        queue.clear();
        if let Err(err) = result {
            tracing::warn!("WAV sink: {err}");
        }
        dropped
    }

    fn flush_faded(&mut self, fade_frames: u64) -> u64 {
        let channels = usize::from(CHANNELS);
        let Some((pacer, queue)) = self.paced.as_mut() else {
            return 0;
        };
        // The queue holds played samples not yet in the file, then the unplayed ones: the first
        // `kept` unplayed frames stay and fade out, the rest go.
        let (dropped, kept) = pacer.flush_keep(fade_frames);
        let keep = queue.len().saturating_sub(dropped as usize * channels);
        queue.truncate(keep);
        let kept = kept as usize;
        let from = keep.saturating_sub(kept * channels);
        for (i, frame) in queue
            .range_mut(from..keep)
            .collect::<Vec<_>>()
            .chunks_mut(channels)
            .enumerate()
        {
            let gain = 1.0 - (i + 1) as f32 / kept.max(1) as f32;
            for sample in frame {
                **sample *= gain;
            }
        }
        dropped
    }

    fn latency_us(&self) -> u64 {
        match &self.paced {
            Some((pacer, _)) => frames_to_us(pacer.latency_frames(), self.rate),
            None => 0,
        }
    }

    fn prefer_sample_rate(&mut self, rate: u32) {
        if self.rate_fixed || self.started || rate == 0 || rate == self.rate {
            return;
        }
        // Nothing has been written yet: start the file again at the track's rate.
        drop(self.writer.take());
        match WavWriter::create(&self.path, spec(rate)) {
            Ok(writer) => {
                self.writer = Some(writer);
                self.rate = rate;
                if let Some((pacer, _)) = self.paced.as_mut() {
                    pacer.set_rate(rate);
                }
            }
            Err(err) => {
                tracing::warn!(
                    "WAV sink: cannot restart {} at {rate} Hz: {err}",
                    self.path.display()
                );
                self.writer = WavWriter::create(&self.path, spec(self.rate)).ok();
            }
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if let Some((pacer, _)) = self.paced.as_mut() {
            pacer.set_paused(paused);
        }
    }

    fn drain(&mut self, timeout: Duration) -> Result<bool, EngineError> {
        let done = match self.paced.as_mut() {
            Some((pacer, _)) => pacer.drain(timeout),
            None => true,
        };
        self.commit_played()?;
        if done && let Some(writer) = self.writer.as_mut() {
            // Rewrites the header so the file is complete even while the engine lives on.
            writer.flush()?;
        }
        Ok(done)
    }
}

impl Drop for WavSink {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take()
            && let Err(err) = writer.finalize()
        {
            tracing::warn!("WAV sink: cannot finalize {}: {err}", self.path.display());
        }
    }
}
