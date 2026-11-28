package org.jellyfesh.logic.search

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.test.assertNull
import org.jellyfesh.logic.core.Album
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SearchGroup
import org.jellyfesh.logic.core.SearchPage
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.SearchUpdate
import org.jellyfesh.logic.core.SourceItem

class SearchModelTest {
    private fun item(source: String, kind: ItemKind, id: String) =
        SourceItem(source, kind, id, "Title $id", null, null, null, null, true, null, null, emptyList())

    private fun update(request: Long, source: String, items: List<SourceItem>, done: Boolean = false, error: String? = null) =
        SearchUpdate(request, "tide", source, items, error, done)

    @Test
    fun aBlankQueryClearsEverything() {
        val state = SearchState(query = "tide", request = 3, sources = listOf(SourceSection("x", emptyList(), null)))
        assertEquals(SearchState(query = " "), SearchModel.queryChanged(state, " "))
    }

    @Test
    fun theTabsAreAllLibraryThenEachSourceThatAnswered() {
        var state = SearchModel.requestSent(SearchModel.queryChanged(SearchState(), "tide"), "tide", 7)
        assertEquals(listOf<SearchTab>(SearchTab.All, SearchTab.Library), SearchModel.tabs(state))
        state = SearchModel.onUpdate(state, update(7, "org.jellyfesh.subsonic", listOf(item("s", ItemKind.Track, "1"))))
        state = SearchModel.onUpdate(state, update(7, "org.jellyfesh.jellyfin", emptyList(), error = "timeout"))
        assertEquals(
            listOf(SearchTab.All, SearchTab.Library, SearchTab.Source("org.jellyfesh.subsonic"), SearchTab.Source("org.jellyfesh.jellyfin")),
            SearchModel.tabs(state),
        )
        assertEquals("timeout", SearchModel.sourceTab(state, "org.jellyfesh.jellyfin")?.error)
    }

    @Test
    fun aChosenSourceTabWaitsForItsSourceAndFallsBackToAll() {
        var state = SearchModel.requestSent(SearchModel.queryChanged(SearchState(), "tide"), "tide", 7)
        state = SearchModel.onUpdate(state, update(7, "org.jellyfesh.subsonic", listOf(item("s", ItemKind.Track, "1"))))
        state = SearchModel.selectTab(state, SearchTab.Source("org.jellyfesh.subsonic"))
        assertEquals(SearchTab.Source("org.jellyfesh.subsonic"), SearchModel.shownTab(state))
        // A new query: until the source answers again, All shows; then its tab comes back.
        state = SearchModel.requestSent(SearchModel.queryChanged(state, "tides"), "tides", 8)
        assertEquals(SearchTab.All, SearchModel.shownTab(state))
        state = SearchModel.onUpdate(state, update(8, "org.jellyfesh.subsonic", listOf(item("s", ItemKind.Album, "2"))))
        assertEquals(SearchTab.Source("org.jellyfesh.subsonic"), SearchModel.shownTab(state))
        // Library stays chosen across queries.
        state = SearchModel.selectTab(state, SearchTab.Library)
        assertEquals(SearchTab.Library, SearchModel.shownTab(SearchModel.queryChanged(state, "")))
    }

    @Test
    fun staleLibraryAnswersAreDropped() {
        var state = SearchModel.queryChanged(SearchState(), "tid")
        state = SearchModel.queryChanged(state, "tide")
        val results = SearchResults(emptyList(), listOf(Album(1, "Tide", null, null, 9, null)), emptyList())
        assertTrue(SearchModel.libraryResults(state, "tid", results).library.isEmpty)
        assertEquals(results, SearchModel.libraryResults(state, "tide", results).library)
    }

    @Test
    fun sourcesArriveInOrderAndReplaceThemselves() {
        var state = SearchModel.requestSent(SearchModel.queryChanged(SearchState(), "tide"), "tide", 7)
        assertFalse(state.done)
        state = SearchModel.onUpdate(state, update(7, "org.jellyfesh.subsonic", listOf(item("s", ItemKind.Track, "1"))))
        state = SearchModel.onUpdate(state, update(7, "library", listOf(item("library", ItemKind.Track, "9"))))
        state = SearchModel.onUpdate(state, update(6, "org.jellyfesh.jellyfin", listOf(item("j", ItemKind.Album, "2"))))
        state = SearchModel.onUpdate(state, update(7, "org.jellyfesh.jellyfin", emptyList(), done = true, error = "timeout"))
        assertEquals(listOf("org.jellyfesh.subsonic", "org.jellyfesh.jellyfin"), state.sources.map { it.source })
        assertEquals("timeout", state.sources[1].error)
        assertTrue(state.done)
    }

    @Test
    fun aFailedSearchMarksSourcesUnavailable() {
        var state = SearchModel.requestSent(SearchModel.queryChanged(SearchState(), "tide"), "tide", 4)
        state = SearchModel.onCommandFailed(state, 3)
        assertFalse(state.sourcesUnavailable)
        state = SearchModel.onCommandFailed(state, 4)
        assertTrue(state.sourcesUnavailable)
        assertTrue(state.done)
        // After shutdown `send` returns 0: nothing will answer.
        assertTrue(SearchModel.requestSent(state, "tide", 0).sourcesUnavailable)
    }

    // ---- search by source (ADR-0024 §4) -----------------------------------------------------------

    private val subsonic = "org.jellyfesh.subsonic"
    private val spotify = "org.jellyfesh.spotify"

    private fun searched(vararg updates: SearchUpdate): SearchState =
        updates.fold(SearchModel.requestSent(SearchModel.queryChanged(SearchState(), "tide"), "tide", 7)) { state, update -> SearchModel.onUpdate(state, update) }

    private val bySource = SearchUpdate(
        7, "tide", subsonic, listOf(item(subsonic, ItemKind.Track, "1")), null, false, merged = true,
        groups = listOf(
            SearchGroup(ItemKind.Track, listOf(item(subsonic, ItemKind.Track, "1"), item(subsonic, ItemKind.Track, "2")), next = "c-2", total = 40),
            SearchGroup(ItemKind.Album, emptyList(), next = null, total = 0),
        ),
    )
    private val apart = SearchUpdate(
        7, "tide", spotify, listOf(item(spotify, ItemKind.Track, "9")), null, true, merged = false,
        groups = listOf(SearchGroup(ItemKind.Track, listOf(item(spotify, ItemKind.Track, "9")), next = null, total = 1)),
    )

    @Test
    fun allMergesTheSourcesThatAllowItAndLinksToTheOthers() {
        val state = searched(bySource, apart)
        assertEquals(listOf(subsonic), SearchModel.mergedSections(state).map { it.source })
        assertEquals(listOf(spotify), SearchModel.separateSections(state).map { it.source })
        // Every source that answered has a tab.
        assertEquals(listOf(SearchTab.All, SearchTab.Library, SearchTab.Source(subsonic), SearchTab.Source(spotify)), SearchModel.tabs(state))
        // A source's tab shows its groups with results.
        assertEquals(listOf(ItemKind.Track), SearchModel.sourceTab(state, subsonic)?.tabGroups?.map { it.kind })
        // A core that sends no groups: its rows by kind.
        val plain = searched(update(7, subsonic, listOf(item(subsonic, ItemKind.Album, "a"), item(subsonic, ItemKind.Track, "t"))))
        assertEquals(listOf(ItemKind.Track, ItemKind.Album), SearchModel.sourceTab(plain, subsonic)?.tabGroups?.map { it.kind })
    }

    @Test
    fun fromYourSourcesHeadsOnlyWhatIsUnderIt() {
        // A merging source's results, or its failure: the heading.
        assertTrue(SearchModel.showsFromSources(searched(bySource, apart), unavailable = false))
        assertTrue(SearchModel.showsFromSources(searched(update(7, subsonic, emptyList(), error = "Enter your server's address.")), unavailable = false))
        // Only a separate source's link, or a merging source that found nothing: no heading.
        assertFalse(SearchModel.showsFromSources(searched(apart), unavailable = false))
        assertFalse(SearchModel.showsFromSources(searched(update(7, subsonic, emptyList(), done = true)), unavailable = false))
        assertFalse(SearchModel.showsFromSources(searched(), unavailable = false))
        // The note that the sources could not be searched has it.
        assertTrue(SearchModel.showsFromSources(searched(), unavailable = true))
    }

    @Test
    fun showMoreAddsTheNextPageOnceAndKeepsItsCursor() {
        var state = searched(bySource)
        val command = SearchModel.showMore(state, subsonic, ItemKind.Track)
        assertEquals(Command.SearchMore(subsonic, ItemKind.Track, "c-2"), command)
        state = SearchModel.moreSent(state, subsonic, ItemKind.Track, 12)
        assertTrue(SearchModel.loadingMore(state, subsonic, ItemKind.Track))
        // One at a time, and none for a group without a cursor.
        assertNull(SearchModel.showMore(state, subsonic, ItemKind.Track))
        assertNull(SearchModel.showMore(state, subsonic, ItemKind.Album))
        // The page's rows join the group (a row it already shows only once), with the new cursor.
        val page = SearchPage(12, "tide", subsonic, ItemKind.Track, listOf(item(subsonic, ItemKind.Track, "2"), item(subsonic, ItemKind.Track, "3")), next = "c-3", error = null)
        state = SearchModel.onPage(state, page)
        val tracks = SearchModel.sourceTab(state, subsonic)!!.groups.first { it.kind == ItemKind.Track }
        assertEquals(listOf("1", "2", "3"), tracks.items.map { it.id })
        assertEquals("c-3", tracks.next)
        assertFalse(SearchModel.loadingMore(state, subsonic, ItemKind.Track))
        // The same answer again changes nothing.
        assertEquals(state, SearchModel.onPage(state, page))
    }

    @Test
    fun aFailedShowMoreSaysWhyUnderItsGroupAndCanBeTriedAgain() {
        var state = SearchModel.moreSent(searched(bySource), subsonic, ItemKind.Track, 12)
        state = SearchModel.onPage(state, SearchPage(12, "tide", subsonic, ItemKind.Track, emptyList(), next = null, error = "The server did not answer."))
        assertEquals("The server did not answer.", state.moreErrors[SearchGroupKey(subsonic, ItemKind.Track)])
        assertEquals(Command.SearchMore(subsonic, ItemKind.Track, "c-2"), SearchModel.showMore(state, subsonic, ItemKind.Track))
        // A refused command fails it too; trying again clears the reason.
        state = SearchModel.moreSent(state, subsonic, ItemKind.Track, 13)
        assertNull(state.moreErrors[SearchGroupKey(subsonic, ItemKind.Track)])
        state = SearchModel.onCommandFailed(state, 13, "Not available in this build yet.")
        assertEquals("Not available in this build yet.", state.moreErrors[SearchGroupKey(subsonic, ItemKind.Track)])
        // A new search forgets them.
        assertEquals(emptyMap(), SearchModel.requestSent(state, "tide", 20).moreErrors)
    }

    @Test
    fun groupsKeepTheCoresOrderWithinAKind() {
        val items = listOf(item("s", ItemKind.Album, "a"), item("s", ItemKind.Track, "t1"), item("s", ItemKind.Track, "t2"))
        val groups = SearchModel.byKind(items)
        assertEquals(listOf(ItemKind.Track, ItemKind.Album), groups.map { it.first })
        assertEquals(listOf("t1", "t2"), groups[0].second.map { it.id })
    }
}
