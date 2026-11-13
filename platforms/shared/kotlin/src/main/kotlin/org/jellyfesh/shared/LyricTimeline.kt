// Reference port of LyricTimeline::cursor and ::next_change (core/jf-lyrics/src/timeline.rs).
// Tested against tests/contract/lyrics-cursor.json; keep it in step with the Rust original.

package org.jellyfesh.shared

import org.jellyfesh.ffi.LyricTimeline

/** Marks "no active line" in [LyricTimeline.segmentLines] and "no end" in [LyricTimeline.lineEnds]. */
val NO_LINE: UInt = UInt.MAX_VALUE

/**
 * Position of the playhead in the lyrics.
 *
 * @property line index of the active line in `LyricDoc.lines`, or null before the first line, in
 *   gaps and after the last line ends.
 * @property word index of the current word in that line's `words`, or null before its first word.
 * @property wordProgress how much of the current word has been sung, 0 to 1.
 */
data class LyricCursor(val line: Int?, val word: Int?, val wordProgress: Float) {
    companion object {
        val NONE = LyricCursor(null, null, 0f)
    }
}

/**
 * The cursor at [tMs]: two binary searches, cheap enough for every frame.
 *
 * A line is active when `start <= t < end` (a line without an end stays active once started); the
 * active line that started last wins. The word is the last word of that line with `start <= t`;
 * its progress is `(t - start) / (end - start)` clamped to 0..1 (1 for zero-length words).
 */
fun LyricTimeline.cursor(tMs: UInt): LyricCursor {
    val line = activeLine(tMs) ?: return LyricCursor.NONE
    val from = wordOffsets.at(line)
    val to = wordOffsets.at(line + 1)
    if (from == null || to == null) return LyricCursor(line.toInt(), null, 0f)
    val (lo, hi) = wordStarts.slice(from, to)
    val count = wordStarts.countAtMost(lo, hi, tMs)
    if (count == 0) return LyricCursor(line.toInt(), null, 0f)
    val word = count - 1
    val start = wordStarts[lo + word]
    val end = wordEnds.at(from.toLong() + word) ?: start
    val progress = if (end <= start) {
        1f
    } else {
        ((tMs - start).toFloat() / (end - start).toFloat()).coerceIn(0f, 1f)
    }
    return LyricCursor(line.toInt(), word, progress)
}

/**
 * The earliest time after [tMs] at which the cursor's line or word changes, or the current word
 * finishes; null when nothing changes after [tMs]. A view that draws no word sweep can sleep until
 * then instead of drawing every frame.
 */
fun LyricTimeline.nextChange(tMs: UInt): UInt? {
    val k = segmentStarts.countAtMost(0, segmentStarts.size, tMs)
    var next: UInt? = segmentStarts.getOrNull(k)
    val line = activeLine(tMs) ?: return next
    val from = wordOffsets.at(line) ?: return next
    val to = wordOffsets.at(line + 1) ?: return next
    val (lo, hi) = wordStarts.slice(from, to)
    val count = wordStarts.countAtMost(lo, hi, tMs)
    if (lo + count < hi) {
        val start = wordStarts[lo + count]
        next = next?.let { minOf(it, start) } ?: start
    }
    if (count > 0) {
        val end = wordEnds.at(from.toLong() + count - 1)
        if (end != null && end > tMs && end != NO_LINE) {
            next = next?.let { minOf(it, end) } ?: end
        }
    }
    return next
}

/** The active line at [tMs] from the segment arrays, as an index, or null for none. */
private fun LyricTimeline.activeLine(tMs: UInt): Long? {
    val k = segmentStarts.countAtMost(0, segmentStarts.size, tMs)
    if (k == 0) return null
    val line = segmentLines.getOrNull(k - 1) ?: return null
    return if (line == NO_LINE) null else line.toLong()
}

/** `list[index]`, or null when the index is out of range. */
private fun List<UInt>.at(index: Long): UInt? =
    if (index in 0 until size) this[index.toInt()] else null

/** The bounds of `from until to` in this list, or an empty range when they do not fit. */
private fun List<UInt>.slice(from: UInt, to: UInt): Pair<Int, Int> =
    if (from <= to && to.toLong() <= size) from.toInt() to to.toInt() else 0 to 0

/** How many of the ascending elements in `[lo, hi)` are at most [t] (Rust's `partition_point`). */
private fun List<UInt>.countAtMost(lo: Int, hi: Int, t: UInt): Int {
    var low = lo
    var high = hi
    while (low < high) {
        val mid = (low + high) ushr 1
        if (this[mid] <= t) low = mid + 1 else high = mid
    }
    return low - lo
}
