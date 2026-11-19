// The lyrics bar (ADR-0022 §6, docs/core-api.md "The lyrics bar"): what the one-line lyrics bar in
// the player bar and Desktop Lyrics show at a time. A port of jf_core::lyrics_bar
// (core/jf-core/src/timing.rs), so it can run every frame while words are swept without asking
// the core; Tests/LyricsBarContractTests.swift replays tests/contract/lyrics-bar.json, which is
// recorded from the Rust original, as every app's port does.

import Foundation

/// What the lyrics bar shows at a time (`LyricsBar`).
struct LyricsBarState: Equatable, Sendable {
    /// The line to show, an index into `LyricsDocument.lines`: the active line, or in a gap
    /// between lines the last one that started. Nil before the first line and for unsynced
    /// lyrics.
    var line: Int?
    /// The line after it (before the first line, the first one).
    var next: Int?
    /// How much of each word of `line` is sung, 0 (to come) to 1 (sung). Empty for lines without
    /// word timing.
    var words: [Float]
    /// How much of `line` is over, 0–1, for lines without word timing.
    var lineProgress: Float

    static let empty = LyricsBarState(line: nil, next: nil, words: [], lineProgress: 0)
}

extension LyricsTimeline {
    /// The lyrics bar at `tMs`: a binary search and one pass over the line's words.
    func bar(atMs tMs: UInt32) -> LyricsBarState {
        let starts = lineStarts
        let count = starts.count
        func timed(_ index: Int) -> Bool { index >= 0 && index < count && starts[index] != .max }

        let active = position(atMs: tMs).line
        var line = active
        if line == nil {
            // The last line that started, if any: it stays through the gap.
            let started = Self.countAtMost(starts, tMs)
            line = (0..<started).reversed().first(where: timed)
        }
        let next: Int?
        if let line {
            next = line + 1 < count ? (line + 1..<count).first(where: timed) : nil
        } else {
            next = (0..<count).first(where: timed)
        }
        guard let line, line < count else {
            return LyricsBarState(line: nil, next: next, words: [], lineProgress: 0)
        }

        let offsets = wordOffsets
        var words: [Float] = []
        if line + 1 < offsets.count {
            let from = Int(offsets[line])
            let to = min(Int(offsets[line + 1]), wordStarts.count, wordEnds.count)
            if to > from {
                words.reserveCapacity(to - from)
                for word in from..<to {
                    let start = wordStarts[word]
                    let end = max(wordEnds[word], start)
                    if tMs < start {
                        words.append(0)
                    } else if tMs >= end {
                        words.append(1)
                    } else {
                        words.append(Float(tMs - start) / Float(end - start))
                    }
                }
            }
        }

        let start = starts[line]
        let end: UInt32?
        if line < lineEnds.count, lineEnds[line] != .max {
            end = lineEnds[line]
        } else {
            end = next.map { starts[$0] }
        }
        let progress: Float
        if active == nil && tMs >= start {
            // In the gap after the line: it is over.
            progress = words.isEmpty ? 0 : 1
        } else if let end, end > start {
            let elapsed = tMs >= start ? tMs - start : 0
            progress = min(max(Float(elapsed) / Float(end - start), 0), 1)
        } else {
            progress = 0
        }
        return LyricsBarState(line: line, next: next, words: words, lineProgress: progress)
    }

    /// How many of the ascending `values` are at most `t` (Rust's `partition_point`).
    private static func countAtMost(_ values: [UInt32], _ t: UInt32) -> Int {
        var low = 0
        var high = values.count
        while low < high {
            let middle = (low + high) / 2
            if values[middle] <= t { low = middle + 1 } else { high = middle }
        }
        return low
    }
}
