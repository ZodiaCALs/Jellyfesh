//! Sample-rate conversion. rubato sits behind the [`Resample`] trait so its API churn stays in
//! this file (ADR-0009).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::types::EngineError;

/// A streaming resampler for interleaved `f32`.
///
/// Output is aligned with input: the converter's start-up delay is dropped, so input frame
/// `n` comes out as output frame `round(n × out_rate / in_rate)`, and [`finish`](Self::finish)
/// releases the tail so a stream of `n` frames becomes exactly that many output frames.
pub(crate) trait Resample: Send + std::fmt::Debug {
    /// Converts `input` and appends whatever output is ready to `out`. State carries over
    /// between calls.
    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), EngineError>;

    /// Flushes the remaining input and the converter's tail into `out` and starts over.
    fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), EngineError>;
}

/// Requested chunk size; rubato rounds it to fit the ratio exactly.
const CHUNK_FRAMES: usize = 1024;

/// rubato's synchronous FFT resampler: fixed ratio, high quality.
pub(crate) struct FftResampler {
    inner: Fft<f32>,
    channels: usize,
    in_rate: u64,
    out_rate: u64,
    chunk_in: usize,
    in_buf: Vec<f32>,
    in_fill: usize,
    out_buf: Vec<f32>,
    /// Start-up delay frames still to drop.
    to_trim: usize,
    total_in: u64,
    total_out: u64,
}

impl std::fmt::Debug for FftResampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FftResampler")
            .field("in_rate", &self.in_rate)
            .field("out_rate", &self.out_rate)
            .field("chunk_in", &self.chunk_in)
            .finish_non_exhaustive()
    }
}

impl FftResampler {
    pub(crate) fn new(in_rate: u32, out_rate: u32, channels: usize) -> Result<Self, EngineError> {
        let inner = Fft::<f32>::new(
            in_rate as usize,
            out_rate as usize,
            CHUNK_FRAMES,
            channels,
            FixedSync::Both,
        )
        .map_err(|e| EngineError::Resample(format!("{in_rate} Hz -> {out_rate} Hz: {e}")))?;
        let chunk_in = inner.input_frames_next();
        let out_max = inner.output_frames_max();
        let to_trim = inner.output_delay();
        Ok(Self {
            inner,
            channels,
            in_rate: u64::from(in_rate),
            out_rate: u64::from(out_rate),
            chunk_in,
            in_buf: vec![0.0; chunk_in * channels],
            in_fill: 0,
            out_buf: vec![0.0; out_max * channels],
            to_trim,
            total_in: 0,
            total_out: 0,
        })
    }

    /// Runs one chunk; `partial` is the number of valid frames when the chunk is not full.
    fn process_chunk(&mut self, partial: Option<usize>, out: &mut Vec<f32>) -> Result<(), EngineError> {
        let channels = self.channels;
        let input = InterleavedSlice::new(&self.in_buf[..], channels, self.chunk_in)
            .map_err(|e| EngineError::Resample(e.to_string()))?;
        let out_frames = self.out_buf.len() / channels;
        let mut output = InterleavedSlice::new_mut(&mut self.out_buf[..], channels, out_frames)
            .map_err(|e| EngineError::Resample(e.to_string()))?;
        let indexing = partial.map(|n| Indexing::new().partial_len(n));
        let (_, produced) = self
            .inner
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|e| EngineError::Resample(e.to_string()))?;
        self.in_fill = 0;
        let skip = self.to_trim.min(produced);
        self.to_trim -= skip;
        out.extend_from_slice(&self.out_buf[skip * channels..produced * channels]);
        self.total_out += (produced - skip) as u64;
        Ok(())
    }
}

impl Resample for FftResampler {
    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), EngineError> {
        let channels = self.channels;
        let mut offset = 0;
        while offset < input.len() {
            let space = (self.chunk_in - self.in_fill) * channels;
            let take = space.min(input.len() - offset);
            let at = self.in_fill * channels;
            self.in_buf[at..at + take].copy_from_slice(&input[offset..offset + take]);
            self.in_fill += take / channels;
            offset += take;
            if self.in_fill == self.chunk_in {
                self.process_chunk(None, out)?;
            }
        }
        self.total_in += (input.len() / channels) as u64;
        Ok(())
    }

    fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), EngineError> {
        let expected = (self.total_in * self.out_rate + self.in_rate / 2) / self.in_rate;
        let start = out.len();
        if self.in_fill > 0 {
            let fill = self.in_fill;
            self.process_chunk(Some(fill), out)?;
        }
        // Pump silence through until the tail is out.
        while self.total_out < expected {
            self.process_chunk(Some(0), out)?;
        }
        let excess = (self.total_out - expected) as usize * self.channels;
        out.truncate(out.len().max(start + excess) - excess);
        self.inner.reset();
        self.in_fill = 0;
        self.to_trim = self.inner.output_delay();
        self.total_in = 0;
        self.total_out = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, freq: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let s = 0.5 * (2.0 * std::f32::consts::PI * freq * n as f32 / rate as f32).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn output_length_matches_the_ratio() {
        for (from, to, frames) in [
            (48_000, 44_100, 48_000),
            (44_100, 48_000, 12_345),
            (22_050, 48_000, 999),
        ] {
            let mut rs = FftResampler::new(from, to, 2).unwrap();
            let input = sine(from, 440.0, frames);
            let mut out = Vec::new();
            for chunk in input.chunks(2 * 700) {
                rs.push(chunk, &mut out).unwrap();
            }
            rs.finish(&mut out).unwrap();
            let expected = (frames as u64 * u64::from(to) + u64::from(from) / 2) / u64::from(from);
            assert_eq!(out.len() as u64, expected * 2, "{from} -> {to}");
        }
    }

    #[test]
    fn output_is_aligned_with_the_input() {
        // A resampled sine must line up with the same sine generated at the output rate.
        // rubato reports its delay in whole frames, so a sub-sample offset (up to about half
        // an input frame) remains.
        for (from, to) in [
            (48_000u32, 44_100u32),
            (44_100, 48_000),
            (22_050, 48_000),
            (96_000, 44_100),
        ] {
            let mut rs = FftResampler::new(from, to, 2).unwrap();
            let mut out = Vec::new();
            rs.push(&sine(from, 1_000.0, from as usize / 2), &mut out)
                .unwrap();
            rs.finish(&mut out).unwrap();
            let best = (-40..=40)
                .map(|k| {
                    let offset = f64::from(k) * 0.05;
                    let worst = (2_000..out.len() / 2 - 2_000).fold(0.0f64, |m, n| {
                        let t = (n as f64 + offset) / f64::from(to);
                        let reference = 0.5 * (2.0 * std::f64::consts::PI * 1_000.0 * t).sin();
                        m.max((f64::from(out[2 * n]) - reference).abs())
                    });
                    (worst, offset)
                })
                .fold((f64::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
            let tolerance = 0.75 * f64::from(to) / f64::from(from);
            assert!(
                best.1.abs() <= tolerance,
                "{from} -> {to}: offset {} frames",
                best.1
            );
            assert!(best.0 < 0.01, "{from} -> {to}: residual {}", best.0);
        }
    }
}
