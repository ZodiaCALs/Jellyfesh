package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.QualityLabel
import org.jellyfesh.logic.core.QualityNote
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.StreamingQuality
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.state.CoreState

/** The quality badge and its menu (ADR-0026 §3), on the core's fields. */
class QualityMenuTest {
    private val flac = AudioFormat("flac", 96_000, 24, 2, 4_600, lossless = true, quality = QualityLabel.HiResLossless)
    private val aac = AudioFormat("aac", 44_100, null, 2, 256, lossless = false, quality = QualityLabel.High)
    private val streamed = NowPlaying(
        3, TrackRef.Source(SourceTrack("org.jellyfesh.jellyfin", "t-1", "Morning Harbour")), null, "Morning Harbour", "Glass Harbour", "Slow Tide",
        200_000, null, null,
        qualities = listOf(StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High, StreamingQuality.Normal, StreamingQuality.Low),
        qualityLevel = StreamingQuality.High,
    )
    private val file = NowPlaying(7, TrackRef.Library(7), "/m/7.flac", "Kettle Song", "Juniper Fathom", "Salt", 200_000, null, null, qualityNote = QualityNote.FilesPlayAsTheyAre)
    private fun state(now: NowPlaying?, chosen: StreamingQuality = StreamingQuality.Auto) =
        CoreState(ready = true, nowPlaying = now, settings = Settings(streamingQuality = chosen))

    @Test
    fun theBadgeShowsTheCoresLabelElseTheChosenLevel() {
        assertEquals(QualityBadgeText.Label(QualityLabel.HiResLossless), QualityMenus.of(state(streamed.copy(quality = flac)))?.badge)
        // Before the engine has opened the song, the level asked for.
        assertEquals(QualityBadgeText.Level(StreamingQuality.Normal), QualityMenus.of(state(streamed, StreamingQuality.Normal))?.badge)
        // Nothing plays: no badge.
        assertNull(QualityMenus.of(state(null)))
    }

    @Test
    fun theMenuOffersAutoThenTheSourcesLevelsWithTheChosenOneChecked() {
        val menu = QualityMenus.of(state(streamed.copy(quality = flac), StreamingQuality.High))!!
        assertEquals(
            listOf(StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High, StreamingQuality.Normal, StreamingQuality.Low),
            menu.levels,
        )
        assertEquals(StreamingQuality.High, menu.chosen)
        assertNull(menu.note)
        // A source listing Auto itself, or out of order, still gets Auto once and first.
        assertEquals(
            listOf(StreamingQuality.Auto, StreamingQuality.High, StreamingQuality.Low),
            QualityMenus.levels(listOf(StreamingQuality.High, StreamingQuality.Auto, StreamingQuality.Low, StreamingQuality.High)),
        )
        assertEquals(emptyList(), QualityMenus.levels(listOf(StreamingQuality.Auto)))
    }

    @Test
    fun withNoLevelsItSaysWhy() {
        assertEquals(QualityNote.FilesPlayAsTheyAre, QualityMenus.of(state(file.copy(quality = flac)))?.note)
        assertEquals(emptyList(), QualityMenus.of(state(file))?.levels)
        val service = streamed.copy(qualities = emptyList(), qualityNote = QualityNote.SetInService("Spotify"))
        assertEquals(QualityNote.SetInService("Spotify"), QualityMenus.of(state(service))?.note)
        // A note that names no service, or none at all, says files play as they are.
        assertEquals(QualityNote.FilesPlayAsTheyAre, QualityMenus.note(emptyList(), QualityNote.SetInService(" ")))
        assertEquals(QualityNote.FilesPlayAsTheyAre, QualityMenus.note(emptyList(), null))
    }

    @Test
    fun aRemoteSessionLeavesTheQualityToItsService() {
        val remote = RemoteSnapshot.Empty.copy(version = 2, source = "spotify", sourceName = "Spotify")
        val menu = QualityMenus.of(state(file).copy(remote = remote))!!
        assertEquals(QualityNote.SetInService("Spotify"), menu.note)
        assertEquals(emptyList(), menu.levels)
        assertNull(menu.format)
        // The core's own note wins when it sends one.
        val noted = remote.copy(qualityNote = QualityNote.SetInService("Spotify Connect"))
        assertEquals(QualityNote.SetInService("Spotify Connect"), QualityMenus.of(state(null).copy(remote = noted))?.note)
    }

    @Test
    fun theFormatLineHasTheBitsAndRateForLosslessAndTheBitrateForTheRest() {
        assertEquals(QualityFormat("FLAC", 24, "96 kHz", null), QualityMenus.format(flac))
        assertEquals(QualityFormat("AAC", null, null, 256), QualityMenus.format(aac))
        // Lossless unknown: a bit depth says so.
        assertEquals(QualityFormat("ALAC", 16, "44.1 kHz", null), QualityMenus.format(AudioFormat("alac", 44_100, 16, 2, null, null, null)))
        assertNull(QualityMenus.format(AudioFormat(" ", null, null, null, null, null, null)))
    }

    @Test
    fun choosingALevelIsTheStreamingQualitySetting() {
        val settings = Settings(streamingQuality = StreamingQuality.Auto)
        assertEquals(Command.SetSettings(settings.copy(streamingQuality = StreamingQuality.Low)), QualityMenus.choose(settings, StreamingQuality.Low))
        assertNull(QualityMenus.choose(settings, StreamingQuality.Auto))
    }
}
