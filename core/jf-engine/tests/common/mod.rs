//! Shared helpers for the engine tests: generated test signals, offline renders and analysis.
//! Every signal is synthesised here; no recordings are used.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use jf_engine::{Engine, EngineError, EngineEvent, PlayItem, Sink, WavSink};
use parking_lot::Mutex;

pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// `amplitude × sin(2π f n / rate)` for frames `start..start + frames` of one continuous sine.
pub fn sine(freq: f64, rate: u32, start: usize, frames: usize, amplitude: f64) -> Vec<f64> {
    (start..start + frames)
        .map(|n| amplitude * (2.0 * PI * freq * n as f64 / f64::from(rate)).sin())
        .collect()
}

/// A mono signal duplicated into interleaved stereo.
pub fn stereo(mono: &[f64]) -> Vec<f64> {
    mono.iter().flat_map(|&s| [s, s]).collect()
}

/// Writes 32-bit float WAV (exact samples).
pub fn write_wav_f32(path: &Path, rate: u32, channels: u16, interleaved: &[f64]) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for &s in interleaved {
        writer.write_sample(s as f32).unwrap();
    }
    writer.finalize().unwrap();
}

/// Rounds to 16-bit integer samples.
pub fn quantize16(interleaved: &[f64]) -> Vec<i32> {
    interleaved
        .iter()
        .map(|&s| (s * 32768.0).round().clamp(-32768.0, 32767.0) as i32)
        .collect()
}

/// Writes 16-bit FLAC with flacenc's default 4096-frame blocks.
pub fn write_flac16(path: &Path, rate: u32, channels: usize, samples: &[i32]) {
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .expect("flac config");
    let source = flacenc::source::MemSource::from_samples(samples, channels, 16, rate as usize);
    let block_size = config.block_size;
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, block_size).expect("flac encode");
    // flacenc records a short last block as the stream's minimum block size; the FLAC spec
    // excludes the last block, and Symphonia (strictly) then takes the stream for a
    // variable-block-size one and rejects its frames. State the fixed size, as libFLAC does.
    stream
        .stream_info_mut()
        .set_block_sizes(block_size, block_size)
        .expect("block sizes");
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).expect("flac write");
    std::fs::write(path, sink.as_slice()).unwrap();
}

/// A sine as 16-bit stereo FLAC; returns the exact decoded values of its left channel.
pub fn sine_flac(path: &Path, freq: f64, rate: u32, start: usize, frames: usize, amplitude: f64) -> Vec<f32> {
    let samples = quantize16(&stereo(&sine(freq, rate, start, frames, amplitude)));
    write_flac16(path, rate, 2, &samples);
    samples.iter().step_by(2).map(|&s| s as f32 / 32768.0).collect()
}

/// A sine as 32-bit float stereo WAV.
pub fn sine_wav(path: &Path, freq: f64, rate: u32, frames: usize, amplitude: f64) {
    write_wav_f32(path, rate, 2, &stereo(&sine(freq, rate, 0, frames, amplitude)));
}

pub fn item(id: u64, path: &Path) -> PlayItem {
    PlayItem::new(id, path)
}

/// The result of an offline render.
#[derive(Debug)]
pub struct Render {
    pub rate: u32,
    /// Interleaved stereo.
    pub samples: Vec<f32>,
    pub events: Vec<EngineEvent>,
}

impl Render {
    pub fn left(&self) -> Vec<f32> {
        self.samples.iter().step_by(2).copied().collect()
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Ids of `TrackStarted` events, in order.
    pub fn started(&self) -> Vec<u64> {
        self.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::TrackStarted { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    pub fn errors(&self) -> Vec<Option<u64>> {
        self.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::Error { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }
}

pub fn read_wav(path: &Path) -> (u32, Vec<f32>) {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 2);
    let samples = reader.samples::<f32>().map(Result::unwrap).collect();
    (spec.sample_rate, samples)
}

/// Renders `items` offline into a WAV file (at `rate`, or the first track's rate) and reads it
/// back. `setup` runs after the queue is set and before `play`, `during` right after `play`.
pub fn render_with(
    dir: &Path,
    items: Vec<PlayItem>,
    rate: Option<u32>,
    setup: impl FnOnce(&Engine),
    during: impl FnOnce(&Engine, &Receiver<EngineEvent>),
) -> Render {
    let out = dir.join("render.wav");
    let engine = Engine::with_sink(Box::new(WavSink::new(&out, rate).unwrap())).unwrap();
    let events = engine.events();
    engine.set_queue(items, 0);
    setup(&engine);
    engine.play();
    during(&engine, &events);
    assert!(engine.wait_until_idle(IDLE_TIMEOUT), "render did not finish");
    drop(engine);
    let (rate, samples) = read_wav(&out);
    Render {
        rate,
        samples,
        events: events.try_iter().collect(),
    }
}

pub fn render(dir: &Path, items: Vec<PlayItem>, rate: Option<u32>, setup: impl FnOnce(&Engine)) -> Render {
    render_with(dir, items, rate, setup, |_, _| {})
}

pub fn temp_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

// ----- analysis --------------------------------------------------------------------------------

/// Frequency from upward zero crossings, interpolated between samples.
pub fn zero_crossing_frequency(x: &[f32], rate: u32) -> f64 {
    let mut crossings = Vec::new();
    for n in 1..x.len() {
        let (a, b) = (f64::from(x[n - 1]), f64::from(x[n]));
        if a < 0.0 && b >= 0.0 {
            crossings.push((n - 1) as f64 + a / (a - b));
        }
    }
    assert!(crossings.len() >= 2, "no tone found");
    let span = crossings[crossings.len() - 1] - crossings[0];
    (crossings.len() - 1) as f64 * f64::from(rate) / span
}

/// Amplitude of the `freq` component (Goertzel), for a signal of whole periods.
pub fn tone_amplitude(x: &[f32], freq: f64, rate: u32) -> f64 {
    let w = 2.0 * PI * freq / f64::from(rate);
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &v in x {
        let s0 = f64::from(v) + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
    2.0 * power.max(0.0).sqrt() / x.len() as f64
}

pub fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / x.len() as f64).sqrt()
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

pub fn db(ratio: f64) -> f64 {
    20.0 * ratio.log10()
}

/// Largest sample-to-sample step and where it is.
pub fn max_step(x: &[f32]) -> (f64, usize) {
    x.windows(2)
        .enumerate()
        .map(|(i, w)| ((f64::from(w[1]) - f64::from(w[0])).abs(), i))
        .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a })
}

/// The largest step a sine of this amplitude and frequency can make between two samples.
pub fn sine_step_bound(freq: f64, rate: u32, amplitude: f64) -> f64 {
    2.0 * amplitude * (PI * freq / f64::from(rate)).sin()
}

// ----- a sink with a fixed latency, for deterministic real-time behaviour -----------------------

/// What [`LatencySink`] has "played", shared with the test.
pub type Played = Arc<Mutex<Vec<f32>>>;

/// A stereo sink that keeps the last `latency` frames buffered like a device would, and can
/// stop the engine thread at a chosen point: once `gate_at` frames have been played it
/// reports on `reached` and blocks until `release` fires. Flush drops the buffered frames.
pub struct LatencySink {
    rate: u32,
    latency: usize,
    pending: VecDeque<f32>,
    played: Played,
    gate: Option<(usize, Sender<()>, Receiver<()>)>,
}

impl LatencySink {
    pub fn new(rate: u32, latency: usize) -> (Self, Played) {
        let played = Played::default();
        (
            Self {
                rate,
                latency,
                pending: VecDeque::new(),
                played: Arc::clone(&played),
                gate: None,
            },
            played,
        )
    }

    /// Stops the writer once `frames` have been played. Returns (reached, release).
    pub fn gate_at(&mut self, frames: usize) -> (Receiver<()>, Sender<()>) {
        let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        self.gate = Some((frames, reached_tx, release_rx));
        (reached_rx, release_tx)
    }
}

impl Sink for LatencySink {
    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn channels(&self) -> u16 {
        2
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        self.pending.extend(samples.iter().copied());
        let excess = self.pending.len().saturating_sub(self.latency * 2);
        let mut played = self.played.lock();
        played.extend(self.pending.drain(..excess));
        let played_frames = played.len() / 2;
        drop(played);
        if let Some((at, _, _)) = &self.gate
            && played_frames >= *at
        {
            let (_, reached, release) = self.gate.take().unwrap();
            reached.send(()).unwrap();
            release.recv().unwrap();
        }
        Ok(())
    }

    fn flush(&mut self) -> u64 {
        let frames = self.pending.len() / 2;
        self.pending.clear();
        frames as u64
    }

    fn latency_us(&self) -> u64 {
        (self.pending.len() / 2) as u64 * 1_000_000 / u64::from(self.rate)
    }

    fn drain(&mut self, _timeout: Duration) -> Result<bool, EngineError> {
        self.played.lock().extend(self.pending.drain(..));
        Ok(true)
    }
}

// ----- a gate around any sink, to stop the engine thread at an exact point --------------------

/// Forwards to `inner` and, once `at` frames have been written, reports on `reached` and blocks
/// the engine thread until `release` fires: a command sent meanwhile is handled at that exact
/// point of the stream, whatever the machine's speed.
pub struct Gated<S> {
    inner: S,
    written: usize,
    gate: Option<(usize, Sender<()>, Receiver<()>)>,
}

impl<S: Sink> Gated<S> {
    /// Returns the sink and (reached, release).
    pub fn new(inner: S, at: usize) -> (Self, Receiver<()>, Sender<()>) {
        let (reached_tx, reached_rx) = crossbeam_channel::bounded(1);
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let sink = Self {
            inner,
            written: 0,
            gate: Some((at, reached_tx, release_rx)),
        };
        (sink, reached_rx, release_tx)
    }
}

impl<S: Sink> Sink for Gated<S> {
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        self.inner.write(samples)?;
        self.written += samples.len() / usize::from(self.channels());
        if let Some((at, _, _)) = &self.gate
            && self.written >= *at
        {
            let (_, reached, release) = self.gate.take().unwrap();
            reached.send(()).unwrap();
            release.recv().unwrap();
        }
        Ok(())
    }

    fn flush(&mut self) -> u64 {
        self.inner.flush()
    }

    fn latency_us(&self) -> u64 {
        self.inner.latency_us()
    }

    fn prefer_sample_rate(&mut self, rate: u32) {
        self.inner.prefer_sample_rate(rate)
    }

    fn set_paused(&mut self, paused: bool) {
        self.inner.set_paused(paused)
    }

    fn drain(&mut self, timeout: Duration) -> Result<bool, EngineError> {
        self.inner.drain(timeout)
    }
}

/// Renders `items` into an offline WAV sink, holding the engine thread once `gate_at` frames
/// are written to run `at_gate` (which sends commands) before letting it continue.
pub fn render_gated(
    dir: &Path,
    items: Vec<PlayItem>,
    gate_at: usize,
    at_gate: impl FnOnce(&Engine),
) -> Render {
    let out = dir.join("render.wav");
    let (sink, reached, release) = Gated::new(WavSink::new(&out, None).unwrap(), gate_at);
    let engine = Engine::with_sink(Box::new(sink)).unwrap();
    let events = engine.events();
    engine.set_queue(items, 0);
    engine.play();
    reached
        .recv_timeout(Duration::from_secs(60))
        .expect("the gate was not reached");
    at_gate(&engine);
    release.send(()).unwrap();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT), "render did not finish");
    drop(engine);
    let (rate, samples) = read_wav(&out);
    Render {
        rate,
        samples,
        events: events.try_iter().collect(),
    }
}
