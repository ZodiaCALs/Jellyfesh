// Reference port of LyricTimeline::cursor and ::next_change (core/jf-lyrics/src/timeline.rs) on
// the generated JellyfeshFFI types. Tests/DrawingContractTests.swift replays
// tests/contract/lyrics-cursor.json. Swift 5 language mode; not compiled in this repository's CI
// yet (no Swift toolchain there): see docs/ffi.md.

#if canImport(JellyfeshFFI)
import JellyfeshFFI
#endif

/// Position of the playhead in the lyrics.
public struct LyricCursor: Equatable, Sendable {
    /// Index of the active line in `LyricDoc.lines`; nil before the first line, in gaps and after
    /// the last line ends.
    public var line: Int?
    /// Index of the current word in that line's `words`; nil before its first word.
    public var word: Int?
    /// How much of the current word has been sung, 0 to 1.
    public var wordProgress: Float

    public init(line: Int?, word: Int?, wordProgress: Float) {
        self.line = line
        self.word = word
        self.wordProgress = wordProgress
    }

    public static let none = LyricCursor(line: nil, word: nil, wordProgress: 0)
}

extension LyricTimeline {
    /// "No active line" in `segmentLines` and "no end" in `lineEnds`.
    public static let noLine: UInt32 = .max

    /// The cursor at `tMs`: two binary searches, cheap enough for every frame.
    ///
    /// A line is active when start ≤ t < end (a line without an end stays active once started);
    /// the active line that started last wins. The word is the last word of that line with
    /// start ≤ t; its progress is (t − start) / (end − start) clamped to 0…1 (1 for zero-length
    /// words).
    public func cursor(_ tMs: UInt32) -> LyricCursor {
        guard let line = activeLine(tMs) else { return .none }
        guard let from = Self.at(wordOffsets, line), let to = Self.at(wordOffsets, line + 1) else {
            return LyricCursor(line: line, word: nil, wordProgress: 0)
        }
        let (lo, hi) = Self.slice(wordStarts, from, to)
        let count = Self.countAtMost(wordStarts, lo, hi, tMs)
        guard count > 0 else { return LyricCursor(line: line, word: nil, wordProgress: 0) }
        let word = count - 1
        let start = wordStarts[lo + word]
        let end = Self.at(wordEnds, Int(from) + word) ?? start
        let progress = end <= start ? 1 : min(max(Float(tMs - start) / Float(end - start), 0), 1)
        return LyricCursor(line: line, word: word, wordProgress: progress)
    }

    /// The earliest time after `tMs` at which the cursor's line or word changes, or the current
    /// word finishes; nil when nothing changes after `tMs`. A view that draws no word sweep can
    /// sleep until then.
    public func nextChange(_ tMs: UInt32) -> UInt32? {
        let k = Self.countAtMost(segmentStarts, 0, segmentStarts.count, tMs)
        var next = Self.at(segmentStarts, k)
        guard let line = activeLine(tMs),
              let from = Self.at(wordOffsets, line),
              let to = Self.at(wordOffsets, line + 1)
        else { return next }
        let (lo, hi) = Self.slice(wordStarts, from, to)
        let count = Self.countAtMost(wordStarts, lo, hi, tMs)
        if lo + count < hi {
            let start = wordStarts[lo + count]
            next = next.map { min($0, start) } ?? start
        }
        if count > 0, let end = Self.at(wordEnds, Int(from) + count - 1), end > tMs, end != Self.noLine {
            next = next.map { min($0, end) } ?? end
        }
        return next
    }

    /// The active line at `tMs` from the segment arrays, or nil for none.
    private func activeLine(_ tMs: UInt32) -> Int? {
        let k = Self.countAtMost(segmentStarts, 0, segmentStarts.count, tMs)
        guard k > 0, let line = Self.at(segmentLines, k - 1), line != Self.noLine else { return nil }
        return Int(line)
    }

    private static func at(_ list: [UInt32], _ index: Int) -> UInt32? {
        index >= 0 && index < list.count ? list[index] : nil
    }

    /// The bounds of `from..<to` in the list, or an empty range when they do not fit.
    private static func slice(_ list: [UInt32], _ from: UInt32, _ to: UInt32) -> (Int, Int) {
        from <= to && Int(to) <= list.count ? (Int(from), Int(to)) : (0, 0)
    }

    /// How many of the ascending elements in `lo..<hi` are at most `t` (Rust's `partition_point`).
    private static func countAtMost(_ list: [UInt32], _ lo: Int, _ hi: Int, _ t: UInt32) -> Int {
        var low = lo
        var high = hi
        while low < high {
            let mid = low + (high - low) / 2
            if list[mid] <= t {
                low = mid + 1
            } else {
                high = mid
            }
        }
        return low - lo
    }
}
