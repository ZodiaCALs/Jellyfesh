package org.jellyfesh.logic.lyrics

import org.jellyfesh.logic.core.AuxKind
import org.jellyfesh.logic.core.LyricDoc
import org.jellyfesh.logic.core.LyricLine
import org.jellyfesh.logic.core.LyricWord
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.time.LyricCursor

/**
 * One lyric line as the view draws it.
 *
 * @property text the display text. For word-timed lines it is built from the words (a space
 *   after each word unless it joins the next, as `jf play` draws it), so [wordRanges] index it.
 * @property wordRanges the UTF-16 range of each word in [text]; empty for line-timed lines.
 */
data class LyricRow(
    val index: Int,
    val text: String,
    val wordRanges: List<IntRange>,
    val startMs: Long?,
    val translation: String?,
    val romanization: String?,
    /** Background vocals, shown smaller under the line. */
    val background: String?,
    val direction: TextDir,
    /** Han, Kana or Hangul: drawn with the CJK line height (tokens: lyrics.line-cjk-line-height). */
    val cjk: Boolean,
    /** An instrumental gap (a timed line without text). */
    val isGap: Boolean,
)

/** How much of the active line is sung. */
data class Sweep(
    /** Characters before this offset are sung. */
    val sungEnd: Int,
    /** The word being sung, and how much of it ([progress], 0 to 1), or null. */
    val current: IntRange?,
    val progress: Float,
) {
    companion object {
        val NoneSung = Sweep(0, null, 0f)
        fun allSung(row: LyricRow) = Sweep(row.text.length, null, 0f)
    }
}

/** Turns a [LyricDoc] into rows and a cursor into a sweep. Pure; tested on the JVM. */
object LyricsLayout {
    fun rows(doc: LyricDoc, showTranslation: Boolean, showRomanization: Boolean): List<LyricRow> {
        val translation = doc.tracks.firstOrNull { it.kind == AuxKind.Translation }.takeIf { showTranslation }
        val romanization = doc.tracks.firstOrNull { it.kind == AuxKind.Romanization }.takeIf { showRomanization }
        return doc.lines.mapIndexed { index, line ->
            val (text, ranges) = displayText(line)
            LyricRow(
                index = index,
                text = text,
                wordRanges = ranges,
                startMs = line.startMs,
                translation = translation?.lines?.get(line.id)?.text?.takeIf { it.isNotBlank() },
                romanization = romanization?.lines?.get(line.id)?.text?.takeIf { it.isNotBlank() },
                background = line.background.takeIf { it.isNotEmpty() }?.let { joinWords(it).first },
                direction = line.direction,
                cjk = isCjk(text),
                isGap = text.isBlank() && doc.timing != Timing.Unsynced,
            )
        }
    }

    /** The line's text and word ranges (built from the words when it has any). */
    fun displayText(line: LyricLine): Pair<String, List<IntRange>> =
        if (line.words.isEmpty()) line.text to emptyList() else joinWords(line.words)

    private fun joinWords(words: List<LyricWord>): Pair<String, List<IntRange>> {
        val text = StringBuilder()
        val ranges = ArrayList<IntRange>(words.size)
        words.forEachIndexed { i, word ->
            val start = text.length
            text.append(word.text)
            ranges += start until text.length
            if (!word.joinsNext && i + 1 < words.size) text.append(' ')
        }
        return text.toString() to ranges
    }

    /**
     * The sweep of the active line at [cursor]. Line-timed lines light whole; a word-timed line
     * before its first word is unsung. With [reduceMotion] the whole active line lights at once
     * (tokens: lyrics.word-sweep, Misty MOT-12).
     */
    fun sweep(row: LyricRow, cursor: LyricCursor, reduceMotion: Boolean): Sweep {
        if (reduceMotion || row.wordRanges.isEmpty()) return Sweep.allSung(row)
        val word = cursor.word ?: return Sweep.NoneSung
        val range = row.wordRanges.getOrNull(word) ?: return Sweep.allSung(row)
        val progress = cursor.wordProgress.coerceIn(0f, 1f)
        return if (progress >= 1f) {
            // The word is sung, and the space after it with it.
            val next = row.wordRanges.getOrNull(word + 1)?.first ?: row.text.length
            Sweep(next, null, 0f)
        } else {
            Sweep(range.first, range, progress)
        }
    }

    /**
     * The sweep of the lyrics bar's line from the core's `lyrics_bar` ([bar]): the words sung
     * whole, then the share of the word being sung; a line without word timing fills as one by
     * its line progress. With [reduceMotion] the line lights whole.
     */
    fun barSweep(row: LyricRow, bar: LyricsBar, reduceMotion: Boolean): Sweep {
        if (reduceMotion) return Sweep.allSung(row)
        if (row.wordRanges.isEmpty() || bar.words.isEmpty()) {
            val p = bar.lineProgress.coerceIn(0f, 1f)
            return when {
                p >= 1f -> Sweep.allSung(row)
                p <= 0f || row.text.isEmpty() -> Sweep.NoneSung
                else -> Sweep(0, 0 until row.text.length, p)
            }
        }
        var sungEnd = 0
        for ((i, p) in bar.words.withIndex()) {
            val range = row.wordRanges.getOrNull(i) ?: break
            when {
                // The word is sung, and the space after it with it.
                p >= 1f -> sungEnd = row.wordRanges.getOrNull(i + 1)?.first ?: row.text.length
                p > 0f -> return Sweep(sungEnd, range, p)
                else -> break
            }
        }
        return Sweep(sungEnd, null, 0f)
    }

    /** True when [text] holds Han, Hiragana, Katakana or Hangul characters. */
    fun isCjk(text: String): Boolean {
        var i = 0
        while (i < text.length) {
            val cp = text.codePointAt(i)
            when (Character.UnicodeScript.of(cp)) {
                Character.UnicodeScript.HAN,
                Character.UnicodeScript.HIRAGANA,
                Character.UnicodeScript.KATAKANA,
                Character.UnicodeScript.HANGUL,
                -> return true
                else -> {}
            }
            i += Character.charCount(cp)
        }
        return false
    }

    /**
     * The scroll offset that puts an item of [itemPx] with its centre at [anchor] of a viewport
     * of [viewportPx] (tokens: lyrics.anchor-position, 38 %).
     */
    fun anchorOffsetPx(viewportPx: Int, itemPx: Int, anchor: Float): Int =
        (viewportPx * anchor - itemPx / 2f).toInt().coerceAtLeast(0)
}

/**
 * Follow mode: the view scrolls the active line into place unless the person scrolled within the
 * last [pauseMs] (docs/lyrics-model.md: manual scrolling pauses follow mode for 4 s).
 */
class LyricsFollow(private val pauseMs: Long = 4_000) {
    private var scrolledAtMs: Long? = null

    fun userScrolled(nowMs: Long) {
        scrolledAtMs = nowMs
    }

    fun isFollowing(nowMs: Long): Boolean {
        val at = scrolledAtMs ?: return true
        return nowMs - at >= pauseMs
    }

    /** Milliseconds until follow mode resumes; 0 when following. */
    fun resumesInMs(nowMs: Long): Long {
        val at = scrolledAtMs ?: return 0
        return (pauseMs - (nowMs - at)).coerceAtLeast(0)
    }

    fun reset() {
        scrolledAtMs = null
    }
}

/** A rectangle in the view's pixels. */
data class Box(val left: Float, val top: Float, val right: Float, val bottom: Float) {
    val width: Float get() = right - left
}

/**
 * Geometry of the word sweep: which parts of a word's glyph boxes are lit at a progress. A word
 * that wraps over rows lights row by row, each in proportion to its width; right-to-left lines
 * light from the right.
 */
object SweepGeometry {
    /** Merges character boxes into one box per visual row (boxes sharing a top). */
    fun rows(charBoxes: List<Box>): List<Box> {
        val rows = ArrayList<Box>()
        for (box in charBoxes) {
            val last = rows.lastOrNull()
            if (last != null && kotlin.math.abs(last.top - box.top) < 0.5f) {
                rows[rows.lastIndex] = Box(
                    minOf(last.left, box.left),
                    minOf(last.top, box.top),
                    maxOf(last.right, box.right),
                    maxOf(last.bottom, box.bottom),
                )
            } else {
                rows += box
            }
        }
        return rows
    }

    /** The lit parts of [rows] at [progress] (0 to 1), in reading order. */
    fun lit(rows: List<Box>, progress: Float, rtl: Boolean): List<Box> {
        val total = rows.sumOf { it.width.toDouble() }.toFloat()
        if (total <= 0f || progress <= 0f) return emptyList()
        var remaining = total * progress.coerceAtMost(1f)
        val out = ArrayList<Box>(rows.size)
        for (row in rows) {
            if (remaining <= 0f) break
            val take = minOf(row.width, remaining)
            out += if (rtl) Box(row.right - take, row.top, row.right, row.bottom) else Box(row.left, row.top, row.left + take, row.bottom)
            remaining -= take
        }
        return out
    }
}
