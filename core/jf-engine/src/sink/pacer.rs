//! A virtual device clock for sinks that emulate real-time playback.

use std::time::{Duration, Instant};

/// Tracks how much of the written audio a virtual device has "played", and blocks writers so
/// that no more than `buffer_frames` are queued, like a device with that buffer would.
#[derive(Debug)]
pub(crate) struct Pacer {
    rate: u32,
    buffer_ms: u32,
    buffer_frames: u64,
    written: u64,
    /// Frames played when `clock_start` was taken.
    played_base: u64,
    /// Running clock, or `None` while paused or idle.
    clock_start: Option<Instant>,
    paused: bool,
}

impl Pacer {
    pub(crate) fn new(rate: u32, buffer_ms: u32) -> Self {
        let mut pacer = Self {
            rate: 1,
            buffer_ms,
            buffer_frames: 0,
            written: 0,
            played_base: 0,
            clock_start: None,
            paused: false,
        };
        pacer.set_rate(rate);
        pacer
    }

    /// Changes the clock rate; only meaningful before anything was written.
    pub(crate) fn set_rate(&mut self, rate: u32) {
        self.rate = rate.max(1);
        self.buffer_frames = (u64::from(self.rate) * u64::from(self.buffer_ms.max(1)) / 1000).max(1);
    }

    /// Frames the virtual device has played so far.
    pub(crate) fn played(&self) -> u64 {
        match self.clock_start {
            Some(start) => {
                let elapsed = start.elapsed().as_nanos();
                let frames = (elapsed * u128::from(self.rate) / 1_000_000_000) as u64;
                (self.played_base + frames).min(self.written)
            }
            None => self.played_base,
        }
    }

    /// Frames written but not yet played.
    pub(crate) fn pending(&self) -> u64 {
        self.written - self.played()
    }

    /// Accounts for `frames` new frames and blocks until the queue is within the buffer.
    pub(crate) fn write(&mut self, frames: u64) {
        let played = self.played();
        if played >= self.written {
            // The virtual device ran dry (or never started): restart its clock now.
            self.played_base = played;
            self.clock_start = None;
        }
        self.written += frames;
        if !self.paused && self.clock_start.is_none() {
            self.clock_start = Some(Instant::now());
        }
        if self.paused {
            return;
        }
        loop {
            let pending = self.pending();
            if pending <= self.buffer_frames {
                break;
            }
            std::thread::sleep(self.frames_to_duration(pending - self.buffer_frames));
        }
    }

    /// Drops everything not yet played; returns the number of frames dropped.
    pub(crate) fn flush(&mut self) -> u64 {
        self.flush_keep(0).0
    }

    /// Drops what is not played yet except the next `keep` frames, which play on. Returns the
    /// frames dropped and the frames kept.
    pub(crate) fn flush_keep(&mut self, keep: u64) -> (u64, u64) {
        let played = self.played();
        let pending = self.written - played;
        let kept = keep.min(pending);
        let dropped = pending - kept;
        self.written -= dropped;
        if kept == 0 {
            self.played_base = self.written;
            self.clock_start = None;
        }
        (dropped, kept)
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        if paused {
            self.played_base = self.played();
            self.clock_start = None;
        } else if self.played_base < self.written {
            self.clock_start = Some(Instant::now());
        }
        self.paused = paused;
    }

    /// Sleeps until everything written has played or `timeout` passes; returns whether all of
    /// it has played.
    pub(crate) fn drain(&mut self, timeout: Duration) -> bool {
        let pending = self.pending();
        if pending == 0 {
            return true;
        }
        if self.paused {
            return false;
        }
        std::thread::sleep(self.frames_to_duration(pending).min(timeout));
        self.pending() == 0
    }

    pub(crate) fn latency_frames(&self) -> u64 {
        self.pending()
    }

    fn frames_to_duration(&self, frames: u64) -> Duration {
        let nanos = u128::from(frames) * 1_000_000_000 / u128::from(self.rate);
        // Never spin with zero-length sleeps.
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX)).max(Duration::from_micros(200))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_block_once_the_buffer_is_full() {
        let mut pacer = Pacer::new(1000, 50); // 50 frames of buffer at 1 kHz
        let start = Instant::now();
        pacer.write(50); // fits the buffer: returns at once
        assert!(start.elapsed() < Duration::from_millis(80));
        pacer.write(100); // 150 queued: has to wait about 100 ms
        let waited = start.elapsed();
        assert!(waited >= Duration::from_millis(90), "{waited:?}");
        assert!(pacer.pending() <= 50);
    }

    #[test]
    fn flush_drops_the_unplayed_part() {
        let mut pacer = Pacer::new(1000, 10_000);
        pacer.write(5_000); // five seconds at 1 kHz, all buffered
        let dropped = pacer.flush();
        assert!(dropped > 4_000 && dropped <= 5_000, "{dropped}");
        assert_eq!(pacer.pending(), 0);
    }

    #[test]
    fn a_kept_flush_plays_the_kept_frames() {
        let mut pacer = Pacer::new(1000, 10_000);
        pacer.write(5_000);
        let (dropped, kept) = pacer.flush_keep(100);
        assert_eq!(kept, 100);
        assert!(dropped > 3_900 && dropped <= 4_900, "{dropped}");
        assert!(pacer.pending() <= 100);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(pacer.pending(), 0);
    }

    #[test]
    fn paused_clock_stands_still() {
        let mut pacer = Pacer::new(1000, 1000);
        pacer.write(300);
        pacer.set_paused(true);
        let before = pacer.pending();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(pacer.pending(), before);
        pacer.set_paused(false);
        std::thread::sleep(Duration::from_millis(30));
        assert!(pacer.pending() < before);
    }
}
