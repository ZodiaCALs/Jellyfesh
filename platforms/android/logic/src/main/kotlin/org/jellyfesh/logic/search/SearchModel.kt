package org.jellyfesh.logic.search

import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.SearchUpdate
import org.jellyfesh.logic.core.SourceItem

/** Results of one online source for the current search, as the core sent them. */
data class SourceSection(val source: String, val items: List<SourceItem>, val error: String?)

/**
 * The search screen: library results from the fast synchronous query (`searchLibrary`) as the
 * person types, and every enabled source's results from `Command.Search`, merged and
 * de-duplicated by the core and shown in the order they arrive. The shell only keeps the latest
 * request's results; it never ranks, filters or merges them itself.
 */
data class SearchState(
    val query: String = "",
    /** The query the library results belong to. */
    val libraryQuery: String = "",
    val library: SearchResults = SearchResults.Empty,
    /** The id `send` returned for the unified search, while its results are wanted. */
    val request: Long? = null,
    val sources: List<SourceSection> = emptyList(),
    /** The core sent the last message for [request]. */
    val done: Boolean = true,
    /** Online search answered "not available in this build yet" (or failed). */
    val sourcesUnavailable: Boolean = false,
) {
    val isBlank: Boolean get() = query.isBlank()

    val hasResults: Boolean get() = !library.isEmpty || sources.any { it.items.isNotEmpty() }
}

object SearchModel {
    /** Pause in typing before a search runs. */
    const val DEBOUNCE_MS: Long = 200

    /** Library results per kind for the fast query. */
    const val LIBRARY_LIMIT: Int = 25

    fun queryChanged(state: SearchState, text: String): SearchState =
        if (text.isBlank()) SearchState(query = text) else state.copy(query = text)

    /** The library's results for [query]; stale answers (the person typed on) are dropped. */
    fun libraryResults(state: SearchState, query: String, results: SearchResults): SearchState =
        if (query != state.query) state else state.copy(libraryQuery = query, library = results)

    /** `Command.Search` for [query] was sent with id [request]. */
    fun requestSent(state: SearchState, query: String, request: Long): SearchState =
        if (query != state.query) {
            state
        } else {
            state.copy(request = request, sources = emptyList(), done = request == 0L, sourcesUnavailable = request == 0L)
        }

    /**
     * One source's results. Library results arrive through the fast query, so the unified
     * search's `library` messages only mark progress.
     */
    fun onUpdate(state: SearchState, update: SearchUpdate): SearchState {
        if (update.request != state.request) return state
        val sources = if (update.source == LIBRARY) {
            state.sources
        } else {
            val section = SourceSection(update.source, update.items, update.error)
            val i = state.sources.indexOfFirst { it.source == update.source }
            if (i >= 0) state.sources.toMutableList().also { it[i] = section } else state.sources + section
        }
        return state.copy(sources = sources, done = update.done)
    }

    /** The unified search failed (online search not available in this build, for example). */
    fun onCommandFailed(state: SearchState, id: Long): SearchState =
        if (id != state.request) state else state.copy(done = true, sourcesUnavailable = true)

    /** Items grouped for display by kind, each group in the order the core sent it. */
    fun byKind(items: List<SourceItem>): List<Pair<ItemKind, List<SourceItem>>> =
        ItemKind.entries.mapNotNull { kind -> items.filter { it.kind == kind }.takeIf { it.isNotEmpty() }?.let { kind to it } }

    const val LIBRARY = "library"
}
