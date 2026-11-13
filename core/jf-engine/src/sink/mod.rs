//! Output sinks: where the engine thread writes finished audio.

use std::time::Duration;

use crate::types::EngineError;

#[cfg(feature = "device")]
mod device;
mod null;
mod pacer;
mod wav;

#[cfg(feature = "device")]
pub(crate) use device::default_device_id;
#[cfg(feature = "device")]
pub use device::{DeviceSink, list_devices};
pub use null::NullSink;
pub use wav::WavSink;

/// An audio output. The engine thread owns its sink and is the only caller.
///
/// Samples are interleaved `f32` in `[-1, 1]` at [`sample_rate`](Sink::sample_rate) with
/// [`channels`](Sink::channels) channels. The engine counts every frame it writes; the
/// audible position is that count minus [`latency_us`](Sink::latency_us).
pub trait Sink: Send {
    /// Output sample rate in Hz.
    fn sample_rate(&self) -> u32;

    /// Output channel count.
    fn channels(&self) -> u16;

    /// Queues interleaved samples (a whole number of frames), blocking while the sink's buffer
    /// is full. Real-time sinks return within about one buffer period.
    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError>;

    /// Discards audio that was written but has not been played yet, and returns how many frames
    /// were discarded. Offline sinks have nothing to discard and return 0.
    fn flush(&mut self) -> u64;

    /// Like [`flush`](Sink::flush), but the first `fade_frames` of the unplayed audio still play,
    /// faded to silence, so cutting playback short does not click. Returns the frames discarded
    /// (the faded ones are heard, so they are not counted). Sinks that cannot fade just flush.
    fn flush_faded(&mut self, fade_frames: u64) -> u64 {
        let _ = fade_frames;
        self.flush()
    }

    /// Identifies the audio device behind the sink (cpal's device id), for following the
    /// system default. `None` for sinks that are not devices.
    fn device_id(&self) -> Option<String> {
        None
    }

    /// Time between a sample being written now and being heard: buffered audio plus the
    /// device's own latency, in microseconds.
    fn latency_us(&self) -> u64;

    /// The engine is about to start a track at `rate`. A sink that has not written anything yet
    /// may switch to that rate so the first track plays without resampling.
    fn prefer_sample_rate(&mut self, _rate: u32) {}

    /// Pauses or resumes playback of the buffered audio, keeping it.
    fn set_paused(&mut self, _paused: bool) {}

    /// No more audio follows for now: waits up to `timeout` for buffered audio to play out and
    /// returns `true` once it has (and has been made durable, for files).
    fn drain(&mut self, _timeout: Duration) -> Result<bool, EngineError> {
        Ok(true)
    }

    /// Applies the volume at the output (for example in the device callback) and returns
    /// `true`, or returns `false` to have the engine apply it before writing.
    fn set_volume(&mut self, _linear: f32) -> bool {
        false
    }

    /// Times the output ran dry while it was expected to play.
    fn underruns(&self) -> u64 {
        0
    }

    /// Whether the output is taking audio: `false` after a flush or a resume until the device
    /// consumes its first samples, so the playback clock does not start before the audio does.
    /// Sinks without a start-up delay always return `true`.
    fn output_started(&self) -> bool {
        true
    }
}

impl<S: Sink + ?Sized> Sink for Box<S> {
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }
    fn channels(&self) -> u16 {
        (**self).channels()
    }
    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        (**self).write(samples)
    }
    fn flush(&mut self) -> u64 {
        (**self).flush()
    }
    fn flush_faded(&mut self, fade_frames: u64) -> u64 {
        (**self).flush_faded(fade_frames)
    }
    fn device_id(&self) -> Option<String> {
        (**self).device_id()
    }
    fn latency_us(&self) -> u64 {
        (**self).latency_us()
    }
    fn prefer_sample_rate(&mut self, rate: u32) {
        (**self).prefer_sample_rate(rate)
    }
    fn set_paused(&mut self, paused: bool) {
        (**self).set_paused(paused)
    }
    fn drain(&mut self, timeout: Duration) -> Result<bool, EngineError> {
        (**self).drain(timeout)
    }
    fn set_volume(&mut self, linear: f32) -> bool {
        (**self).set_volume(linear)
    }
    fn underruns(&self) -> u64 {
        (**self).underruns()
    }
    fn output_started(&self) -> bool {
        (**self).output_started()
    }
}

/// A sound output device as reported by `list_devices`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    /// Stable identifier (cpal's `DeviceId`, for example `alsa:default`), unique per device.
    /// [`OutputKind::Device`](crate::OutputKind::Device) accepts it as the name.
    pub id: String,
    /// Human-readable name. Several devices can share one.
    pub name: String,
    pub is_default: bool,
    /// Common sample rates the device accepts, ascending.
    pub sample_rates: Vec<u32>,
}

/// Frames to microseconds, rounded down.
pub(crate) fn frames_to_us(frames: u64, rate: u32) -> u64 {
    if rate == 0 {
        return 0;
    }
    (u128::from(frames) * 1_000_000 / u128::from(rate)) as u64
}
