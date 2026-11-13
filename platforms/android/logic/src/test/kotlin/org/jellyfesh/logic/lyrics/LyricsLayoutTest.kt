package org.jellyfesh.logic.lyrics

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AuxKind
import org.jellyfesh.logic.core.AuxLine
import org.jellyfesh.logic.core.AuxTrack
import org.jellyfesh.logic.core.LyricDoc
import org.jellyfesh.logic.core.LyricFormat
import org.jellyfesh.logic.core.LyricLine
import org.jellyfesh.logic.core.LyricWord
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.time.LyricCursor

class LyricsLayoutTest {
    private fun word(text: String, start: Long, end: Long, joins: Boolean = false) = LyricWord(start, end, text, joins)

    private fun line(id: Int, text: String, words: List<LyricWord> = emptyList(), background: List<LyricWord> = emptyList()) =
        LyricLine(id, id * 1_000L, id * 1_000L + 900, null, text, words, background, TextDir.Auto)

    private fun doc(lines: List<LyricLine>, tracks: List<AuxTrack> = emptyList(), timing: Timing = Timing.Word) =
        LyricDoc("en", timing, 0, emptyList(), lines, tracks, LyricFormat.Ttml, null)

    @Test
    fun wordTimedLinesAreBuiltFromTheirWords() {
        val row = LyricsLayout.rows(
            doc(listOf(line(0, "ignored", listOf(word("Every", 0, 100), word("rope", 100, 200), word("and", 200, 300))))),
            showTranslation = true,
            showRomanization = true,
        ).single()
        assertEquals("Every rope and", row.text)
        assertEquals(listOf(0..4, 6..9, 11..13), row.wordRanges)
    }

    @Test
    fun joinedWordsHaveNoSpace() {
        val (text, ranges) = LyricsLayout.displayText(line(0, "", listOf(word("清", 0, 1, joins = true), word("晨", 1, 2))))
        assertEquals("清晨", text)
        assertEquals(listOf(0..0, 1..1), ranges)
    }

    @Test
    fun auxiliaryTracksFollowTheSettings() {
        val tracks = listOf(
            AuxTrack(AuxKind.Translation, "zh-Hans", mapOf(0 to AuxLine("清晨", emptyList()))),
            AuxTrack(AuxKind.Romanization, "ja-Latn", mapOf(0 to AuxLine("asa", emptyList()), 1 to AuxLine(" ", emptyList()))),
        )
        val d = doc(listOf(line(0, "Morning"), line(1, "Night")), tracks)
        val shown = LyricsLayout.rows(d, showTranslation = true, showRomanization = true)
        assertEquals("清晨", shown[0].translation)
        assertEquals("asa", shown[0].romanization)
        assertNull(shown[1].romanization, "blank lines are dropped")
        val hidden = LyricsLayout.rows(d, showTranslation = false, showRomanization = false)
        assertNull(hidden[0].translation)
        assertNull(hidden[0].romanization)
    }

    @Test
    fun backgroundVocalsAndGaps() {
        val rows = LyricsLayout.rows(doc(listOf(line(0, "Main", background = listOf(word("ooh", 0, 1))), line(1, ""))), true, true)
        assertEquals("ooh", rows[0].background)
        assertTrue(rows[1].isGap)
        assertFalse(LyricsLayout.rows(doc(listOf(line(0, "")), timing = Timing.Unsynced), true, true)[0].isGap)
    }

    @Test
    fun theSweepFollowsTheCursor() {
        val row = LyricsLayout.rows(doc(listOf(line(0, "", listOf(word("Every", 0, 100), word("rope", 100, 200))))), true, true)[0]
        assertEquals(Sweep.NoneSung, LyricsLayout.sweep(row, LyricCursor(0, null, 0f), reduceMotion = false))
        assertEquals(Sweep(0, 0..4, 0.5f), LyricsLayout.sweep(row, LyricCursor(0, 0, 0.5f), reduceMotion = false))
        // A finished word lights the space after it too.
        assertEquals(Sweep(6, null, 0f), LyricsLayout.sweep(row, LyricCursor(0, 0, 1f), reduceMotion = false))
        assertEquals(Sweep(10, null, 0f), LyricsLayout.sweep(row, LyricCursor(0, 1, 1f), reduceMotion = false))
        // Reduced motion lights the whole active line at once.
        assertEquals(Sweep.allSung(row), LyricsLayout.sweep(row, LyricCursor(0, 0, 0.1f), reduceMotion = true))
    }

    @Test
    fun lineTimedLinesLightWhole() {
        val row = LyricsLayout.rows(doc(listOf(line(0, "Hold still")), timing = Timing.Line), true, true)[0]
        assertEquals(Sweep(10, null, 0f), LyricsLayout.sweep(row, LyricCursor(0, null, 0f), reduceMotion = false))
    }

    @Test
    fun cjkDetection() {
        assertTrue(LyricsLayout.isCjk("清晨把港湾"))
        assertTrue(LyricsLayout.isCjk("あさ"))
        assertTrue(LyricsLayout.isCjk("아침"))
        assertFalse(LyricsLayout.isCjk("Morning folds the harbour"))
        assertFalse(LyricsLayout.isCjk("صباح"))
    }

    @Test
    fun anchorOffsetCentresTheLine() {
        assertEquals(354, LyricsLayout.anchorOffsetPx(1_000, 52, 0.38f))
        assertEquals(0, LyricsLayout.anchorOffsetPx(100, 400, 0.38f))
    }

    @Test
    fun followModePausesForFourSeconds() {
        val follow = LyricsFollow()
        assertTrue(follow.isFollowing(0))
        follow.userScrolled(10_000)
        assertFalse(follow.isFollowing(13_999))
        assertEquals(1, follow.resumesInMs(13_999))
        assertTrue(follow.isFollowing(14_000))
    }

    @Test
    fun sweepGeometryLightsRowsInReadingOrder() {
        val boxes = listOf(Box(0f, 0f, 10f, 20f), Box(10f, 0f, 20f, 20f), Box(0f, 30f, 10f, 50f))
        val rows = SweepGeometry.rows(boxes)
        assertEquals(listOf(Box(0f, 0f, 20f, 20f), Box(0f, 30f, 10f, 50f)), rows)
        // Half of 30 px of width: the whole first row's 15 px.
        assertEquals(listOf(Box(0f, 0f, 15f, 20f)), SweepGeometry.lit(rows, 0.5f, rtl = false))
        assertEquals(listOf(Box(0f, 0f, 20f, 20f), Box(0f, 30f, 5f, 50f)), SweepGeometry.lit(rows, 25f / 30f, rtl = false))
        // Right to left lights from the right edge.
        assertEquals(listOf(Box(5f, 0f, 20f, 20f)), SweepGeometry.lit(rows, 0.5f, rtl = true))
        assertEquals(emptyList(), SweepGeometry.lit(rows, 0f, rtl = false))
    }
}
