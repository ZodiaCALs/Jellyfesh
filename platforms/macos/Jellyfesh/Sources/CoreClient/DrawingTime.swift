// Drawing time without waste (docs/architecture.md §5): the app draws positions from the shared
// ports in platforms/shared/swift (DisplayClock, LyricTimeline.cursor and nextChange), compiled
// into the JellyfeshFFI module, and never asks the core per frame. These wrappers keep the
// generated types inside this folder.

import JellyfeshFFI

/// The position to draw progress and lyrics at, fed by every playback anchor.
///
/// Wraps the shared `DisplayClock` port: it never steps back while playing and slews small
/// re-anchoring corrections. A reference type so time-driven views can read it per frame without
/// SwiftUI observing it; stores signal new anchors separately. Main thread only.
final class PlaybackClock {
    private var clock = DisplayClock()
    /// The latest anchor.
    private(set) var anchor: Anchor = .zero

    init() {}

    func update(_ anchor: Anchor, nowNs: UInt64) {
        self.anchor = anchor
        clock.update(
            PlaybackAnchor(
                trackGeneration: anchor.trackGeneration,
                positionUs: anchor.positionUs,
                anchorMonoNs: anchor.anchorMonoNs,
                rate: anchor.rate,
                durationUs: anchor.durationUs
            ),
            nowNs: nowNs
        )
    }

    /// The position to draw at `nowNs`, in microseconds within the track.
    func positionUs(nowNs: UInt64) -> Int64 {
        clock.positionUs(nowNs: nowNs)
    }

    /// Where the latest anchor alone puts playback at `nowNs`, kept within the track. The drawn
    /// position slews towards it after a small correction and equals it once the slew is over.
    func anchorPositionUs(nowNs: UInt64) -> Int64 {
        guard let latest = clock.anchor else { return 0 }
        var position = latest.positionAt(nowNs)
        if let duration = latest.durationUs {
            position = min(position, max(duration, 0))
        }
        return max(position, 0)
    }

    /// True while the position moves.
    var isRunning: Bool { clock.isRunning }

    /// The anchor's rate: 1 while playing, 0 while paused, buffering or stopped.
    var rate: Float { clock.anchor?.rate ?? 0 }

    var durationUs: Int64? { anchor.durationUs }

    /// Microseconds until the drawn position reaches `targetUs` (0 when it has), nil while paused.
    func waitUntilUs(targetUs: Int64, nowNs: UInt64) -> UInt64? {
        clock.waitUntilUs(targetUs: targetUs, nowNs: nowNs)
    }
}

/// The next whole second after `positionUs`, when an m:ss label changes (shared port).
func nextSecondUs(after positionUs: Int64) -> Int64 {
    nextSecond(positionUs)
}

/// The next multiple of `stepUs` after `positionUs`, when a bar drawn in steps changes (shared
/// port); nil for a step of zero or less.
func nextStepUs(after positionUs: Int64, step stepUs: Int64) -> Int64? {
    nextStep(positionUs, step: stepUs)
}

/// Where the playhead is in the lyrics (`LyricCursor`).
struct LyricsPosition: Equatable, Sendable {
    /// Index of the active line in `LyricsDocument.lines`; nil before the first line and in gaps.
    var line: Int?
    /// Index of the current word in that line's `words`; nil before its first word.
    var word: Int?
    /// How much of the current word has been sung, 0 to 1.
    var wordProgress: Float

    static let none = LyricsPosition(line: nil, word: nil, wordProgress: 0)
}

/// The flat lyric lookup arrays (`LyricTimeline`) with the shared cursor port.
struct LyricsTimeline: Equatable, @unchecked Sendable {
    fileprivate let timeline: LyricTimeline

    init(_ timeline: LyricTimeline) {
        self.timeline = timeline
    }

    /// For tests and previews: the arrays as the core sends them (`u32::MAX` for none).
    init(lineStarts: [UInt32], lineEnds: [UInt32], segmentStarts: [UInt32], segmentLines: [UInt32],
         wordOffsets: [UInt32], wordStarts: [UInt32], wordEnds: [UInt32]) {
        timeline = LyricTimeline(
            lineStarts: lineStarts,
            lineEnds: lineEnds,
            segmentStarts: segmentStarts,
            segmentLines: segmentLines,
            wordOffsets: wordOffsets,
            wordStarts: wordStarts,
            wordEnds: wordEnds
        )
    }

    /// The cursor at `tMs`: two binary searches, cheap enough for every frame.
    func position(atMs tMs: UInt32) -> LyricsPosition {
        let cursor = timeline.cursor(tMs)
        return LyricsPosition(line: cursor.line, word: cursor.word, wordProgress: cursor.wordProgress)
    }

    /// The next time after `tMs` the line or word changes or a word finishes; nil when nothing
    /// changes after it.
    func nextChange(afterMs tMs: UInt32) -> UInt32? {
        timeline.nextChange(tMs)
    }

    /// Start of each line in `LyricsDocument.lines`, ascending.
    var lineStarts: [UInt32] { timeline.lineStarts }
    /// End of each line; `UInt32.max` when the line never ends.
    var lineEnds: [UInt32] { timeline.lineEnds }
    /// Words of line `i` are at `wordOffsets[i]..<wordOffsets[i + 1]` in the word arrays.
    var wordOffsets: [UInt32] { timeline.wordOffsets }
    var wordStarts: [UInt32] { timeline.wordStarts }
    var wordEnds: [UInt32] { timeline.wordEnds }
}

/// Milliseconds for the lyric cursor from a drawn position in microseconds.
func lyricMilliseconds(_ positionUs: Int64) -> UInt32 {
    UInt32(clamping: max(positionUs, 0) / 1_000)
}
