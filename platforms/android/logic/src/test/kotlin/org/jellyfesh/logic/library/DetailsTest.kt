package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DetailsPage
import org.jellyfesh.logic.core.Fact
import org.jellyfesh.logic.core.FactKey
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.QualityLabel
import org.jellyfesh.logic.core.ReplayGainValues
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.TrackRef

class DetailsTest {
    private fun page(request: Long, facts: List<Fact> = emptyList(), error: String? = null) =
        DetailsPage(request, ItemRef.Track(1), "Song", "Artist", null, null, facts, null, null, null, error)

    @Test
    fun detailsAnswerTheirRequest() {
        val sent = mutableListOf<Command>()
        val state = DetailsModel.load(ItemRef.Track(1)) { sent += it; 5 }
        assertEquals(listOf<Command>(Command.Details(ItemRef.Track(1))), sent)
        assertEquals(state, DetailsModel.onPage(state, page(4)))
        assertEquals(page(5), DetailsModel.onPage(state, page(5)).page)
        assertEquals("gone", DetailsModel.onPage(state, page(5, error = "gone")).error)
        assertEquals("failed", DetailsModel.onFailed(state, 5, "failed").error)
    }

    @Test
    fun songsAreAskedAboutByWhatTheyAre() {
        assertEquals(ItemRef.Track(3), DetailsModel.itemFor(TrackRef.Library(3)))
        assertEquals(ItemRef.File("/m/a.flac"), DetailsModel.itemFor(TrackRef.File("/m/a.flac")))
        assertEquals(ItemRef.Source("subsonic", ItemKind.Track, "tr:9"), DetailsModel.itemFor(TrackRef.Source(SourceTrack("subsonic", "tr:9", "S"))))
    }

    @Test
    fun factsAreFoundByKeyAndTheRestFollow() {
        val facts = listOf(
            Fact(FactKey.Genre, "Jazz"),
            Fact(FactKey.Isrc, "USRC17607839"),
            Fact(FactKey.Quality, "lossless"),
            Fact(FactKey.Other("Mood"), "Calm"),
            Fact(FactKey.Composer, " "),
        )
        assertEquals("Jazz", DetailsModel.fact(page(1, facts), FactKey.Genre))
        assertNull(DetailsModel.fact(page(1, facts), FactKey.Composer))
        assertEquals(listOf(FactKey.Isrc, FactKey.Other("Mood")), DetailsModel.moreFacts(page(1, facts)).map { it.key })
    }

    @Test
    fun formatAndGainsAreWrittenOutForTheApp() {
        val format = AudioFormat("flac", 96_000, 24, 2, 2_300, true, QualityLabel.HiResLossless)
        assertEquals(FormatParts("FLAC", 24, "96 kHz", 2), DetailsModel.formatParts(format))
        assertEquals(FormatParts(null, null, "44.1 kHz", null), DetailsModel.formatParts(AudioFormat(" ", 44_100, 0, 0, null, null, null)))
        assertEquals("−6.2 dB", DetailsModel.gain(-6.2f))
        assertEquals("+1.5 dB", DetailsModel.gain(1.5f))
        assertEquals("−6.2 dB" to null, DetailsModel.gains(ReplayGainValues(-6.2f, 0.98f, null, null)))
    }
}
