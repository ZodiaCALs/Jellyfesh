// TimelineView schedules that wake a view only when what it shows changes (docs/architecture.md,
// "Drawing time without waste"): the next second of an m:ss label, the next step of a bar, the
// next lyric line or word. Paused, or with the window out of sight, they wake once and then sleep;
// a new anchor gives a new schedule (views read `PlaybackStore.scheduleStart()`, which depends on
// the anchor version).

import SwiftUI

/// Where playback is when a schedule starts, and how fast it moves.
struct PlaybackSnapshotForSchedule: Equatable {
    /// The drawn position (DisplayClock) when the schedule starts.
    var positionUs: Int64
    /// Where the anchor alone puts playback at that moment. After a small correction the drawn
    /// position slews from `positionUs` towards it, so a change is due by the later of the two
    /// estimates and a wake-up never lands before it.
    var anchorPositionUs: Int64
    var running: Bool
    /// Playback speed while running (the anchor's rate, 1 in practice).
    var rate: Double = 1
    var durationUs: Int64?
}

/// Wakes at the next whole second and the next `stepUs` of the position.
struct ProgressSchedule: TimelineSchedule {
    var start: PlaybackSnapshotForSchedule
    /// Steps of a bar, in microseconds of playback per drawn step; nil for no bar.
    var stepUs: Int64?
    /// Whether an m:ss label shows.
    var seconds: Bool
    /// False while the window cannot be seen: draw once, then sleep.
    var active: Bool

    func entries(from startDate: Date, mode: TimelineScheduleMode) -> Entries {
        var step = stepUs
        if mode == .lowFrequency, let s = step {
            step = max(s, 1_000_000)
        }
        return Entries(startDate: startDate, start: start, stepUs: step, seconds: seconds,
                       running: start.running && active)
    }

    struct Entries: Sequence, IteratorProtocol {
        let startDate: Date
        let start: PlaybackSnapshotForSchedule
        let stepUs: Int64?
        let seconds: Bool
        let running: Bool
        private var last: Int64?

        init(startDate: Date, start: PlaybackSnapshotForSchedule, stepUs: Int64?, seconds: Bool, running: Bool) {
            self.startDate = startDate
            self.start = start
            self.stepUs = stepUs
            self.seconds = seconds
            self.running = running
        }

        mutating func next() -> Date? {
            guard let previous = last else {
                last = start.positionUs
                return startDate
            }
            guard running else { return nil }
            if let duration = start.durationUs, previous >= duration {
                return nil
            }
            var target: Int64?
            if seconds {
                target = nextSecondUs(after: previous)
            }
            if let step = stepUs, let next = nextStepUs(after: previous, step: step) {
                target = min(target ?? next, next)
            }
            guard let due = target else { return nil }
            last = due
            return ScheduleClock.date(for: due, start: start, at: startDate)
        }
    }
}

/// Wakes when the active lyric line or word changes or a word finishes (`nextChange`).
struct LyricsSchedule: TimelineSchedule {
    var timeline: LyricsTimeline
    var start: PlaybackSnapshotForSchedule
    var active: Bool

    func entries(from startDate: Date, mode: TimelineScheduleMode) -> Entries {
        Entries(timeline: timeline, startDate: startDate, start: start, running: start.running && active)
    }

    struct Entries: Sequence, IteratorProtocol {
        let timeline: LyricsTimeline
        let startDate: Date
        let start: PlaybackSnapshotForSchedule
        let running: Bool
        private var lastMs: UInt32?

        init(timeline: LyricsTimeline, startDate: Date, start: PlaybackSnapshotForSchedule, running: Bool) {
            self.timeline = timeline
            self.startDate = startDate
            self.start = start
            self.running = running
        }

        mutating func next() -> Date? {
            guard let previous = lastMs else {
                lastMs = lyricMilliseconds(start.positionUs)
                return startDate
            }
            guard running, let change = timeline.nextChange(afterMs: previous) else { return nil }
            lastMs = change
            return ScheduleClock.date(for: Int64(change) * 1_000, start: start, at: startDate)
        }
    }
}

enum ScheduleClock {
    /// When playback reaches `targetUs`: the later of the drawn position's and the anchor's
    /// estimate (see `PlaybackSnapshotForSchedule.anchorPositionUs`), a millisecond late so the
    /// drawn position is past the change (as `jf play` does), and never at or before the start.
    static func date(for targetUs: Int64, start: PlaybackSnapshotForSchedule, at startDate: Date) -> Date {
        let ahead = max(targetUs - start.positionUs, targetUs - start.anchorPositionUs)
        let rate = start.rate > 0 ? start.rate : 1
        let seconds = Double(ahead) / 1_000_000 / rate + 0.001
        return startDate.addingTimeInterval(max(seconds, 0.001))
    }
}

/// The time one step of a `lengthPoints` long bar stands for at `scale` pixels per point, at
/// least 100 ms (as the Linux app): a bar redraws when it moves by a pixel, never more often.
func barStepUs(durationUs: Int64?, lengthPoints: CGFloat, scale: CGFloat) -> Int64? {
    guard let duration = durationUs, duration > 0 else { return nil }
    let pixels = Double(lengthPoints * max(scale, 1))
    guard pixels.isFinite, pixels >= 1 else { return nil }
    return max(Int64(Double(duration) / pixels), 100_000)
}
