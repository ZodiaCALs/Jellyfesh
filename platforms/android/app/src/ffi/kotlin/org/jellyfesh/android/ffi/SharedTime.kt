package org.jellyfesh.android.ffi

import org.jellyfesh.ffi.LyricTimeline
import org.jellyfesh.logic.core.PlaybackAnchor
import org.jellyfesh.logic.lyrics.LyricsBar
import org.jellyfesh.logic.lyrics.LyricsBars
import org.jellyfesh.logic.lyrics.TimelineArrays
import org.jellyfesh.logic.time.DrawClock
import org.jellyfesh.logic.time.LyricCursor
import org.jellyfesh.logic.time.LyricTimelineLookup
import org.jellyfesh.shared.DisplayClock
import org.jellyfesh.shared.cursor
import org.jellyfesh.shared.nextChange

/**
 * The shared Kotlin port of `DisplayClock` (platforms/shared/kotlin) behind :logic's [DrawClock].
 * Main thread only, like the port.
 */
class PortDrawClock : DrawClock {
    private val clock = DisplayClock()

    override fun update(anchor: PlaybackAnchor, nowNs: Long) {
        clock.update(FfiMapping.ffi(anchor), nowNs.toULongClamped())
    }

    override fun positionUs(nowNs: Long): Long = clock.positionUs(nowNs.toULongClamped())

    override val isRunning: Boolean get() = clock.isRunning

    override fun waitUntilNs(targetUs: Long, nowNs: Long): Long? =
        clock.waitUntil(targetUs, nowNs.toULongClamped())?.inWholeNanoseconds
}

/**
 * The shared port of `LyricTimeline.cursor` and `nextChange` over the core's timeline arrays, as
 * the core sent them (no copy).
 */
class PortLyricTimeline(private val timeline: LyricTimeline) : LyricTimelineLookup {
    override val isEmpty: Boolean get() = timeline.segmentStarts.isEmpty()

    override fun cursor(tMs: Long): LyricCursor {
        val c = timeline.cursor(tMs.toUIntClamped())
        return LyricCursor(c.line, c.word, c.wordProgress)
    }

    override fun nextChange(tMs: Long): Long? = timeline.nextChange(tMs.toUIntClamped())?.toLong()

    /** The timeline's arrays for the lyrics bar's port, copied once, when the bar first needs them. */
    private val arrays: TimelineArrays by lazy {
        TimelineArrays(
            lineStarts = timeline.lineStarts.longs(),
            lineEnds = timeline.lineEnds.longs(),
            segmentStarts = timeline.segmentStarts.longs(),
            segmentLines = timeline.segmentLines.longs(),
            wordOffsets = timeline.wordOffsets.longs(),
            wordStarts = timeline.wordStarts.longs(),
            wordEnds = timeline.wordEnds.longs(),
        )
    }

    override fun bar(tMs: Long): LyricsBar = LyricsBars.bar(arrays, tMs.coerceAtLeast(0))
}

private fun List<UInt>.longs(): LongArray = LongArray(size) { this[it].toLong() }

private fun Long.toULongClamped(): ULong = if (this < 0) 0uL else toULong()

private fun Long.toUIntClamped(): UInt = when {
    this < 0 -> 0u
    this > UInt.MAX_VALUE.toLong() -> UInt.MAX_VALUE
    else -> toUInt()
}
