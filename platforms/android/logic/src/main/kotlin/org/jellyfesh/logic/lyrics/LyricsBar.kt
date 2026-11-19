package org.jellyfesh.logic.lyrics

/**
 * The core's `LyricTimeline` arrays, copied once per Lyrics slice: times in milliseconds, with
 * [NONE] (the core's `u32::MAX`) for "no line" in [segmentLines] and "no end" in [lineEnds].
 * Lines are in start order, as the core keeps them.
 */
class TimelineArrays(
    val lineStarts: LongArray,
    val lineEnds: LongArray,
    val segmentStarts: LongArray,
    val segmentLines: LongArray,
    val wordOffsets: LongArray,
    val wordStarts: LongArray,
    val wordEnds: LongArray,
) {
    companion object {
        /** `u32::MAX`: no line, or no end. */
        const val NONE: Long = 4_294_967_295L
    }
}

/**
 * What the one-line lyrics bar shows at a time (the core's `LyricsBar`, ADR-0022 §6): the line
 * (the active one, or in a gap the last one that started), the next line, how much of each word
 * of the line is sung (0 to 1) and, for lines without word timing, how much of the line is over.
 */
data class LyricsBar(val line: Int?, val next: Int?, val words: List<Float>, val lineProgress: Float) {
    companion object {
        val Empty = LyricsBar(null, null, emptyList(), 0f)
    }
}

/**
 * The port of the core's `lyrics_bar` (core/jf-core/src/timing.rs): a binary search and one pass
 * over a line's words, so the lyrics bar may run it every frame while words are swept, instead of
 * asking the core. Replays tests/contract/lyrics-bar.json.
 */
object LyricsBars {
    /** What the lyrics bar shows at [tMs] for [timeline]. */
    fun bar(timeline: TimelineArrays, tMs: Long): LyricsBar {
        val starts = timeline.lineStarts
        fun timed(i: Int): Boolean = i in starts.indices && starts[i] != TimelineArrays.NONE
        val active = activeLine(timeline, tMs)
        // In a gap, the last line that started stays.
        val line = active ?: (countAtMost(starts, 0, starts.size, tMs) - 1 downTo 0).firstOrNull(::timed)
        val next = if (line != null) (line + 1 until starts.size).firstOrNull(::timed) else starts.indices.firstOrNull(::timed)
        if (line == null) return LyricsBar(null, next, emptyList(), 0f)
        val from = timeline.wordOffsets.getOrNull(line)
        val to = timeline.wordOffsets.getOrNull(line + 1)
        val words = if (from == null || to == null || from > to) {
            emptyList()
        } else {
            (from.toInt() until to.toInt()).map { w ->
                val start = timeline.wordStarts.getOrElse(w) { 0L }
                val end = maxOf(timeline.wordEnds.getOrElse(w) { start }, start)
                when {
                    tMs < start -> 0f
                    tMs >= end -> 1f
                    else -> (tMs - start).toFloat() / (end - start).toFloat()
                }
            }
        }
        val start = starts[line]
        val ownEnd = timeline.lineEnds.getOrNull(line)?.takeIf { it != TimelineArrays.NONE }
        val end = ownEnd ?: next?.let { starts[it] }
        val lineProgress = when {
            // In the gap after the line: it is over.
            active == null && tMs >= start -> if (words.isEmpty()) 0f else 1f
            end != null && end > start -> ((tMs - start).coerceAtLeast(0).toFloat() / (end - start).toFloat()).coerceIn(0f, 1f)
            else -> 0f
        }
        return LyricsBar(line, next, words, lineProgress)
    }

    /** The active line at [tMs] (the line of the core's `LyricTimeline::cursor`), or null. */
    fun activeLine(timeline: TimelineArrays, tMs: Long): Int? {
        val k = countAtMost(timeline.segmentStarts, 0, timeline.segmentStarts.size, tMs)
        if (k == 0) return null
        val line = timeline.segmentLines.getOrNull(k - 1) ?: return null
        return if (line == TimelineArrays.NONE) null else line.toInt()
    }

    /** How many of the ascending values in `[lo, hi)` are at most [t] (Rust's `partition_point`). */
    private fun countAtMost(values: LongArray, lo: Int, hi: Int, t: Long): Int {
        var low = lo
        var high = hi
        while (low < high) {
            val mid = (low + high) ushr 1
            if (values[mid] <= t) low = mid + 1 else high = mid
        }
        return low - lo
    }
}
