package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef

class OpenedFilesTest {
    @Test
    fun copiesKeepTheirNamesSafelyInThePickedOrder() {
        assertEquals("1 Morning Harbour.flac", OpenedFiles.copyName(0, "Morning Harbour.flac"))
        assertEquals("2 AC_DC _ Live.mp3", OpenedFiles.copyName(1, "AC/DC : Live.mp3"))
        assertEquals("3 Song", OpenedFiles.copyName(2, null))
    }

    @Test
    fun thePickedFilesPlayFromTheFirstAndNothingPlaysWithoutThem() {
        assertEquals(
            Command.PlayTracks(listOf(TrackRef.File("/storage/emulated/0/Music/a.flac"), TrackRef.File("/cache/opened/2 b.mp3")), 0),
            OpenedFiles.command(listOf("/storage/emulated/0/Music/a.flac", "/cache/opened/2 b.mp3")),
        )
        assertNull(OpenedFiles.command(emptyList()))
    }
}
