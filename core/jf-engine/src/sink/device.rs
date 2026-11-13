//! Audio device output through cpal.
//!
//! The engine thread pushes samples into a lock-free SPSC ring buffer; the real-time callback
//! only pops from it, applies the volume and converts the sample format. The callback never
//! locks, allocates or logs: everything it shares with the engine thread is an atomic.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::{DeviceInfo, Sink, frames_to_us};
use crate::types::EngineError;

/// A write that makes no progress for this long means the device stopped consuming.
const STALL_TIMEOUT: Duration = Duration::from_secs(2);
/// Callback-side scratch buffer, in samples. Larger device buffers are handled in pieces.
const SCRATCH_SAMPLES: usize = 16_384;
/// Extra wait after the device's own buffer should have played out, for the stream's
/// bookkeeping (timestamps are estimates).
const DRAIN_MARGIN: Duration = Duration::from_millis(5);
/// Upper bound on the play-out wait, in case a backend reports a nonsense delay.
const DRAIN_TAIL_MAX: Duration = Duration::from_secs(2);
/// Rates reported by [`list_devices`] when a device accepts a range.
const COMMON_RATES: [u32; 8] = [22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// State shared between the engine thread and the real-time callback. Atomics only.
#[derive(Debug, Default)]
struct Shared {
    /// Target volume as `f32` bits; the callback ramps towards it.
    volume_bits: AtomicU32,
    /// Output silence and keep the buffered audio.
    paused: AtomicBool,
    /// Audio is expected; an empty buffer now counts as an underrun.
    streaming: AtomicBool,
    /// The callback drops samples until it has consumed this many (a flush).
    discard_until: AtomicU64,
    /// Samples the callback still plays, fading out, before it drops the flushed rest.
    fade_samples: AtomicU64,
    /// Bumped by every flush, after `discard_until` and `fade_samples` are set.
    flush_seq: AtomicU64,
    /// Samples the callback has taken out of the ring (played or dropped).
    popped: AtomicU64,
    underruns: AtomicU64,
    /// Latest callback-to-playback delay reported by the device.
    device_latency_ns: AtomicU64,
    /// Frames the device asked for in the latest callback (its buffer period).
    callback_frames: AtomicU64,
    /// The stream reported an error it cannot recover from.
    failed: AtomicBool,
    /// Recoverable stream errors (cpal recovers from these and keeps playing).
    stream_errors: AtomicU64,
}

/// Plays through an output device with cpal.
pub struct DeviceSink {
    stream: cpal::Stream,
    producer: HeapProd<f32>,
    shared: Arc<Shared>,
    name: String,
    /// cpal's device id, as text.
    id: Option<String>,
    rate: u32,
    channels: u16,
    /// Samples pushed into the ring so far.
    pushed: u64,
    /// While draining: when the device will have played what it already took.
    drain_deadline: Option<Instant>,
    /// Samples counted as consumed when the output last (re)started; the output has started
    /// once the callback's count passes it.
    start_mark: u64,
    /// Recoverable stream errors already logged.
    reported_errors: u64,
    /// The last flush's `discard_until`.
    discard_mark: u64,
    /// Where the callback's fade-out after the last flush ends (estimated: the callback starts
    /// it at its own position).
    fade_end: u64,
}

impl fmt::Debug for DeviceSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceSink")
            .field("name", &self.name)
            .field("rate", &self.rate)
            .field("channels", &self.channels)
            .field("underruns", &self.shared.underruns.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl DeviceSink {
    /// Opens `name` (or the default output when `None`) at its default configuration, with a
    /// ring buffer of `buffer_ms`. A name matches a device's id (`alsa:default` or just
    /// `default`) or its display name. Fails cleanly when there is no usable device.
    pub fn open(name: Option<&str>, buffer_ms: u32) -> Result<Self, EngineError> {
        let host = cpal::default_host();
        let device = match name {
            None => host
                .default_output_device()
                .ok_or_else(|| EngineError::DeviceNotFound("no default output device".into()))?,
            Some(wanted) => host
                .output_devices()
                .map_err(|e| EngineError::Device(e.to_string()))?
                .find(|d| device_matches(d, wanted))
                .ok_or_else(|| EngineError::DeviceNotFound(wanted.to_string()))?,
        };
        let device_name = device.to_string();
        let device_id = device.id().ok().map(|id| id.to_string());
        let supported = device
            .default_output_config()
            .map_err(|e| EngineError::Device(format!("{device_name}: {e}")))?;
        let rate = supported.sample_rate();
        let channels = supported.channels();
        if rate == 0 || channels == 0 {
            return Err(EngineError::Device(format!(
                "{device_name}: unusable default configuration"
            )));
        }
        let config = cpal::StreamConfig {
            channels,
            sample_rate: rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let frames = (u64::from(rate) * u64::from(buffer_ms.max(20)) / 1000).max(256) as usize;
        let (producer, consumer) = HeapRb::<f32>::new(frames * usize::from(channels)).split();
        let shared = Arc::new(Shared::default());
        shared.volume_bits.store(1.0f32.to_bits(), Ordering::Relaxed);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, consumer, &shared),
            SampleFormat::F64 => build::<f64>(&device, &config, consumer, &shared),
            SampleFormat::I16 => build::<i16>(&device, &config, consumer, &shared),
            SampleFormat::I32 => build::<i32>(&device, &config, consumer, &shared),
            SampleFormat::U16 => build::<u16>(&device, &config, consumer, &shared),
            SampleFormat::I8 => build::<i8>(&device, &config, consumer, &shared),
            SampleFormat::U8 => build::<u8>(&device, &config, consumer, &shared),
            other => Err(EngineError::Device(format!(
                "{device_name}: unsupported sample format {other}"
            ))),
        }?;
        stream
            .play()
            .map_err(|e| EngineError::Device(format!("{device_name}: {e}")))?;
        tracing::info!("audio device {device_name}: {rate} Hz, {channels} channels, {buffer_ms} ms buffer");
        Ok(Self {
            stream,
            producer,
            shared,
            name: device_name,
            id: device_id,
            rate,
            channels,
            pushed: 0,
            drain_deadline: None,
            start_mark: 0,
            reported_errors: 0,
            discard_mark: 0,
            fade_end: 0,
        })
    }

    /// The device's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Samples still to be heard: the fade-out after a flush, plus what was pushed since.
    fn pending_samples(&self) -> u64 {
        let consumed = self.shared.popped.load(Ordering::Acquire);
        let fading = self.fade_end.saturating_sub(consumed);
        fading + self.pushed.saturating_sub(consumed.max(self.discard_mark))
    }

    /// Drops everything pushed so far except the next `fade_samples`, which the callback plays
    /// with a linear fade to silence. Returns the samples dropped.
    fn flush_samples(&mut self, fade_samples: u64) -> u64 {
        self.drain_deadline = None;
        // Everything pushed so far is dropped: only samples after it count as playing.
        self.start_mark = self.pushed;
        let consumed = self.shared.popped.load(Ordering::Acquire);
        let unplayed = self.pushed.saturating_sub(consumed.max(self.discard_mark));
        let channels = u64::from(self.channels);
        let fade = fade_samples.min(unplayed) / channels * channels;
        self.fade_end = consumed.max(self.discard_mark) + fade;
        self.discard_mark = self.pushed;
        self.shared.streaming.store(false, Ordering::Relaxed);
        self.shared.fade_samples.store(fade, Ordering::Relaxed);
        self.shared.discard_until.store(self.pushed, Ordering::Relaxed);
        self.shared.flush_seq.fetch_add(1, Ordering::Release);
        unplayed - fade
    }
}

/// Whether `device` is the one called `wanted`: its full id (`alsa:default`), its host-specific
/// id (`default`), or its display name.
fn device_matches(device: &cpal::Device, wanted: &str) -> bool {
    device
        .id()
        .is_ok_and(|id| id.to_string() == wanted || id.id() == wanted)
        || device.to_string() == wanted
}

/// cpal's id of the system's default output device, as text.
pub(crate) fn default_device_id() -> Option<String> {
    let device = cpal::default_host().default_output_device()?;
    device.id().ok().map(|id| id.to_string())
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: HeapCons<f32>,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, EngineError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let callback_shared = Arc::clone(shared);
    let error_shared = Arc::clone(shared);
    // Everything the callback needs is allocated here, before the stream starts.
    let mut scratch = vec![0.0f32; SCRATCH_SAMPLES - SCRATCH_SAMPLES % channels];
    let mut state = CallbackState {
        popped: 0,
        flush_seq: 0,
        fade_start: 0,
        fade_end: 0,
        discard_until: 0,
        volume: 1.0,
        // One-pole smoothing towards the target volume, about 5 ms.
        ramp: 1.0 - (-1.0 / (0.005 * f64::from(rate))).exp() as f32,
    };
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                render(
                    data,
                    info,
                    &mut consumer,
                    &callback_shared,
                    &mut scratch,
                    &mut state,
                    channels,
                );
            },
            move |err: cpal::Error| match err.kind() {
                // The device or the audio server is gone, or the stream must be rebuilt.
                cpal::ErrorKind::DeviceNotAvailable
                | cpal::ErrorKind::HostUnavailable
                | cpal::ErrorKind::StreamInvalidated => error_shared.failed.store(true, Ordering::Release),
                // An underrun: cpal re-prepares the stream and playback continues.
                cpal::ErrorKind::Xrun => {
                    error_shared.underruns.fetch_add(1, Ordering::Relaxed);
                }
                // Anything else (an unsupported pause, a rerouted default device, a denied
                // real-time priority) leaves the stream playing. A stream that really stopped
                // is caught by the stall check in `write`.
                _ => {
                    error_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
                }
            },
            None,
        )
        .map_err(|e| EngineError::Device(e.to_string()))
}

struct CallbackState {
    popped: u64,
    /// The last flush seen, and the fade-out and discard it asked for.
    flush_seq: u64,
    fade_start: u64,
    fade_end: u64,
    discard_until: u64,
    volume: f32,
    ramp: f32,
}

/// The real-time callback body: no locks, no allocation, no logging.
fn render<T>(
    data: &mut [T],
    info: &cpal::OutputCallbackInfo,
    consumer: &mut HeapCons<f32>,
    shared: &Shared,
    scratch: &mut [f32],
    state: &mut CallbackState,
    channels: usize,
) where
    T: SizedSample + FromSample<f32>,
{
    let stamp = info.timestamp();
    if let Some(latency) = stamp.playback.checked_duration_since(stamp.callback) {
        let nanos = u64::try_from(latency.as_nanos()).unwrap_or(u64::MAX);
        shared.device_latency_ns.store(nanos, Ordering::Relaxed);
    }
    shared
        .callback_frames
        .store((data.len() / channels.max(1)) as u64, Ordering::Relaxed);

    // A flush: fade out the first part of what the engine wrote before it, drop the rest.
    let seq = shared.flush_seq.load(Ordering::Acquire);
    if seq != state.flush_seq {
        state.flush_seq = seq;
        state.discard_until = shared.discard_until.load(Ordering::Relaxed);
        let fade = shared.fade_samples.load(Ordering::Relaxed);
        state.fade_start = state.popped;
        state.fade_end = (state.popped + fade).min(state.discard_until.max(state.popped));
    }

    if shared.paused.load(Ordering::Acquire) {
        data.fill(T::EQUILIBRIUM);
        shared.popped.store(state.popped, Ordering::Release);
        return;
    }

    let target = f32::from_bits(shared.volume_bits.load(Ordering::Relaxed));
    let mut done = 0;
    while done < data.len() {
        if state.popped >= state.fade_end && state.popped < state.discard_until {
            let available = consumer.occupied_len() as u64;
            let skip = (state.discard_until - state.popped).min(available) as usize;
            state.popped += consumer.skip(skip) as u64;
        }
        let fading = state.popped < state.fade_end;
        let mut want = (data.len() - done).min(scratch.len());
        if fading {
            want = want.min((state.fade_end - state.popped) as usize);
        }
        // Whole frames only, so the channel order never slips.
        let available = consumer.occupied_len() / channels * channels;
        let got = consumer.pop_slice(&mut scratch[..want.min(available)]);
        let fade_len = (state.fade_end - state.fade_start).max(1) as f32;
        for (k, (out_frame, in_frame)) in data[done..done + got]
            .chunks_exact_mut(channels)
            .zip(scratch[..got].chunks_exact(channels))
            .enumerate()
        {
            state.volume += (target - state.volume) * state.ramp;
            let gain = if fading {
                let at = state.popped + (k * channels) as u64 - state.fade_start;
                state.volume * (1.0 - at as f32 / fade_len).max(0.0)
            } else {
                state.volume
            };
            for (out, &sample) in out_frame.iter_mut().zip(in_frame) {
                *out = T::from_sample(sample * gain);
            }
        }
        state.popped += got as u64;
        done += got;
        if got < want {
            if shared.streaming.load(Ordering::Relaxed) {
                shared.underruns.fetch_add(1, Ordering::Relaxed);
            }
            data[done..].fill(T::EQUILIBRIUM);
            break;
        }
    }
    shared.popped.store(state.popped, Ordering::Release);
}

impl Sink for DeviceSink {
    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        if samples.is_empty() {
            return Ok(());
        }
        self.shared.streaming.store(true, Ordering::Relaxed);
        self.drain_deadline = None;
        let errors = self.shared.stream_errors.load(Ordering::Relaxed);
        if errors > self.reported_errors {
            tracing::debug!(
                "{}: {} recoverable stream error(s)",
                self.name,
                errors - self.reported_errors
            );
            self.reported_errors = errors;
        }
        let mut offset = 0;
        let mut last_progress = Instant::now();
        while offset < samples.len() {
            if self.shared.failed.load(Ordering::Acquire) {
                return Err(EngineError::Device(format!(
                    "{}: the output stream failed",
                    self.name
                )));
            }
            let pushed = self.producer.push_slice(&samples[offset..]);
            offset += pushed;
            self.pushed += pushed as u64;
            if offset == samples.len() {
                break;
            }
            if pushed > 0 {
                last_progress = Instant::now();
            } else if last_progress.elapsed() > STALL_TIMEOUT {
                return Err(EngineError::Device(format!(
                    "{}: the device stopped playing",
                    self.name
                )));
            }
            // Sleep about as long as the device needs to free the space still missing.
            let missing_frames = ((samples.len() - offset) / usize::from(self.channels)) as u64;
            let wait = Duration::from_micros(frames_to_us(missing_frames, self.rate));
            std::thread::sleep(wait.clamp(Duration::from_millis(1), Duration::from_millis(20)));
        }
        Ok(())
    }

    fn flush(&mut self) -> u64 {
        self.flush_samples(0) / u64::from(self.channels)
    }

    fn flush_faded(&mut self, fade_frames: u64) -> u64 {
        let channels = u64::from(self.channels);
        self.flush_samples(fade_frames.saturating_mul(channels)) / channels
    }

    fn device_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn latency_us(&self) -> u64 {
        let buffered = frames_to_us(self.pending_samples() / u64::from(self.channels), self.rate);
        buffered + self.shared.device_latency_ns.load(Ordering::Relaxed) / 1000
    }

    fn set_paused(&mut self, paused: bool) {
        if paused {
            self.shared.streaming.store(false, Ordering::Relaxed);
            self.shared.paused.store(true, Ordering::Release);
            // Stopping the stream saves power; backends that cannot pause play silence.
            if let Err(err) = self.stream.pause() {
                tracing::debug!("{}: pause not supported ({err}); playing silence", self.name);
            }
        } else {
            self.start_mark = self.shared.popped.load(Ordering::Acquire);
            if let Err(err) = self.stream.play() {
                tracing::warn!("{}: cannot resume the stream: {err}", self.name);
            }
            self.shared.paused.store(false, Ordering::Release);
        }
    }

    fn drain(&mut self, timeout: Duration) -> Result<bool, EngineError> {
        self.shared.streaming.store(false, Ordering::Relaxed);
        if self.shared.failed.load(Ordering::Acquire) {
            return Ok(true);
        }
        let poll = timeout.max(Duration::from_millis(1));
        let pending_frames = self.pending_samples() / u64::from(self.channels);
        if pending_frames > 0 {
            self.drain_deadline = None;
            let wait = Duration::from_micros(frames_to_us(pending_frames, self.rate));
            std::thread::sleep(wait.clamp(Duration::from_millis(1), poll));
            return Ok(false);
        }
        // The ring is empty, but the device still holds what its last callbacks took from
        // it. Pausing or closing the stream now would cut the end of the audio, so wait
        // until the device has played that too.
        let now = Instant::now();
        let deadline = *self.drain_deadline.get_or_insert_with(|| {
            now + device_tail(
                self.shared.device_latency_ns.load(Ordering::Relaxed),
                self.shared.callback_frames.load(Ordering::Relaxed),
                self.rate,
            )
        });
        if now >= deadline {
            return Ok(true);
        }
        std::thread::sleep((deadline - now).min(poll));
        Ok(Instant::now() >= deadline)
    }

    fn set_volume(&mut self, linear: f32) -> bool {
        self.shared.volume_bits.store(linear.to_bits(), Ordering::Relaxed);
        true
    }

    fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    fn output_started(&self) -> bool {
        self.shared.popped.load(Ordering::Acquire) > self.start_mark
    }
}

/// How long a device needs to play out the audio it has already taken from the ring: its
/// reported callback-to-playback delay, one callback period, and a small margin.
fn device_tail(latency_ns: u64, callback_frames: u64, rate: u32) -> Duration {
    Duration::from_nanos(latency_ns)
        .saturating_add(Duration::from_micros(frames_to_us(callback_frames, rate.max(1))))
        .saturating_add(DRAIN_MARGIN)
        .min(DRAIN_TAIL_MAX)
}

/// Lists the output devices of the default host. Without a sound card this returns an empty
/// list or an error, never panics.
///
/// The default device is matched by id, not by name: ALSA calls its default "Default Audio
/// Device" while the enumerated `default` entry carries its plugin's description. When the
/// enumeration does not include the default at all, it is listed first.
pub fn list_devices() -> Result<Vec<DeviceInfo>, EngineError> {
    let host = cpal::default_host();
    let default = host.default_output_device();
    let default_id = default.as_ref().and_then(|d| d.id().ok());
    let devices = host
        .output_devices()
        .map_err(|e| EngineError::Device(e.to_string()))?;
    let mut list = Vec::new();
    for device in devices {
        let id = device.id().ok();
        let is_default = id.is_some() && id == default_id;
        list.push(device_info(&device, is_default));
    }
    if !list.iter().any(|d| d.is_default)
        && let Some(device) = default
    {
        list.insert(0, device_info(&device, true));
    }
    Ok(list)
}

fn device_info(device: &cpal::Device, is_default: bool) -> DeviceInfo {
    let name = device.to_string();
    let id = device.id().map_or_else(|_| name.clone(), |id| id.to_string());
    let mut sample_rates = Vec::new();
    if let Ok(configs) = device.supported_output_configs() {
        for range in configs {
            let (min, max) = (range.min_sample_rate(), range.max_sample_rate());
            sample_rates.extend(COMMON_RATES.iter().copied().filter(|r| (min..=max).contains(r)));
            if !COMMON_RATES.iter().any(|r| (min..=max).contains(r)) {
                sample_rates.push(max);
            }
        }
    }
    sample_rates.sort_unstable();
    sample_rates.dedup();
    DeviceInfo {
        id,
        name,
        is_default,
        sample_rates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_play_out_wait_covers_the_device_delay_and_one_period() {
        // 20 ms reported delay, 1024-frame periods at 48 kHz (21.3 ms), 5 ms margin.
        let tail = device_tail(20_000_000, 1024, 48_000);
        assert!((tail.as_secs_f64() - 0.046_333).abs() < 0.000_5, "{tail:?}");
        // Nothing reported yet: still one margin.
        assert_eq!(device_tail(0, 0, 48_000), DRAIN_MARGIN);
        // A nonsense delay is capped.
        assert_eq!(device_tail(u64::MAX, u64::MAX, 1), DRAIN_TAIL_MAX);
    }
}
