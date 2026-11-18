package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.BrowseUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SourceItem

/**
 * A level of a source's catalogue as its page shows it, read page by page with `Command.Browse`:
 * the rows so far, the next page to ask for, the request waiting for its answer (the core answers
 * with a BrowsePage event carrying the id `send` returned) and the last error.
 */
data class BrowseState(
    val items: List<SourceItem> = emptyList(),
    /** The next page to ask for; null once the last page has arrived. */
    val nextPage: Int? = 0,
    /** The `Browse` command waiting for its page. */
    val request: Long? = null,
    /** Why the last page did not come. */
    val error: String? = null,
    /** The command could not be sent: browsing is not in this build. */
    val unavailable: Boolean = false,
) {
    val loading: Boolean get() = request != null

    /** Every page has arrived. */
    val complete: Boolean get() = nextPage == null && request == null

    /** The songs among the rows, in their order. */
    val tracks: List<SourceItem> get() = items.filter { it.kind == ItemKind.Track }

    /** Some song here plays here, so the page offers Play and Shuffle. */
    val playable: Boolean get() = tracks.any { it.playable && it.track != null }

    /** The songs' total length, for "12 songs, 48 minutes". */
    val totalMs: Long get() = tracks.sumOf { it.durationMs ?: 0L }
}

/**
 * A source's catalogue pages (docs/ui.md, "Sources"; the web's browse page is the model): an album
 * or playlist gets the full page (artwork, Play, Shuffle, More, the songs and their total), other
 * levels their title and rows.
 */
object BrowseModel {
    /**
     * Asks for the next page through [send], unless one is on its way or every page has arrived;
     * a command that could not be sent marks browsing unavailable.
     */
    fun loadMore(state: BrowseState, plugin: String, node: String?, send: (Command) -> Long): BrowseState {
        val page = state.nextPage ?: return state
        if (state.request != null) return state
        val id = send(Command.Browse(plugin, node, page))
        return if (id == 0L) state.copy(unavailable = true) else state.copy(request = id, error = null, unavailable = false)
    }

    /** A page arrived: its rows join the list. Answers to other requests are ignored. */
    fun onPage(state: BrowseState, update: BrowseUpdate): BrowseState =
        if (update.request != state.request) {
            state
        } else {
            state.copy(items = state.items + update.items, nextPage = update.nextPage, request = null, error = update.error)
        }

    /** The request failed: the source did not answer. */
    fun onFailed(state: BrowseState, id: Long, message: String): BrowseState =
        if (id != state.request) state else state.copy(request = null, error = message)

    /** Whether a level of [kind] gets the full page with Play and Shuffle: albums and playlists. */
    fun isCollection(kind: ItemKind?): Boolean = kind == ItemKind.Album || kind == ItemKind.Playlist

    /**
     * Whether the page's main action is Open in {Service}: every page has arrived, it has songs,
     * none of them plays here, and it has a link the app may open.
     */
    fun opensLink(state: BrowseState, link: String?): Boolean =
        Links.openable(link) != null && state.complete && state.tracks.isNotEmpty() && !state.playable

    /** Each row's number on an album's page (its place among the songs), or null; other pages are not numbered. */
    fun numbers(state: BrowseState, kind: ItemKind?): List<Int?> {
        if (kind != ItemKind.Album) return state.items.map { null }
        var n = 0
        return state.items.map { if (it.kind == ItemKind.Track) ++n else null }
    }
}
