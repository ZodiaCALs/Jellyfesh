package org.jellyfesh.logic.search

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SearchGroup
import org.jellyfesh.logic.core.SearchPage
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.SearchUpdate
import org.jellyfesh.logic.core.SourceItem

/** Results of one online source for the current search, as the core sent them. */
data class SourceSection(
    val source: String,
    /** The All tab's rows: with [merged], its results less those an earlier section shows. */
    val items: List<SourceItem>,
    val error: String?,
    /**
     * Part of the merged results (From Your Sources). False for sources whose terms forbid merging:
     * never shown beside other rows; the All tab links to the source's own tab ("Results from Spotify").
     */
    val merged: Boolean = true,
    /** Its results kind by kind, for its tab, each with Show More's cursor. */
    val groups: List<SearchGroup> = emptyList(),
) {
    /** What its tab shows: its groups with results, else its rows by kind (a core that sends no groups). */
    val tabGroups: List<SearchGroup>
        get() = if (groups.isNotEmpty()) {
            groups.filter { it.items.isNotEmpty() }
        } else {
            SearchModel.byKind(items).map { (kind, rows) -> SearchGroup(kind, rows, next = null, total = null) }
        }
}

/** One kind of one source's results: what Show More pages. */
data class SearchGroupKey(val source: String, val kind: ItemKind)

/**
 * A tab of the Search page (docs/ui.md, "Search"; ADR-0024 §4): All, Library, and one per source
 * that answered.
 */
sealed interface SearchTab {
    /** The library's results, then From Your Sources. */
    data object All : SearchTab

    /** The library's results alone. */
    data object Library : SearchTab

    /** One source's results by kind. */
    data class Source(val source: String) : SearchTab
}

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
    /** The tab the person chose; [SearchModel.shownTab] is the one shown. */
    val tab: SearchTab = SearchTab.All,
    /** Show More on its way: each group's request id. */
    val more: Map<SearchGroupKey, Long> = emptyMap(),
    /** Why a group's last Show More failed, shown under it. */
    val moreErrors: Map<SearchGroupKey, String> = emptyMap(),
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
        if (text.isBlank()) SearchState(query = text, tab = state.tab) else state.copy(query = text)

    /** The tabs over the results: All, Library, then each source that answered, in the order they did. */
    fun tabs(state: SearchState): List<SearchTab> =
        listOf(SearchTab.All, SearchTab.Library) + state.sources.map { SearchTab.Source(it.source) }

    /** The tab shown: the one chosen while it is still there (a source answers each query anew), else All. */
    fun shownTab(state: SearchState): SearchTab = state.tab.takeIf { it in tabs(state) } ?: SearchTab.All

    fun selectTab(state: SearchState, tab: SearchTab): SearchState = state.copy(tab = tab)

    /** A source's tab: its results by kind, Songs, Albums, Artists and Playlists first. */
    fun sourceTab(state: SearchState, source: String): SourceSection? = state.sources.firstOrNull { it.source == source }

    /** The library's results for [query]; stale answers (the person typed on) are dropped. */
    fun libraryResults(state: SearchState, query: String, results: SearchResults): SearchState =
        if (query != state.query) state else state.copy(libraryQuery = query, library = results)

    /** `Command.Search` for [query] was sent with id [request]. */
    fun requestSent(state: SearchState, query: String, request: Long): SearchState =
        if (query != state.query) {
            state
        } else {
            state.copy(
                request = request,
                sources = emptyList(),
                done = request == 0L,
                sourcesUnavailable = request == 0L,
                more = emptyMap(),
                moreErrors = emptyMap(),
            )
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
            val section = SourceSection(update.source, update.items, update.error, update.merged, update.groups)
            val i = state.sources.indexOfFirst { it.source == update.source }
            if (i >= 0) state.sources.toMutableList().also { it[i] = section } else state.sources + section
        }
        return state.copy(sources = sources, done = update.done)
    }

    /** The unified search failed (online search not available in this build, for example). */
    fun onCommandFailed(state: SearchState, id: Long): SearchState =
        if (id != state.request) state else state.copy(done = true, sourcesUnavailable = true)

    /** A command failed: the unified search, or a Show More, whose reason ([error]) shows under its group. */
    fun onCommandFailed(state: SearchState, id: Long, error: String): SearchState {
        val key = pending(state, id) ?: return onCommandFailed(state, id)
        return state.copy(more = state.more - key, moreErrors = state.moreErrors + (key to error))
    }

    /**
     * From Your Sources on the All tab: the sources that allow merging and found something or
     * failed, in the order they answered.
     */
    fun mergedSections(state: SearchState): List<SourceSection> = state.sources.filter { it.merged && (it.items.isNotEmpty() || it.error != null) }

    /**
     * Whether the All tab shows the From Your Sources heading: over a merging source's results or
     * failure, or the note that the sources could not be searched ([unavailable]); never over
     * nothing, nor over the links to the separate sources' tabs alone (docs/ui.md, "Search").
     */
    fun showsFromSources(state: SearchState, unavailable: Boolean): Boolean = unavailable || mergedSections(state).isNotEmpty()

    /** The sources whose terms forbid merging and that found something: the All tab links to their tabs. */
    fun separateSections(state: SearchState): List<SourceSection> = state.sources.filter { !it.merged && it.tabGroups.isNotEmpty() }

    /**
     * Show More for one kind of a source's results: the command for the group's next page of the
     * newest search, or null when it has no more or one is on its way.
     */
    fun showMore(state: SearchState, source: String, kind: ItemKind): Command.SearchMore? {
        if (SearchGroupKey(source, kind) in state.more) return null
        val cursor = sourceTab(state, source)?.groups?.firstOrNull { it.kind == kind }?.next ?: return null
        return Command.SearchMore(source, kind, cursor)
    }

    /** Show More was sent with id [request] (0 when the core is not running: nothing is on its way). */
    fun moreSent(state: SearchState, source: String, kind: ItemKind, request: Long): SearchState {
        if (request == 0L) return state
        val key = SearchGroupKey(source, kind)
        return state.copy(more = state.more + (key to request), moreErrors = state.moreErrors - key)
    }

    /** Whether a group's Show More is on its way. */
    fun loadingMore(state: SearchState, source: String, kind: ItemKind): Boolean = SearchGroupKey(source, kind) in state.more

    /**
     * Show More's answer: its rows join their group once, and the group keeps the new cursor; a
     * failure shows under the group, which keeps its cursor to try again. Answers to requests the
     * state no longer waits for (a newer search) are dropped.
     */
    fun onPage(state: SearchState, page: SearchPage): SearchState {
        val key = pending(state, page.request) ?: return state
        val more = state.more - key
        if (page.error != null) return state.copy(more = more, moreErrors = state.moreErrors + (key to page.error))
        val sources = state.sources.map { section ->
            if (section.source != key.source) {
                section
            } else {
                section.copy(
                    groups = section.groups.map { group ->
                        if (group.kind != key.kind) {
                            group
                        } else {
                            val shown = group.items.mapTo(HashSet()) { it.id }
                            group.copy(items = group.items + page.items.filter { shown.add(it.id) }, next = page.next)
                        }
                    },
                )
            }
        }
        return state.copy(sources = sources, more = more, moreErrors = state.moreErrors - key)
    }

    private fun pending(state: SearchState, request: Long): SearchGroupKey? = state.more.entries.firstOrNull { it.value == request }?.key

    /** Items grouped for display by kind, each group in the order the core sent it. */
    fun byKind(items: List<SourceItem>): List<Pair<ItemKind, List<SourceItem>>> =
        ItemKind.entries.mapNotNull { kind -> items.filter { it.kind == kind }.takeIf { it.isNotEmpty() }?.let { kind to it } }

    const val LIBRARY = "library"
}
