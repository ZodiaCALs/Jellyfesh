package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourceLibrary
import org.jellyfesh.logic.core.SourceLibraryEntry
import org.jellyfesh.logic.core.SourceLibraryList
import org.jellyfesh.logic.core.SourceLibraryPage
import org.jellyfesh.logic.core.SourceLibraryPart
import org.jellyfesh.logic.core.SourceLibrarySnapshot

/** The sources' likes and playlists (ADR-0024), from the SourceLibrary slice. */
class SourceLibraryModelTest {
    private val spotify = "org.jellyfesh.spotify"
    private fun entry(id: String, kind: ItemKind = ItemKind.Playlist) =
        SourceLibraryEntry(SourceItem(spotify, kind, id, "Title $id", null, null, null, null, true, null, null, emptyList()), owned = true, songsListed = true)
    private fun list(part: SourceLibraryPart, entries: List<SourceLibraryEntry>, next: String? = null, refreshed: Long? = 1_000, loading: Boolean = false, error: String? = null) =
        SourceLibraryList(part, entries, total = null, next = next, refreshedAtMs = refreshed, loading = loading, error = error)

    private val playlists = list(SourceLibraryPart.Playlists, listOf(entry("p1"), entry("p2")), next = "c-2")
    private val snapshot = SourceLibrarySnapshot(
        4,
        listOf(
            SourceLibrary(
                spotify, "Spotify", "Spotify",
                listOf(
                    list(SourceLibraryPart.LikedArtists, emptyList(), loading = true),
                    list(SourceLibraryPart.LikedSongs, listOf(entry("s1", ItemKind.Track))),
                    list(SourceLibraryPart.LikedAlbums, emptyList()),
                    playlists,
                ),
            ),
            // The M1 core lists signed-in sources with empty lists: nothing to show.
            SourceLibrary("org.jellyfesh.jellyfin", "Jellyfin", null, listOf(list(SourceLibraryPart.Playlists, emptyList()))),
        ),
    )

    @Test
    fun aListShowsOnceItHasEntriesOrSomethingToSay() {
        val likes = SourceLibraryModel.likes(snapshot)
        assertEquals(listOf("Spotify"), likes.map { it.source.name })
        // Liked Songs before Liked Artists, whatever the slice's order; empty Liked Albums is left out.
        assertEquals(listOf(SourceLibraryPart.LikedSongs, SourceLibraryPart.LikedArtists), likes[0].lists.map { it.part })
        assertEquals(listOf(SourceLibraryPart.Playlists), SourceLibraryModel.playlists(snapshot).single().lists.map { it.part })
    }

    @Test
    fun theListsRefreshWhenThePageOpensOrAfterTenMinutes() {
        assertTrue(SourceLibraryModel.refreshes(null, 5_000))
        assertFalse(SourceLibraryModel.refreshes(5_000, 5_000 + SourceLibraryModel.REFRESH_AFTER_MS - 1))
        assertTrue(SourceLibraryModel.refreshes(5_000, 5_000 + SourceLibraryModel.REFRESH_AFTER_MS))
    }

    @Test
    fun showMoreAddsTheNextPageAfterTheFirst() {
        var pages = SourceLibraryPages()
        assertEquals(Command.SourceLibraryMore(spotify, SourceLibraryPart.Playlists, "c-2"), SourceLibraryModel.showMore(spotify, playlists, pages))
        pages = SourceLibraryModel.moreSent(pages, spotify, playlists, 31)
        assertTrue(SourceLibraryModel.loadingMore(pages, spotify, SourceLibraryPart.Playlists))
        assertNull(SourceLibraryModel.showMore(spotify, playlists, pages))
        pages = SourceLibraryModel.onPage(pages, SourceLibraryPage(31, spotify, SourceLibraryPart.Playlists, listOf(entry("p2"), entry("p3")), next = null, error = null))
        assertEquals(listOf("p1", "p2", "p3"), SourceLibraryModel.entries(spotify, playlists, pages).map { it.item.id })
        // No more pages.
        assertNull(SourceLibraryModel.showMore(spotify, playlists, pages))
        // A refreshed first page starts over from its own cursor.
        val refreshed = playlists.copy(refreshedAtMs = 9_000, next = "c-9")
        assertEquals(listOf("p1", "p2"), SourceLibraryModel.entries(spotify, refreshed, pages).map { it.item.id })
        assertEquals(Command.SourceLibraryMore(spotify, SourceLibraryPart.Playlists, "c-9"), SourceLibraryModel.showMore(spotify, refreshed, pages))
    }

    @Test
    fun aSourcePlaylistSavesAsAJellyfeshPlaylistAndSaysWhereItsSongsAre() {
        val followed = snapshot.copy(
            sources = listOf(snapshot.sources[0].copy(lists = listOf(playlists.copy(entries = listOf(entry("p1"), entry("p9").copy(owned = false, songsListed = false)))))),
        )
        assertTrue(SourceLibraryModel.songsOnlyInService(followed, spotify, "p9"))
        assertFalse(SourceLibraryModel.songsOnlyInService(followed, spotify, "p1"))
        // A playlist the slice does not list says nothing.
        assertFalse(SourceLibraryModel.songsOnlyInService(followed, spotify, "p404"))
        val playlist = CollectionRef.Source(spotify, "p1", ItemKind.Playlist)
        assertTrue(SourceLibraryModel.canSaveAsPlaylist(playlist))
        assertFalse(SourceLibraryModel.canSaveAsPlaylist(playlist.copy(kind = ItemKind.Album)))
        assertEquals(Command.SavePlaylistCopy(spotify, "p1", "Road Trip"), SourceLibraryModel.saveAsPlaylist(playlist, "  Road Trip "))
        assertNull(SourceLibraryModel.saveAsPlaylist(playlist, " "))
    }

    @Test
    fun aFailedOrRefusedPageSaysWhy() {
        var pages = SourceLibraryModel.moreSent(SourceLibraryPages(), spotify, playlists, 31)
        pages = SourceLibraryModel.onCommandFailed(pages, 31, "Not available in this build yet.")
        assertEquals("Not available in this build yet.", SourceLibraryModel.error(pages, spotify, SourceLibraryPart.Playlists))
        assertFalse(SourceLibraryModel.loadingMore(pages, spotify, SourceLibraryPart.Playlists))
        // It can be tried again from the same cursor.
        assertEquals(Command.SourceLibraryMore(spotify, SourceLibraryPart.Playlists, "c-2"), SourceLibraryModel.showMore(spotify, playlists, pages))
        pages = SourceLibraryModel.moreSent(pages, spotify, playlists, 32)
        assertNull(SourceLibraryModel.error(pages, spotify, SourceLibraryPart.Playlists))
        pages = SourceLibraryModel.onPage(pages, SourceLibraryPage(32, spotify, SourceLibraryPart.Playlists, emptyList(), null, "The service did not answer."))
        assertEquals("The service did not answer.", SourceLibraryModel.error(pages, spotify, SourceLibraryPart.Playlists))
    }
}
