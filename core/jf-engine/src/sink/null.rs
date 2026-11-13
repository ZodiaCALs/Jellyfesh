//! A sink that discards audio.

use std::time::Duration;

use super::pacer::Pacer;
use super::{Sink, frames_to_us};
use crate::types::EngineError;

const DEFAULT_RATE: u32 = 48_000;

/// Discards everything. Offline it accepts audio as fast as the engine produces it; in
/// real-time mode it is paced like a device with a `buffer_ms` buffer, which is useful for
/// exercising the clock and the anchor without a sound card.
#[derive(Debug)]
pub struct NullSink {
    rate: u32,
    rate_fixed: bool,
    started: bool,
    frames: u64,
    pacer: Option<Pacer>,
}

impl NullSink {
    /// An offline null sink. `sample_rate: None` adopts the first track's rate.
    pub fn new(sample_rate: Option<u32>) -> Self {
        Self::create(sample_rate, None)
    }

    /// A null sink paced like a device with a `buffer_ms` buffer.
    pub fn realtime(sample_rate: Option<u32>, buffer_ms: u32) -> Self {
        Self::create(sample_rate, Some(buffer_ms))
    }

    fn create(sample_rate: Option<u32>, buffer_ms: Option<u32>) -> Self {
        let rate = sample_rate.filter(|&r| r > 0).unwrap_or(DEFAULT_RATE);
        Self {
            rate,
            rate_fixed: sample_rate.is_some(),
            started: false,
            frames: 0,
            pacer: buffer_ms.map(|ms| Pacer::new(rate, ms)),
        }
    }

    /// Frames written so far.
    pub fn frames_written(&self) -> u64 {
        self.frames
    }
}

impl Sink for NullSink {
    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn channels(&self) -> u16 {
        2
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        let frames = (samples.len() / 2) as u64;
        if frames == 0 {
            return Ok(());
        }
        self.started = true;
        self.frames += frames;
        if let Some(pacer) = self.pacer.as_mut() {
            pacer.write(frames);
        }
        Ok(())
    }

    fn flush(&mut self) -> u64 {
        self.pacer.as_mut().map_or(0, Pacer::flush)
    }

    fn flush_faded(&mut self, fade_frames: u64) -> u64 {
        self.pacer.as_mut().map_or(0, |p| p.flush_keep(fade_frames).0)
    }

    fn latency_us(&self) -> u64 {
        self.pacer
            .as_ref()
            .map_or(0, |p| frames_to_us(p.latency_frames(), self.rate))
    }

    fn prefer_sample_rate(&mut self, rate: u32) {
        if self.rate_fixed || self.started || rate == 0 {
            return;
        }
        self.rate = rate;
        if let Some(pacer) = self.pacer.as_mut() {
            pacer.set_rate(rate);
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if let Some(pacer) = self.pacer.as_mut() {
            pacer.set_paused(paused);
        }
    }

    fn drain(&mut self, timeout: Duration) -> Result<bool, EngineError> {
        Ok(self.pacer.as_mut().is_none_or(|p| p.drain(timeout)))
    }
}
