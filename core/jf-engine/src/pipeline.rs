//! The processing chain between the decoders and the sink.
//!
//! Decoded stereo at the source rate goes into a *lane*; stereo at the output rate comes out:
//! lane (resample when rates differ) → mixer (fades) → EQ → limiter (when engaged) → volume
//! (unless the sink applies it). ReplayGain is a per-track scalar the engine applies just
//! before `push`: a scalar gain commutes with resampling, and applying it there keeps a gain
//! change exactly on the track boundary.
//!
//! There are two lanes. Normally one is in use and the chain persists across tracks, which is
//! what makes joins gapless: a track that follows at the same rate continues the same
//! resampler run, and the EQ and limiter never notice the boundary. During a crossfade the
//! outgoing track keeps its lane, fading out, while the incoming one starts in the other lane,
//! fading in; the mixer adds them frame by frame on one timeline.
//!
//! Every stage is time-aligned (start-up delays are dropped, tails flushed), so the frame index
//! at which an input frame leaves the chain is known exactly: the "stream index". The engine
//! uses it to place track boundaries and fades and to compute positions.

use std::f64::consts::FRAC_PI_2;

use crate::dsp::{EqSettings, Equalizer, Limiter};
use crate::resample::{FftResampler, Resample};
use crate::types::EngineError;

const CHANNELS: usize = 2;
/// Limiter ceiling in dBFS.
pub(crate) const LIMITER_CEILING_DB: f32 = -1.0;

/// An equal-power fade over stream frames `[start, start + len)`: the incoming side follows
/// `sin`, the outgoing side `cos` of `π/2 · (s − start) / len`, so the summed power stays
/// constant. `start` may lie before the lane's first frame (a fade resumed mid-way).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fade {
    pub start: i64,
    pub len: u64,
}

impl Fade {
    pub(crate) fn end(&self) -> i64 {
        self.start
            .saturating_add(i64::try_from(self.len).unwrap_or(i64::MAX))
    }

    /// Progress through the fade at stream frame `s`, 0.0 to 1.0.
    fn phase(&self, s: u64) -> f64 {
        if self.len == 0 {
            return 1.0;
        }
        let into = i128::from(s) - i128::from(self.start);
        (into as f64 / self.len as f64).clamp(0.0, 1.0)
    }

    pub(crate) fn fade_in_gain(&self, s: u64) -> f32 {
        (FRAC_PI_2 * self.phase(s)).sin() as f32
    }

    pub(crate) fn fade_out_gain(&self, s: u64) -> f32 {
        (FRAC_PI_2 * self.phase(s)).cos() as f32
    }
}

/// One input path: a resampler run and the frames it produced that the mixer has not taken.
#[derive(Debug, Default)]
struct Lane {
    /// Rate of the current resampler run, `None` until the first input after a reset.
    in_rate: Option<u32>,
    resampler: Option<Box<dyn Resample>>,
    /// Stream index where the current run's output starts.
    run_start: u64,
    /// Input frames fed into the current run.
    run_in: u64,
    /// Produced frames not mixed yet, interleaved stereo; the first has index `ready_start`.
    ready: Vec<f32>,
    ready_start: u64,
    fade_in: Option<Fade>,
    /// Frames at or after the fade's end are dropped.
    fade_out: Option<Fade>,
    /// No more input comes: the lane contributes silence after its last frame.
    finished: bool,
}

impl Lane {
    fn reset(&mut self, stream_index: u64) {
        *self = Lane {
            run_start: stream_index,
            ready_start: stream_index,
            ready: std::mem::take(&mut self.ready),
            ..Lane::default()
        };
        self.ready.clear();
    }

    /// Stream index just past the last produced frame.
    fn ready_end(&self) -> u64 {
        self.ready_start + (self.ready.len() / CHANNELS) as u64
    }

    /// Stream index at which the next input frame will come out.
    fn next_input_index(&self, out_rate: u32) -> u64 {
        match self.in_rate {
            Some(rate) if rate != out_rate => {
                let (rate, out) = (u64::from(rate), u64::from(out_rate));
                self.run_start + (self.run_in * out + rate / 2) / rate
            }
            _ => self.run_start + self.run_in,
        }
    }

    /// The last stream index (exclusive) this lane can contribute to.
    fn cut(&self) -> u64 {
        self.fade_out
            .map_or(u64::MAX, |f| u64::try_from(f.end().max(0)).unwrap_or(0))
    }

    /// Up to which stream index the mixer can take frames: what the lane produced, or anything
    /// once it is finished or past its cut.
    fn available_until(&self) -> u64 {
        if self.finished || self.ready_end() >= self.cut() {
            u64::MAX
        } else {
            self.ready_end()
        }
    }

    fn finish_run(&mut self, out_rate: u32) -> Result<(), EngineError> {
        let mut result = Ok(());
        if let Some(mut resampler) = self.resampler.take() {
            result = resampler.finish(&mut self.ready);
        }
        let _ = out_rate;
        self.in_rate = None;
        self.run_start = self.ready_end();
        self.run_in = 0;
        result
    }

    /// Gain at stream index `s`.
    fn gain(&self, s: u64) -> f32 {
        let mut gain = 1.0;
        if let Some(fade) = self.fade_in {
            gain *= fade.fade_in_gain(s);
        }
        if let Some(fade) = self.fade_out {
            gain *= fade.fade_out_gain(s);
        }
        gain
    }

    fn has_fade_at(&self, from: u64, to: u64) -> bool {
        let overlaps = |f: &Fade| {
            let (start, end) = (f.start, f.end());
            i128::from(start) < i128::from(to) && i128::from(end) > i128::from(from)
        };
        self.fade_in.as_ref().is_some_and(overlaps) || self.fade_out.as_ref().is_some_and(overlaps)
    }
}

/// Which lane a decoder feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaneId {
    /// The lane new tracks go to (the incoming one during a crossfade).
    Main,
    /// The outgoing lane during a crossfade.
    Fading,
}

#[derive(Debug)]
pub(crate) struct Pipeline {
    out_rate: u32,
    lanes: [Lane; 2],
    /// Index of the main lane in `lanes`.
    main: usize,
    /// A crossfade is running: the other lane fades out.
    fading: bool,
    /// Stream index of the next frame the mixer emits.
    mix_pos: u64,
    /// The mixer emits nothing at or after this index (a crossfade being prepared).
    hold: Option<u64>,
    eq: Equalizer,
    limiter: Limiter,
    limiter_engaged: bool,
    volume: f32,
    volume_target: f32,
    volume_ramp: f32,
    mixed: Vec<f32>,
}

impl Pipeline {
    pub(crate) fn new(out_rate: u32, eq: &EqSettings) -> Self {
        let mut equalizer = Equalizer::new(out_rate, CHANNELS);
        equalizer.set_settings(eq);
        Self {
            out_rate,
            lanes: [Lane::default(), Lane::default()],
            main: 0,
            fading: false,
            mix_pos: 0,
            hold: None,
            eq: equalizer,
            limiter: Limiter::new(out_rate, CHANNELS, LIMITER_CEILING_DB),
            limiter_engaged: false,
            volume: 1.0,
            volume_target: 1.0,
            // One-pole smoothing, about 5 ms, so volume changes do not click.
            volume_ramp: 1.0 - (-1.0 / (0.005 * f64::from(out_rate.max(1)))).exp() as f32,
            mixed: Vec::new(),
        }
    }

    pub(crate) fn out_rate(&self) -> u32 {
        self.out_rate
    }

    fn lane_index(&self, lane: LaneId) -> usize {
        match lane {
            LaneId::Main => self.main,
            LaneId::Fading => 1 - self.main,
        }
    }

    fn lane(&self, lane: LaneId) -> &Lane {
        &self.lanes[self.lane_index(lane)]
    }

    fn lane_mut(&mut self, lane: LaneId) -> &mut Lane {
        let index = self.lane_index(lane);
        &mut self.lanes[index]
    }

    /// Rebuilds the chain for a new output rate. Only valid while nothing is in flight.
    pub(crate) fn set_out_rate(&mut self, out_rate: u32, stream_index: u64) {
        let eq = self.eq.settings().clone();
        let (volume, engaged) = (self.volume_target, self.limiter_engaged);
        *self = Self::new(out_rate, &eq);
        self.volume = volume;
        self.volume_target = volume;
        self.limiter_engaged = engaged;
        self.reset(stream_index);
    }

    /// Drops everything in flight (seek, skip, stop); the next output frame gets
    /// `stream_index`.
    pub(crate) fn reset(&mut self, stream_index: u64) {
        for lane in &mut self.lanes {
            lane.reset(stream_index);
        }
        self.main = 0;
        self.fading = false;
        self.hold = None;
        self.mix_pos = stream_index;
        self.eq.reset();
        self.limiter.reset();
        self.volume = self.volume_target;
    }

    /// Stream index of the next frame the mixer emits.
    pub(crate) fn mix_pos(&self) -> u64 {
        self.mix_pos
    }

    /// Stream index at which the next input frame of `lane` will come out.
    pub(crate) fn next_input_index(&self, lane: LaneId) -> u64 {
        self.lane(lane).next_input_index(self.out_rate)
    }

    /// Stream index just past what `lane` produced so far.
    pub(crate) fn produced_until(&self, lane: LaneId) -> u64 {
        self.lane(lane).ready_end()
    }

    /// Stream index where `frames` more input frames at `rate` would end in `lane`, assuming
    /// the lane's current run continues at that rate.
    pub(crate) fn index_after(&self, lane: LaneId, rate: u32, frames: u64) -> u64 {
        let lane = self.lane(lane);
        let run_in = if lane.in_rate == Some(rate) {
            lane.run_in
        } else {
            0
        };
        let start = if lane.in_rate == Some(rate) {
            lane.run_start
        } else {
            lane.next_input_index(self.out_rate)
        };
        if rate == self.out_rate {
            start + run_in + frames
        } else {
            let (rate, out) = (u64::from(rate), u64::from(self.out_rate));
            start + ((run_in + frames) * out + rate / 2) / rate
        }
    }

    /// Prepares `lane` for input at `rate`. When the rate changes, the lane's current
    /// resampler run is finished (its tail goes to `out` through the mixer) and a new one starts.
    pub(crate) fn prepare_input(
        &mut self,
        lane: LaneId,
        rate: u32,
        out: &mut Vec<f32>,
    ) -> Result<(), EngineError> {
        let out_rate = self.out_rate;
        let target = self.lane_mut(lane);
        if target.in_rate == Some(rate) {
            return Ok(());
        }
        let result = target.finish_run(out_rate);
        target.resampler = if rate == out_rate {
            None
        } else {
            Some(Box::new(FftResampler::new(rate, out_rate, CHANNELS)?))
        };
        target.in_rate = Some(rate);
        target.finished = false;
        self.mix(out);
        result
    }

    /// Processes interleaved stereo for `lane` at the rate given to
    /// [`prepare_input`](Self::prepare_input) and appends whatever the mixer can emit to `out`.
    pub(crate) fn push(
        &mut self,
        lane: LaneId,
        input: &[f32],
        out: &mut Vec<f32>,
    ) -> Result<(), EngineError> {
        let target = self.lane_mut(lane);
        let at = target.ready.len();
        match target.resampler.as_mut() {
            Some(resampler) => resampler.push(input, &mut target.ready)?,
            None => target.ready.extend_from_slice(input),
        }
        target.run_in += (input.len() / CHANNELS) as u64;
        // Frames past a fade-out's end are never heard.
        let cut = target.cut();
        if target.ready_end() > cut {
            let keep = usize::try_from(cut.saturating_sub(target.ready_start)).unwrap_or(0) * CHANNELS;
            target.ready.truncate(keep.max(at.min(keep)));
        }
        self.mix(out);
        Ok(())
    }

    /// No more input for `lane`: flushes its resampler and lets the mixer pass its end.
    pub(crate) fn finish_lane(&mut self, lane: LaneId, out: &mut Vec<f32>) -> Result<(), EngineError> {
        let out_rate = self.out_rate;
        let target = self.lane_mut(lane);
        let result = target.finish_run(out_rate);
        let cut = target.cut();
        if target.ready_end() > cut {
            let keep = usize::try_from(cut.saturating_sub(target.ready_start)).unwrap_or(0) * CHANNELS;
            target.ready.truncate(keep);
        }
        target.finished = true;
        self.mix(out);
        result
    }

    /// End of the stream: flushes every lane and the limiter tail into `out`.
    pub(crate) fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), EngineError> {
        self.hold = None;
        let mut result = Ok(());
        for lane in [LaneId::Fading, LaneId::Main] {
            if lane == LaneId::Fading && !self.fading {
                continue;
            }
            if let Err(err) = self.finish_lane(lane, out) {
                result = Err(err);
            }
        }
        if self.limiter_engaged {
            let start = out.len();
            self.limiter.flush(out);
            self.apply_volume(&mut out[start..]);
        }
        result
    }

    /// Holds the mixer at stream index `at` (or releases it with `None`).
    pub(crate) fn set_hold(&mut self, at: Option<u64>, out: &mut Vec<f32>) {
        self.hold = at;
        self.mix(out);
    }

    /// Starts a crossfade over `[start, start + len)`: the main lane becomes the fading lane
    /// and fades out, ending at `start + len`; a fresh main lane starts at `start`, fading in.
    pub(crate) fn begin_crossfade(&mut self, start: u64, len: u64) {
        debug_assert!(!self.fading);
        let fade = Fade {
            start: i64::try_from(start).unwrap_or(i64::MAX),
            len,
        };
        self.lanes[self.main].fade_out = Some(fade);
        self.main = 1 - self.main;
        let incoming = &mut self.lanes[self.main];
        incoming.reset(start);
        incoming.fade_in = Some(fade);
        self.fading = true;
        self.hold = None;
    }

    /// Whether a crossfade is running.
    pub(crate) fn crossfading(&self) -> bool {
        self.fading
    }

    /// Drops the incoming lane of a crossfade that has not been mixed yet; the outgoing lane
    /// becomes the main lane again, without its fade-out. Returns `false` (and does nothing)
    /// when some of the incoming lane was already emitted.
    pub(crate) fn cancel_incoming(&mut self) -> bool {
        if !self.fading {
            return true;
        }
        let incoming = &self.lanes[self.main];
        if self.mix_pos > incoming.run_start.min(incoming.ready_start) {
            return false;
        }
        self.lanes[self.main].reset(self.mix_pos);
        self.main = 1 - self.main;
        self.lanes[self.main].fade_out = None;
        self.lanes[self.main].finished = false;
        self.fading = false;
        true
    }

    /// The outgoing track ended at stream index `end`, before the incoming lane's start: move
    /// the incoming lane back to `end` and drop both fades, so the tracks join gaplessly.
    /// Only valid while nothing of the incoming lane was emitted.
    pub(crate) fn join_incoming_at(&mut self, end: u64) {
        if !self.fading {
            return;
        }
        let incoming = &mut self.lanes[self.main];
        let shift = incoming.ready_start.saturating_sub(end);
        incoming.ready_start -= shift;
        incoming.run_start -= shift.min(incoming.run_start);
        incoming.fade_in = None;
        let outgoing = 1 - self.main;
        self.lanes[outgoing].fade_out = None;
    }

    /// Fades the main lane in over `[start, start + len)` (after a manual transition, so the
    /// new audio does not start mid-wave).
    pub(crate) fn fade_in_main(&mut self, fade: Fade) {
        self.lanes[self.main].fade_in = Some(fade);
    }

    pub(crate) fn set_eq(&mut self, settings: &EqSettings) {
        self.eq.set_settings(settings);
    }

    /// Engages or bypasses the limiter. Bypassing releases the frames it holds into `out`, so
    /// the stream stays continuous either way.
    pub(crate) fn set_limiter_engaged(&mut self, engaged: bool, out: &mut Vec<f32>) {
        if engaged == self.limiter_engaged {
            return;
        }
        if engaged {
            self.limiter.reset();
        } else {
            let start = out.len();
            self.limiter.flush(out);
            self.apply_volume(&mut out[start..]);
        }
        self.limiter_engaged = engaged;
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume_target = volume;
    }

    /// Emits every frame all active lanes can provide, mixed, through EQ, limiter and volume.
    fn mix(&mut self, out: &mut Vec<f32>) {
        loop {
            let main = &self.lanes[self.main];
            let mut limit = main.available_until();
            if self.fading {
                limit = limit.min(self.lanes[1 - self.main].available_until());
            }
            if let Some(hold) = self.hold {
                limit = limit.min(hold);
            }
            // Nothing past the produced frames, even when every lane is finished.
            let produced = if self.fading {
                main.ready_end().max(self.lanes[1 - self.main].ready_end())
            } else {
                main.ready_end()
            };
            let limit = limit.min(produced);
            if limit <= self.mix_pos {
                break;
            }
            let frames = (limit - self.mix_pos) as usize;
            self.mixed.clear();
            self.mixed.resize(frames * CHANNELS, 0.0);
            let from = self.mix_pos;
            let lanes = if self.fading { 2 } else { 1 };
            for k in 0..lanes {
                let index = if k == 0 { self.main } else { 1 - self.main };
                let lane = &mut self.lanes[index];
                Self::take_from_lane(lane, from, frames, &mut self.mixed);
            }
            self.mix_pos = limit;
            self.process_stage(out);
            // A finished outgoing lane past its cut is done.
            if self.fading {
                let fading = &self.lanes[1 - self.main];
                if self.mix_pos >= fading.cut() || (fading.finished && self.mix_pos >= fading.ready_end()) {
                    let index = 1 - self.main;
                    self.lanes[index].reset(self.mix_pos);
                    self.lanes[index].finished = true;
                    self.fading = false;
                }
            }
        }
    }

    /// Adds `lane`'s frames for stream indices `[from, from + frames)` into `mixed`, applying
    /// its fades, and drops them from the lane.
    fn take_from_lane(lane: &mut Lane, from: u64, frames: usize, mixed: &mut [f32]) {
        let to = from + frames as u64;
        // Frames before `from` are already mixed (or belong to nothing): drop them.
        if lane.ready_start < from {
            let stale = ((from - lane.ready_start) as usize * CHANNELS).min(lane.ready.len());
            lane.ready.drain(..stale);
            lane.ready_start = from;
        }
        let begin = lane.ready_start.max(from);
        let end = lane.ready_end().min(to).min(lane.cut());
        if end <= begin {
            return;
        }
        let offset = (begin - from) as usize;
        let count = (end - begin) as usize;
        let src = &lane.ready[..count * CHANNELS];
        let dst = &mut mixed[offset * CHANNELS..(offset + count) * CHANNELS];
        if lane.has_fade_at(begin, end) {
            for (i, (d, s)) in dst
                .chunks_exact_mut(CHANNELS)
                .zip(src.chunks_exact(CHANNELS))
                .enumerate()
            {
                let gain = lane.gain(begin + i as u64);
                d[0] += s[0] * gain;
                d[1] += s[1] * gain;
            }
        } else {
            for (d, s) in dst.iter_mut().zip(src) {
                *d += *s;
            }
        }
        // Consume what was mixed; frames past `to` stay for later.
        let consumed =
            ((to.min(lane.ready_end()) - lane.ready_start) as usize * CHANNELS).min(lane.ready.len());
        lane.ready.drain(..consumed);
        lane.ready_start = to
            .min(lane.ready_start + (consumed / CHANNELS) as u64)
            .max(lane.ready_start);
        if lane.ready_start < to && lane.ready.is_empty() {
            lane.ready_start = lane.ready_start.max(to.min(lane.ready_start + consumed as u64));
        }
        // Past a fade-in the fade is no longer needed.
        if lane
            .fade_in
            .is_some_and(|f| i128::from(f.end()) <= i128::from(lane.ready_start))
        {
            lane.fade_in = None;
        }
    }

    /// EQ, limiter and volume over `mixed`, appending to `out`.
    fn process_stage(&mut self, out: &mut Vec<f32>) {
        self.eq.process(&mut self.mixed);
        let start = out.len();
        if self.limiter_engaged {
            self.limiter.process(&self.mixed, out);
        } else {
            out.extend_from_slice(&self.mixed);
        }
        self.apply_volume(&mut out[start..]);
    }

    fn apply_volume(&mut self, samples: &mut [f32]) {
        if self.volume == 1.0 && self.volume_target == 1.0 {
            return;
        }
        for frame in samples.chunks_exact_mut(CHANNELS) {
            if self.volume != self.volume_target {
                self.volume += (self.volume_target - self.volume) * self.volume_ramp;
                if (self.volume - self.volume_target).abs() < 1e-6 {
                    self.volume = self.volume_target;
                }
            }
            for sample in frame {
                *sample *= self.volume;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| [n as f32 / 1e6, -(n as f32) / 1e6])
            .collect()
    }

    #[test]
    fn same_rate_is_bit_transparent_and_aligned() {
        let mut p = Pipeline::new(44_100, &EqSettings::default());
        let mut out = Vec::new();
        p.prepare_input(LaneId::Main, 44_100, &mut out).unwrap();
        let input = ramp(1000);
        p.push(LaneId::Main, &input, &mut out).unwrap();
        assert_eq!(p.next_input_index(LaneId::Main), 1000);
        p.finish(&mut out).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn resampled_runs_keep_exact_lengths_across_rate_changes() {
        let mut p = Pipeline::new(44_100, &EqSettings::default());
        let mut out = Vec::new();
        p.prepare_input(LaneId::Main, 48_000, &mut out).unwrap();
        p.push(LaneId::Main, &ramp(4_800), &mut out).unwrap();
        // 4 800 frames at 48 kHz are 4 410 at 44.1 kHz.
        assert_eq!(p.next_input_index(LaneId::Main), 4_410);
        p.prepare_input(LaneId::Main, 44_100, &mut out).unwrap();
        assert_eq!(out.len(), 4_410 * 2);
        assert_eq!(p.next_input_index(LaneId::Main), 4_410);
        p.push(LaneId::Main, &ramp(100), &mut out).unwrap();
        p.finish(&mut out).unwrap();
        assert_eq!(out.len(), 4_510 * 2);
    }

    #[test]
    fn engaging_the_limiter_keeps_the_stream_continuous() {
        let mut p = Pipeline::new(48_000, &EqSettings::default());
        let mut out = Vec::new();
        p.prepare_input(LaneId::Main, 48_000, &mut out).unwrap();
        let input = ramp(3_000);
        p.push(LaneId::Main, &input[..2_000], &mut out).unwrap();
        p.set_limiter_engaged(true, &mut out);
        p.push(LaneId::Main, &input[2_000..4_000], &mut out).unwrap();
        p.set_limiter_engaged(false, &mut out);
        p.push(LaneId::Main, &input[4_000..], &mut out).unwrap();
        p.finish(&mut out).unwrap();
        // The ramp stays far below the ceiling, so the limiter is transparent.
        assert_eq!(out, input);
    }

    fn constant(frames: usize, left: f32, right: f32) -> Vec<f32> {
        (0..frames).flat_map(|_| [left, right]).collect()
    }

    #[test]
    fn a_crossfade_mixes_two_lanes_with_equal_power() {
        let mut p = Pipeline::new(1_000, &EqSettings::default());
        let mut out = Vec::new();
        p.prepare_input(LaneId::Main, 1_000, &mut out).unwrap();
        // A: left channel only, 300 frames; the fade covers its last 100.
        p.set_hold(Some(200), &mut out);
        p.push(LaneId::Main, &constant(300, 1.0, 0.0), &mut out).unwrap();
        assert_eq!(out.len(), 200 * 2, "held at the fade start");
        p.begin_crossfade(200, 100);
        p.prepare_input(LaneId::Main, 1_000, &mut out).unwrap();
        p.finish_lane(LaneId::Fading, &mut out).unwrap();
        // B: right channel only.
        p.push(LaneId::Main, &constant(150, 0.0, 1.0), &mut out).unwrap();
        p.finish(&mut out).unwrap();
        assert_eq!(out.len(), 350 * 2, "200 + 100 overlapped + 50");
        for s in 200..300 {
            let (a, b) = (f64::from(out[2 * s]), f64::from(out[2 * s + 1]));
            let phase = (s - 200) as f64 / 100.0;
            assert!((a - (FRAC_PI_2 * phase).cos()).abs() < 1e-6, "{s}: {a}");
            assert!((b - (FRAC_PI_2 * phase).sin()).abs() < 1e-6, "{s}: {b}");
            assert!((a * a + b * b - 1.0).abs() < 1e-5);
        }
        assert!(!p.crossfading());
    }

    #[test]
    fn an_outgoing_lane_that_ends_early_lets_the_incoming_one_join() {
        let mut p = Pipeline::new(1_000, &EqSettings::default());
        let mut out = Vec::new();
        p.prepare_input(LaneId::Main, 1_000, &mut out).unwrap();
        p.set_hold(Some(200), &mut out);
        p.push(LaneId::Main, &constant(150, 1.0, 0.0), &mut out).unwrap();
        p.begin_crossfade(200, 100);
        p.prepare_input(LaneId::Main, 1_000, &mut out).unwrap();
        p.push(LaneId::Main, &constant(100, 0.0, 1.0), &mut out).unwrap();
        // A ended at 150, before the fade: B moves back to 150 and plays unfaded.
        p.join_incoming_at(150);
        p.finish_lane(LaneId::Fading, &mut out).unwrap();
        p.finish(&mut out).unwrap();
        assert_eq!(out.len(), 250 * 2);
        assert!(out[..300].chunks(2).all(|f| f == [1.0, 0.0]));
        assert!(out[300..].chunks(2).all(|f| f == [0.0, 1.0]));
    }
}
