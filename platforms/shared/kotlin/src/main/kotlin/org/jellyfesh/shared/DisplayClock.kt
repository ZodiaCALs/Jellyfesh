// Reference port of DisplayClock, next_second_us and next_step_us (core/jf-core/src/timing.rs) and
// PlaybackAnchor::position_at (core/jf-engine/src/anchor.rs). Tested against
// tests/contract/display-clock.json; keep it in step with the Rust original.

package org.jellyfesh.shared

import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.sign
import kotlin.math.truncate
import kotlin.time.Duration
import kotlin.time.Duration.Companion.microseconds
import org.jellyfesh.ffi.PlaybackAnchor

/**
 * A smoothed, monotonic playback position for drawing progress bars and lyrics.
 *
 * Feed it every `PlaybackSnapshot.anchor` and draw from [positionUs] with `monotonicNowNs()`.
 * It never steps backwards while playing, and spreads the small corrections the engine makes when
 * it re-anchors (up to 250 ms) over a short slew instead of jumping; a new track, a seek, a pause
 * or a resume shows at once. [waitUntil] turns "the next visible change is at position X" into a
 * delay, so a view sleeps until something on screen changes. Not thread-safe: use it from the
 * thread that draws.
 */
class DisplayClock {
    /** The latest anchor. */
    var anchor: PlaybackAnchor? = null
        private set

    /** Correction added to the anchor's position, fading linearly to zero. */
    private var offsetUs = 0L
    private var offsetSinceNs: ULong = 0uL
    private var slewUs = 0L

    /** Last position handed out, so it never decreases while playing. */
    private var lastUs: Long? = null

    /** Feeds an anchor from the playback slice. */
    fun update(anchor: PlaybackAnchor, nowNs: ULong) {
        val old = this.anchor
        val continuous = old != null &&
            old.trackGeneration == anchor.trackGeneration &&
            old.rate > 0f &&
            anchor.rate > 0f
        val shown = drawn(nowNs)
        this.anchor = anchor
        val error = shown - anchor.positionAt(nowNs)
        if (continuous && error != 0L && abs(error) <= MAX_SLEW_US) {
            offsetUs = error
            offsetSinceNs = nowNs
            slewUs = maxOf(abs(error) * 4, MIN_SLEW_US)
        } else {
            // Show the new anchor exactly, backwards too.
            offsetUs = 0
            lastUs = null
        }
    }

    /** The position to draw at [nowNs], in microseconds within the track. */
    fun positionUs(nowNs: ULong): Long {
        val anchor = this.anchor ?: return 0
        var pos = drawn(nowNs)
        val last = lastUs
        if (anchor.rate > 0f && last != null) pos = maxOf(pos, last)
        anchor.durationUs?.let { pos = minOf(pos, maxOf(it, 0L)) }
        pos = maxOf(pos, 0L)
        lastUs = pos
        return pos
    }

    /** True while the position moves. */
    val isRunning: Boolean
        get() = (anchor?.rate ?: 0f) > 0f

    /**
     * How long until the drawn position reaches [targetUs]: zero when it already has, null while
     * stopped or paused. During a slew the result is off by up to a quarter, so re-ask on waking.
     */
    fun waitUntil(targetUs: Long, nowNs: ULong): Duration? {
        val anchor = this.anchor?.takeIf { it.rate > 0f } ?: return null
        val pos = maxOf(drawn(nowNs), lastUs ?: Long.MIN_VALUE)
        val ahead = saturatingSub(targetUs, pos)
        if (ahead <= 0) return Duration.ZERO
        val us = ceil(ahead.toDouble() / anchor.rate.toDouble())
        return us.toLong().coerceAtLeast(0).microseconds
    }

    private fun drawn(nowNs: ULong): Long {
        val anchor = this.anchor ?: return 0
        return saturatingAdd(anchor.positionAt(nowNs), fadingOffset(nowNs))
    }

    private fun fadingOffset(nowNs: ULong): Long {
        if (offsetUs == 0L) return 0
        val elapsedNs = if (nowNs > offsetSinceNs) nowNs - offsetSinceNs else 0uL
        val elapsedUs = (elapsedNs / 1_000uL).let { if (it > Long.MAX_VALUE.toULong()) Long.MAX_VALUE else it.toLong() }
        return if (elapsedUs >= slewUs) 0 else offsetUs * (slewUs - elapsedUs) / slewUs
    }

    companion object {
        /** Corrections up to this size are slewed; larger ones jump. */
        const val MAX_SLEW_US: Long = 250_000

        /** Shortest slew; longer corrections take four times their size. */
        const val MIN_SLEW_US: Long = 100_000
    }
}

/**
 * The extrapolated audible position at [nowNs], in microseconds:
 * `positionUs + (nowNs - anchorMonoNs) * rate / 1000`, rounded half away from zero. A paused
 * anchor (rate 0) always returns `positionUs`.
 */
fun PlaybackAnchor.positionAt(nowNs: ULong): Long {
    if (rate == 0f) return positionUs
    val elapsedNs = if (nowNs >= anchorMonoNs) {
        (nowNs - anchorMonoNs).toDouble()
    } else {
        -(anchorMonoNs - nowNs).toDouble()
    }
    // Double.toLong() saturates, like Rust's `as i64`.
    return saturatingAdd(positionUs, roundHalfAwayFromZero(elapsedNs * rate.toDouble() / 1000.0).toLong())
}

/** The next whole second after [positionUs], when an `m:ss` display changes. */
fun nextSecond(positionUs: Long): Long = (Math.floorDiv(positionUs, 1_000_000L) + 1) * 1_000_000L

/**
 * The next multiple of [stepUs] after [positionUs], when a progress bar drawn in steps of [stepUs]
 * changes; null for a step of zero or less.
 */
fun nextStep(positionUs: Long, stepUs: Long): Long? =
    if (stepUs > 0) (Math.floorDiv(positionUs, stepUs) + 1) * stepUs else null

/** Rust's `f64::round`: halves round away from zero (Kotlin's `round` rounds them to even). */
private fun roundHalfAwayFromZero(x: Double): Double {
    val whole = truncate(x)
    return if (abs(x - whole) >= 0.5) whole + sign(x) else whole
}

private fun saturatingAdd(a: Long, b: Long): Long {
    val sum = a + b
    return if (((a xor sum) and (b xor sum)) < 0) (if (a < 0) Long.MIN_VALUE else Long.MAX_VALUE) else sum
}

private fun saturatingSub(a: Long, b: Long): Long {
    val difference = a - b
    return if (((a xor b) and (a xor difference)) < 0) {
        if (a < 0) Long.MIN_VALUE else Long.MAX_VALUE
    } else {
        difference
    }
}
