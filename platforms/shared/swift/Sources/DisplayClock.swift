// Reference port of DisplayClock, next_second_us and next_step_us (core/jf-core/src/timing.rs) and
// PlaybackAnchor::position_at (core/jf-engine/src/anchor.rs) on the generated JellyfeshFFI types.
// Tests/DrawingContractTests.swift replays tests/contract/display-clock.json. Swift 5 language
// mode; not compiled in this repository's CI yet (no Swift toolchain there): see docs/ffi.md.

#if canImport(JellyfeshFFI)
import JellyfeshFFI
#endif

/// A smoothed, monotonic playback position for drawing progress bars and lyrics.
///
/// Feed it every `PlaybackSnapshot.anchor` and draw from `positionUs(nowNs: monotonicNowNs())`.
/// It never steps backwards while playing, and spreads the small corrections the engine makes
/// when it re-anchors (up to 250 ms) over a short slew; a new track, a seek, a pause or a resume
/// shows at once. `waitUntil` turns "the next visible change is at position X" into a delay, so a
/// view sleeps until something on screen changes. Use it from the thread that draws.
public struct DisplayClock {
    /// Corrections up to this size are slewed; larger ones jump.
    public static let maxSlewUs: Int64 = 250_000
    /// Shortest slew; longer corrections take four times their size.
    public static let minSlewUs: Int64 = 100_000

    /// The latest anchor.
    public private(set) var anchor: PlaybackAnchor?
    // Correction added to the anchor's position, fading linearly to zero.
    private var offsetUs: Int64 = 0
    private var offsetSinceNs: UInt64 = 0
    private var slewUs: Int64 = 0
    // Last position handed out, so it never decreases while playing.
    private var lastUs: Int64?

    public init() {}

    /// Feeds an anchor from the playback slice.
    public mutating func update(_ anchor: PlaybackAnchor, nowNs: UInt64) {
        let continuous = self.anchor.map {
            $0.trackGeneration == anchor.trackGeneration && $0.rate > 0 && anchor.rate > 0
        } ?? false
        let shown = drawn(nowNs)
        self.anchor = anchor
        let error = shown &- anchor.positionAt(nowNs)
        let size = error.magnitude
        if continuous && error != 0 && size <= UInt64(Self.maxSlewUs) {
            offsetUs = error
            offsetSinceNs = nowNs
            slewUs = max(Int64(size) * 4, Self.minSlewUs)
        } else {
            // Show the new anchor exactly, backwards too.
            offsetUs = 0
            lastUs = nil
        }
    }

    /// The position to draw at `nowNs`, in microseconds within the track.
    public mutating func positionUs(nowNs: UInt64) -> Int64 {
        guard let anchor else { return 0 }
        var pos = drawn(nowNs)
        if anchor.rate > 0, let last = lastUs {
            pos = max(pos, last)
        }
        if let duration = anchor.durationUs {
            pos = min(pos, max(duration, 0))
        }
        pos = max(pos, 0)
        lastUs = pos
        return pos
    }

    /// True while the position moves.
    public var isRunning: Bool { (anchor?.rate ?? 0) > 0 }

    /// How many microseconds until the drawn position reaches `targetUs`: 0 when it already has,
    /// nil while stopped or paused. During a slew the result is off by up to a quarter, so re-ask
    /// on waking.
    public func waitUntilUs(targetUs: Int64, nowNs: UInt64) -> UInt64? {
        guard let anchor, anchor.rate > 0 else { return nil }
        let pos = max(drawn(nowNs), lastUs ?? .min)
        let ahead = saturatingSub(targetUs, pos)
        guard ahead > 0 else { return 0 }
        let us = (Double(ahead) / Double(anchor.rate)).rounded(.up)
        return us >= 18_446_744_073_709_551_615.0 ? .max : UInt64(us)
    }

    /// `waitUntilUs` as a `Duration`, for `Task.sleep(for:)`.
    @available(macOS 13.0, iOS 16.0, *)
    public func waitUntil(targetUs: Int64, nowNs: UInt64) -> Duration? {
        waitUntilUs(targetUs: targetUs, nowNs: nowNs).map { .microseconds(Int64(clamping: $0)) }
    }

    private func drawn(_ nowNs: UInt64) -> Int64 {
        guard let anchor else { return 0 }
        return saturatingAdd(anchor.positionAt(nowNs), fadingOffset(nowNs))
    }

    private func fadingOffset(_ nowNs: UInt64) -> Int64 {
        guard offsetUs != 0 else { return 0 }
        let elapsedNs = nowNs > offsetSinceNs ? nowNs - offsetSinceNs : 0
        let elapsedUs = Int64(clamping: elapsedNs / 1_000)
        return elapsedUs >= slewUs ? 0 : offsetUs * (slewUs - elapsedUs) / slewUs
    }
}

extension PlaybackAnchor {
    /// The extrapolated audible position at `nowNs` in microseconds:
    /// positionUs + (nowNs − anchorMonoNs) × rate / 1000, rounded half away from zero. A paused
    /// anchor (rate 0) always returns `positionUs`.
    public func positionAt(_ nowNs: UInt64) -> Int64 {
        guard rate != 0 else { return positionUs }
        let elapsedNs = nowNs >= anchorMonoNs ? Double(nowNs - anchorMonoNs) : -Double(anchorMonoNs - nowNs)
        let advance = (elapsedNs * Double(rate) / 1000).rounded(.toNearestOrAwayFromZero)
        return saturatingAdd(positionUs, saturatingInt64(advance))
    }
}

/// The next whole second after `positionUs`, when an m:ss display changes.
public func nextSecond(_ positionUs: Int64) -> Int64 {
    (floorDiv(positionUs, 1_000_000) + 1) * 1_000_000
}

/// The next multiple of `stepUs` after `positionUs`, when a progress bar drawn in steps of
/// `stepUs` changes; nil for a step of zero or less.
public func nextStep(_ positionUs: Int64, step stepUs: Int64) -> Int64? {
    stepUs > 0 ? (floorDiv(positionUs, stepUs) + 1) * stepUs : nil
}

private func floorDiv(_ a: Int64, _ b: Int64) -> Int64 {
    let q = a / b
    return a % b != 0 && a < 0 ? q - 1 : q
}

private func saturatingAdd(_ a: Int64, _ b: Int64) -> Int64 {
    let (sum, overflow) = a.addingReportingOverflow(b)
    return overflow ? (b > 0 ? .max : .min) : sum
}

private func saturatingSub(_ a: Int64, _ b: Int64) -> Int64 {
    let (difference, overflow) = a.subtractingReportingOverflow(b)
    return overflow ? (b < 0 ? .max : .min) : difference
}

/// Rust's `as i64`: saturating, NaN to 0.
private func saturatingInt64(_ x: Double) -> Int64 {
    if x.isNaN { return 0 }
    if x >= 9_223_372_036_854_775_807.0 { return .max }
    if x <= -9_223_372_036_854_775_808.0 { return .min }
    return Int64(x)
}
