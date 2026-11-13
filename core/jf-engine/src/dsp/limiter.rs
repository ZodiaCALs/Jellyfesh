//! A look-ahead peak limiter.

/// Look-ahead (and attack) time.
const LOOKAHEAD_SECS: f64 = 0.0015;
/// Release time constant.
const RELEASE_SECS: f64 = 0.08;

/// A look-ahead brick-wall limiter with a fixed ceiling.
///
/// For every frame it computes the gain that keeps the frame's peak (over all channels, so
/// the stereo image stays put) at or below the ceiling, takes the minimum of that gain over the
/// look-ahead window, lets it recover with a release time constant and smooths it with a moving
/// average as long as the window. The average of window minimums can never exceed the
/// gain the peak needs, so the output stays under the ceiling without clipping, and the gain
/// ramps down over the look-ahead time instead of jumping. Below the ceiling the gain is
/// exactly 1.0, so the limiter is bit-transparent there.
///
/// The output is delayed by [`latency_frames`](Self::latency_frames); the first frames after
/// a reset prime the delay line, and [`flush`](Self::flush) releases what it still holds, so
/// a stream keeps its length.
#[derive(Debug, Clone)]
pub struct Limiter {
    channels: usize,
    ceiling: f32,
    window: usize,
    release: f64,
    /// Last `window` input frames.
    delay: Vec<f32>,
    /// Gain each of those frames needs.
    needed: Vec<f32>,
    pos: usize,
    filled: usize,
    /// Released envelope.
    envelope: f64,
    /// Moving average over the last `window` envelope values.
    average: Vec<f64>,
    average_pos: usize,
    average_sum: f64,
    primed: bool,
}

impl Limiter {
    /// A limiter for `channels` interleaved channels at `sample_rate` with its ceiling at
    /// `ceiling_db` dBFS.
    pub fn new(sample_rate: u32, channels: usize, ceiling_db: f32) -> Self {
        let channels = channels.max(1);
        let rate = f64::from(sample_rate.max(1));
        let window = ((LOOKAHEAD_SECS * rate).round() as usize).max(1);
        Self {
            channels,
            ceiling: super::db_to_linear(ceiling_db.min(0.0)),
            window,
            release: 1.0 - (-1.0 / (RELEASE_SECS * rate)).exp(),
            delay: vec![0.0; window * channels],
            needed: vec![1.0; window],
            pos: 0,
            filled: 0,
            envelope: 1.0,
            average: vec![1.0; window],
            average_pos: 0,
            average_sum: window as f64,
            primed: false,
        }
    }

    /// The ceiling as a linear amplitude.
    pub fn ceiling(&self) -> f32 {
        self.ceiling
    }

    /// Frames between a frame going in and coming out.
    pub fn latency_frames(&self) -> usize {
        self.window - 1
    }

    /// Frames held back in the delay line right now.
    pub fn held_frames(&self) -> usize {
        self.filled
    }

    /// Forgets all state: the next frames prime the delay line again.
    pub fn reset(&mut self) {
        self.delay.fill(0.0);
        self.needed.fill(1.0);
        self.pos = 0;
        self.filled = 0;
        self.envelope = 1.0;
        self.average.fill(1.0);
        self.average_pos = 0;
        self.average_sum = self.window as f64;
        self.primed = false;
    }

    /// Limits interleaved `input` and appends the frames that leave the delay line to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for frame in input.chunks_exact(self.channels) {
            self.push_frame(frame, out);
        }
    }

    /// Releases the frames still held in the delay line.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        let held = self.filled;
        let silence = [0.0f32; 16];
        let before = out.len();
        for _ in 0..self.window - 1 {
            if self.channels <= silence.len() {
                self.push_frame(&silence[..self.channels], out);
            } else {
                self.push_frame(&vec![0.0; self.channels], out);
            }
        }
        // Only the held frames are real; the silence pushed in behind them stays inside.
        out.truncate(before + held * self.channels);
        self.reset();
    }

    fn push_frame(&mut self, frame: &[f32], out: &mut Vec<f32>) {
        let channels = self.channels;
        let peak = frame.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let needed = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        self.delay[self.pos * channels..(self.pos + 1) * channels].copy_from_slice(frame);
        self.needed[self.pos] = needed;
        self.pos = (self.pos + 1) % self.window;
        if self.filled < self.window {
            self.filled += 1;
            if self.filled < self.window {
                return;
            }
        }
        // The window is full: the oldest frame (at `pos`) leaves now. Its gain must not exceed
        // what any frame in the window needs.
        let window_min = f64::from(self.needed.iter().fold(1.0f32, |m, &g| m.min(g)));
        if !self.primed {
            self.primed = true;
            self.envelope = window_min;
            self.average.fill(window_min);
            self.average_sum = window_min * self.window as f64;
        } else if window_min < self.envelope {
            self.envelope = window_min;
        } else {
            self.envelope += (window_min - self.envelope) * self.release;
        }
        self.average_sum += self.envelope - self.average[self.average_pos];
        self.average[self.average_pos] = self.envelope;
        self.average_pos = (self.average_pos + 1) % self.window;
        let gain = (self.average_sum / self.window as f64).min(1.0) as f32;

        let oldest = &self.delay[self.pos * channels..(self.pos + 1) * channels];
        let ceiling = self.ceiling;
        if gain >= 1.0 {
            out.extend_from_slice(oldest);
        } else {
            // The clamp only catches rounding in the running average.
            out.extend(oldest.iter().map(|s| (s * gain).clamp(-ceiling, ceiling)));
        }
        // `filled` stays at `window - 1` frames held once primed.
        self.filled = self.window - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .flat_map(|n| {
                let s = amplitude * (2.0 * std::f32::consts::PI * 997.0 * n as f32 / 48_000.0).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn keeps_the_stream_length() {
        let mut limiter = Limiter::new(48_000, 2, -1.0);
        let input = sine(10_000, 2.0);
        let mut out = Vec::new();
        for chunk in input.chunks(2 * 333) {
            limiter.process(chunk, &mut out);
        }
        limiter.flush(&mut out);
        assert_eq!(out.len(), input.len());
    }

    #[test]
    fn output_never_exceeds_the_ceiling() {
        let mut limiter = Limiter::new(48_000, 2, -1.0);
        let input = sine(48_000, 4.0);
        let mut out = Vec::new();
        limiter.process(&input, &mut out);
        limiter.flush(&mut out);
        let ceiling = limiter.ceiling();
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= ceiling, "{peak} > {ceiling}");
        assert!(peak > ceiling * 0.95, "limiting too hard: {peak}");
    }

    #[test]
    fn transparent_below_the_ceiling() {
        let mut limiter = Limiter::new(48_000, 2, -1.0);
        let input = sine(5_000, 0.8);
        let mut out = Vec::new();
        limiter.process(&input, &mut out);
        limiter.flush(&mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn a_short_input_comes_out_on_flush() {
        let mut limiter = Limiter::new(48_000, 2, -1.0);
        let mut out = Vec::new();
        limiter.process(&[0.5, -0.5, 0.25, -0.25], &mut out);
        assert!(out.is_empty());
        limiter.flush(&mut out);
        assert_eq!(out, [0.5, -0.5, 0.25, -0.25]);
    }
}
