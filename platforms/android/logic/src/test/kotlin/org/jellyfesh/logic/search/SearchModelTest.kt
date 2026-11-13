package org.jellyfesh.logic.search

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Album
import org.jellyfesh.logic.core.ItemKind
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

    @Test
    fun groupsKeepTheCoresOrderWithinAKind() {
        val items = listOf(item("s", ItemKind.Album, "a"), item("s", ItemKind.Track, "t1"), item("s", ItemKind.Track, "t2"))
        val groups = SearchModel.byKind(items)
        assertEquals(listOf(ItemKind.Track, ItemKind.Album), groups.map { it.first })
        assertEquals(listOf("t1", "t2"), groups[0].second.map { it.id })
    }
}
