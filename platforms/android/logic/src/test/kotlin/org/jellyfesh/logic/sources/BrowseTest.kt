package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.BrowseUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.TrackRef

/** A source's catalogue pages, page by page (docs/parity.md N18; the web's browse page). */
class BrowseTest {
    private fun song(id: String, playable: Boolean = true, ms: Long? = 200_000) = SourceItem(
        "nav", ItemKind.Track, id, "Song $id", "Artist", "Album", ms, null, playable,
        if (playable) null else "Not available in your region",
        if (playable) TrackRef.Source(SourceTrack("nav", id, "Song $id")) else null,
        emptyList(),
    )

    private fun folder(id: String) = song(id).copy(kind = ItemKind.Folder, track = null, durationMs = null)

    private val sent = mutableListOf<Command>()
    private fun send(command: Command): Long {
        sent += command
        return 40L + sent.size
    }

    @Test
    fun pagesArriveForTheirRequestAndTheLastOneCompletesThePage() {
        var state = BrowseModel.loadMore(BrowseState(), "nav", "album-1", ::send)
        assertEquals(Command.Browse("nav", "album-1", 0), sent.single())
        assertTrue(state.loading)
        // A second ask while one is on its way sends nothing.
        assertSame(state, BrowseModel.loadMore(state, "nav", "album-1", ::send))
        // Another request's page is not ours.
        assertSame(state, BrowseModel.onPage(state, BrowseUpdate(99, "nav", "album-1", listOf(song("x")), null, null)))
        state = BrowseModel.onPage(state, BrowseUpdate(41, "nav", "album-1", listOf(song("1"), song("2")), nextPage = 1, error = null))
        assertFalse(state.complete)
        state = BrowseModel.loadMore(state, "nav", "album-1", ::send)
        assertEquals(Command.Browse("nav", "album-1", 1), sent.last())
        state = BrowseModel.onPage(state, BrowseUpdate(42, "nav", "album-1", listOf(song("3", ms = null)), nextPage = null, error = null))
        assertTrue(state.complete)
        assertEquals(listOf("1", "2", "3"), state.items.map { it.id })
        assertEquals(400_000L, state.totalMs, "a song without a length adds nothing")
        // Every page is here: nothing more to ask.
        assertSame(state, BrowseModel.loadMore(state, "nav", "album-1", ::send))
    }

    @Test
    fun aFailureEndsTheRequestWithItsMessage() {
        val asked = BrowseModel.loadMore(BrowseState(), "nav", null, ::send)
        assertSame(asked, BrowseModel.onFailed(asked, 7, "elsewhere"))
        val failed = BrowseModel.onFailed(asked, 41, "The server did not answer.")
        assertEquals("The server did not answer.", failed.error)
        assertFalse(failed.loading)
        // A core that cannot browse in this build sends nothing.
        assertTrue(BrowseModel.loadMore(BrowseState(), "nav", null) { 0L }.unavailable)
    }

    @Test
    fun albumsAndPlaylistsGetTheFullPageAndAlbumsNumberTheirSongs() {
        assertTrue(BrowseModel.isCollection(ItemKind.Album))
        assertTrue(BrowseModel.isCollection(ItemKind.Playlist))
        assertFalse(BrowseModel.isCollection(ItemKind.Artist))
        assertFalse(BrowseModel.isCollection(null))
        val page = BrowseState(items = listOf(song("1"), folder("f"), song("2")), nextPage = null)
        assertEquals(listOf(1, null, 2), BrowseModel.numbers(page, ItemKind.Album))
        assertEquals(listOf(null, null, null), BrowseModel.numbers(page, ItemKind.Playlist))
    }

    @Test
    fun aPageWhoseSongsCannotPlayHereOpensItsLink() {
        val locked = BrowseState(items = listOf(song("1", playable = false), song("2", playable = false)), nextPage = null)
        assertFalse(locked.playable)
        assertTrue(BrowseModel.opensLink(locked, "https://music.example/album/1"))
        assertFalse(BrowseModel.opensLink(locked, null))
        assertFalse(BrowseModel.opensLink(locked, "intent://x"), "only https links")
        assertFalse(BrowseModel.opensLink(locked.copy(nextPage = 1), "https://music.example/album/1"), "not before every page arrived")
        val mixed = locked.copy(items = locked.items + song("3"))
        assertTrue(mixed.playable)
        assertFalse(BrowseModel.opensLink(mixed, "https://music.example/album/1"))
    }
}
