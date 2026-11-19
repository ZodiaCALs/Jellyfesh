package org.jellyfesh.logic.lyrics

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.SpanTimeline
import org.jellyfesh.logic.core.LyricDoc
import org.jellyfesh.logic.core.LyricFormat
import org.jellyfesh.logic.core.LyricLine
import org.jellyfesh.logic.core.LyricWord
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing

class LyricsBarTest {
    private fun doc(timing: Timing) = LyricDoc(
        language = "en",
        timing = timing,
        offsetMs = 0,
        agents = emptyList(),
        lines = listOf(LyricLine(0, 1_000, 2_000, null, "one two", listOf(LyricWord(1_000, 1_500, "one", false), LyricWord(1_500, 2_000, "two", false)), emptyList(), TextDir.Ltr)),
        tracks = emptyList(),
        format = LyricFormat.EnhancedLrc,
        origin = null,
    )

    private val timeline = SpanTimeline(listOf(SpanTimeline.Span(0, 0, 1_000, 1_500), SpanTimeline.Span(0, 1, 1_500, 2_000)))

    @Test
    fun theBarShowsTimedLyricsOfTheSongPlayingHere() {
        val lyrics = LyricsSnapshot(1, doc(Timing.Word), timeline, null)
        assertTrue(LyricsBars.shows(showLyricsBar = true, lyrics = lyrics, remote = false))
        assertFalse(LyricsBars.shows(showLyricsBar = false, lyrics = lyrics, remote = false), "Show Lyrics Bar is off")
        assertFalse(LyricsBars.shows(true, lyrics, remote = true), "a remote session's song has no lyrics here")
        assertFalse(LyricsBars.shows(true, lyrics.copy(doc = doc(Timing.Unsynced)), false), "unsynced lyrics have no line to follow")
        assertFalse(LyricsBars.shows(true, lyrics.copy(timeline = null), false))
        assertFalse(LyricsBars.shows(true, null, false))
        assertTrue(LyricsBars.shows(true, lyrics.copy(doc = doc(Timing.Line)), false), "line-timed lyrics show too")
    }

    @Test
    fun beforeTheFirstLineTheBarShowsTheOneToCome() {
        assertEquals(0, LyricsBars.shownLine(timeline.bar(500)))
        assertEquals(0, LyricsBars.shownLine(timeline.bar(1_200)))
        assertNull(LyricsBars.shownLine(LyricsBar.Empty))
    }
}
