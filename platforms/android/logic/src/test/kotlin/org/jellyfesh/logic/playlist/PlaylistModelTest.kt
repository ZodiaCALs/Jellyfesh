package org.jellyfesh.logic.playlist

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Command

class PlaylistReorderTest {
    private val row = 60f

    @Test
    fun aDropMovesTheEntryToTheRowItLandsOn() {
        val drag = PlaylistDrag(entryId = 41, from = 1, offsetPx = 130f)
        val (pending, command) = PlaylistReorder.drop(playlist = 7, drag = drag, rowPx = row, count = 5, revision = 12)!!
        assertEquals(Command.MovePlaylistEntry(7, 41, 3), command)
        assertEquals(PendingMove(from = 1, to = 3, revision = 12), pending)
        // Far past the end lands on the last row.
        assertEquals(4, PlaylistReorder.target(drag.copy(offsetPx = 9_999f), row, 5))
    }

    @Test
    fun aDropInPlaceSendsNothing() {
        assertNull(PlaylistReorder.drop(7, PlaylistDrag(41, 2, 20f), row, 5, 12))
    }

    @Test
    fun theRowsBetweenMakeRoomWhileDragging() {
        val down = PlaylistDrag(41, from = 1, offsetPx = 125f) // lands on row 3
        assertEquals(125f, PlaylistReorder.shift(1, down, row, 5))
        assertEquals(-row, PlaylistReorder.shift(2, down, row, 5))
        assertEquals(-row, PlaylistReorder.shift(3, down, row, 5))
        assertEquals(0f, PlaylistReorder.shift(4, down, row, 5))
        val up = PlaylistDrag(41, from = 3, offsetPx = -125f) // lands on row 1
        assertEquals(row, PlaylistReorder.shift(1, up, row, 5))
        assertEquals(row, PlaylistReorder.shift(2, up, row, 5))
        assertEquals(0f, PlaylistReorder.shift(0, up, row, 5))
        assertEquals(0f, PlaylistReorder.shift(2, null, row, 5))
    }

    @Test
    fun theMoveShowsUntilRowsReadAfterTheChangeArrive() {
        val pending = PendingMove(from = 1, to = 3, revision = 12)
        // Rows 0..4 drawn from the loaded rows: 0, 2, 3, 1, 4.
        assertEquals(listOf(0, 2, 3, 1, 4), (0..4).map { PlaylistReorder.rowAt(it, pending) })
        assertEquals(listOf(0, 3, 1, 2, 4), (0..4).map { PlaylistReorder.rowAt(it, PendingMove(3, 1, 12)) })
        assertEquals(listOf(0, 1, 2), (0..2).map { PlaylistReorder.rowAt(it, null) })
        assertSame(pending, PlaylistReorder.stillPending(pending, rowsRevision = 12))
        assertSame(pending, PlaylistReorder.stillPending(pending, rowsRevision = 11))
        assertNull(PlaylistReorder.stillPending(pending, rowsRevision = 13))
    }

    @Test
    fun talkBackMovesOneRowAtATime() {
        assertEquals(Command.MovePlaylistEntry(7, 41, 1), PlaylistReorder.step(7, 41, index = 2, by = -1, count = 5))
        assertEquals(Command.MovePlaylistEntry(7, 41, 3), PlaylistReorder.step(7, 41, index = 2, by = 1, count = 5))
        assertNull(PlaylistReorder.step(7, 41, index = 0, by = -1, count = 5))
        assertNull(PlaylistReorder.step(7, 41, index = 4, by = 1, count = 5))
    }
}

class PlaylistFilesTest {
    @Test
    fun exportSuggestsTheNameAsM3u8() {
        assertEquals("Road Trip.m3u8", PlaylistFiles.exportName("Road Trip"))
        assertEquals("AC_DC _ Live_.m3u8", PlaylistFiles.exportName("AC/DC : Live?"))
        assertEquals("Playlist.m3u8", PlaylistFiles.exportName(" ... "))
        assertEquals("audio/x-mpegurl", PlaylistFiles.EXPORT_TYPE)
        assertTrue(PlaylistFiles.EXPORT_TYPE in PlaylistFiles.IMPORT_TYPES)
    }

    @Test
    fun importKeepsTheFilesNameSoThePlaylistIsNamedAfterIt() {
        assertEquals("Evening.m3u8", PlaylistFiles.importName("Evening.m3u8"))
        assertEquals("Old.M3U", PlaylistFiles.importName("Old.M3U"))
        assertEquals("Mix.m3u8", PlaylistFiles.importName("Mix"))
        assertEquals("Playlist.m3u8", PlaylistFiles.importName(null))
        assertEquals("a_b.m3u8", PlaylistFiles.importName("a/b.m3u8"))
    }

    @Test
    fun aPickedFileIsReadWhereItIsWhenItHasAPath() {
        assertEquals("/storage/emulated/0/Music/Mix.m3u8", PlaylistFiles.pathOfDocument("primary:Music/Mix.m3u8"))
        assertEquals("/storage/1A2B-3C4D/Lists/Mix.m3u", PlaylistFiles.pathOfDocument("1A2B-3C4D:Lists/Mix.m3u"))
        assertEquals("/storage/emulated/0/Download/Mix.m3u8", PlaylistFiles.pathOfDocument("raw:/storage/emulated/0/Download/Mix.m3u8"))
        // Other providers (downloads by number, cloud drives) are copied instead.
        assertNull(PlaylistFiles.pathOfDocument("msf:1234"))
        assertNull(PlaylistFiles.pathOfDocument("1234"))
        assertNull(PlaylistFiles.pathOfDocument("primary:"))
        assertNull(PlaylistFiles.pathOfDocument("raw:relative/name"))
    }
}
