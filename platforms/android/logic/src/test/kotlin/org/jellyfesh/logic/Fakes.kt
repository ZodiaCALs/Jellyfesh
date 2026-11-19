package org.jellyfesh.logic

import org.jellyfesh.logic.core.PlaybackAnchor
import org.jellyfesh.logic.time.DrawClock
import org.jellyfesh.logic.time.LyricCursor
import org.jellyfesh.logic.time.LyricTimelineLookup

/** A plain extrapolating clock (no slew): enough to test scheduling around the real port. */
class LinearClock : DrawClock {
    var anchor: PlaybackAnchor? = null

    override fun update(anchor: PlaybackAnchor, nowNs: Long) {
        this.anchor = anchor
    }

    override fun positionUs(nowNs: Long): Long {
        val a = anchor ?: return 0
        if (a.rate == 0f) return a.positionUs
        val pos = a.positionUs + ((nowNs - a.anchorMonoNs) * a.rate / 1_000.0).toLong()
        return pos.coerceIn(0, a.durationUs ?: Long.MAX_VALUE)
    }

    override val isRunning: Boolean get() = (anchor?.rate ?: 0f) > 0f

    override fun waitUntilNs(targetUs: Long, nowNs: Long): Long? {
        val a = anchor?.takeIf { it.rate > 0f } ?: return null
        val ahead = targetUs - positionUs(nowNs)
        return if (ahead <= 0) 0 else (ahead * 1_000 / a.rate).toLong()
    }
}

/**
 * Lyrics as (line, word, start, end) word spans, each line holding its words back to back:
 * a small stand-in for the ported cursor with the same rules for these simple cases.
 */
class SpanTimeline(private val words: List<Span>) : LyricTimelineLookup {
    data class Span(val line: Int, val word: Int, val startMs: Long, val endMs: Long)

    override val isEmpty: Boolean get() = words.isEmpty()

    override fun cursor(tMs: Long): LyricCursor {
        val current = words.lastOrNull { it.startMs <= tMs } ?: return LyricCursor.None
        val lineEnd = words.filter { it.line == current.line }.maxOf { it.endMs }
        if (tMs >= lineEnd) return LyricCursor.None
        val progress = if (current.endMs <= current.startMs) 1f else ((tMs - current.startMs).toFloat() / (current.endMs - current.startMs)).coerceIn(0f, 1f)
        return LyricCursor(current.line, current.word, progress)
    }

    override fun nextChange(tMs: Long): Long? {
        val points = words.flatMap { listOf(it.startMs, it.endMs) }.filter { it > tMs }
        return points.minOrNull()
    }

    /** The bar the real port gives for these spans: lines in order, words back to back. */
    override fun bar(tMs: Long): org.jellyfesh.logic.lyrics.LyricsBar {
        val lines = words.map { it.line }.distinct().sorted()
        val starts = lines.map { l -> words.filter { it.line == l }.minOf { it.startMs } }
        val ends = lines.map { l -> words.filter { it.line == l }.maxOf { it.endMs } }
        val spans = lines.flatMap { l -> words.filter { it.line == l }.sortedBy { it.word } }
        val offsets = lines.runningFold(0L) { acc, l -> acc + words.count { it.line == l } }
        val segments = lines.indices.flatMap { i -> listOf(starts[i] to i.toLong(), ends[i] to org.jellyfesh.logic.lyrics.TimelineArrays.NONE) }
            .sortedBy { it.first }
        val arrays = org.jellyfesh.logic.lyrics.TimelineArrays(
            lineStarts = starts.toLongArray(),
            lineEnds = ends.toLongArray(),
            segmentStarts = segments.map { it.first }.toLongArray(),
            segmentLines = segments.map { it.second }.toLongArray(),
            wordOffsets = offsets.toLongArray(),
            wordStarts = spans.map { it.startMs }.toLongArray(),
            wordEnds = spans.map { it.endMs }.toLongArray(),
        )
        return org.jellyfesh.logic.lyrics.LyricsBars.bar(arrays, tMs)
    }
}

fun anchor(positionMs: Long, atMs: Long, rate: Float = 1f, generation: Long = 1, durationMs: Long? = 240_000) =
    PlaybackAnchor(generation, positionMs * 1_000, atMs * 1_000_000, rate, durationMs?.times(1_000))

const val MS: Long = 1_000_000
