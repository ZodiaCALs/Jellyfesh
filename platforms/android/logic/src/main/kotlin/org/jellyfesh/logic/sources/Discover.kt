package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DiscoverPage
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.Shelf

/** A source's Discover page as it loads (docs/ui.md, "Discover"; ADR-0022). */
data class DiscoverState(
    /** The `Discover` command's id: the page answering it is this one. */
    val request: Long = 0,
    val page: DiscoverPage? = null,
    val loading: Boolean = false,
    /** Why the page did not come (the page's error, or the command's failure). */
    val error: String? = null,
)

/** The shelves the core makes for sources without a Discover of their own, worded by id. */
enum class KnownShelf { ForYou, RecentlyAdded, MostPlayed, Rediscover, Browse }

/** A shelf's title: the plugin's words, one the app words, or the source's name (its top level). */
sealed interface ShelfTitle {
    data class Given(val title: String) : ShelfTitle
    data class Known(val shelf: KnownShelf) : ShelfTitle
    data class SourceName(val name: String) : ShelfTitle
}

object DiscoverModel {
    /** Asks for [source]'s page (again, for Try Again). */
    fun load(state: DiscoverState, source: String, send: (Command) -> Long): DiscoverState =
        state.copy(request = send(Command.Discover(source)), loading = true, error = null)

    /** The core's page: this one's when it answers our request. */
    fun onPage(state: DiscoverState, page: DiscoverPage): DiscoverState {
        if (page.request != state.request) return state
        return state.copy(page = page, loading = false, error = page.error?.takeIf { it.isNotBlank() })
    }

    fun onFailed(state: DiscoverState, id: Long, message: String): DiscoverState =
        if (id == state.request && state.loading) state.copy(loading = false, error = message) else state

    /** The shelves to show: those with items. */
    fun shelves(state: DiscoverState): List<Shelf> = state.page?.shelves.orEmpty().filter { it.items.isNotEmpty() }

    /**
     * A shelf's title. The core's own shelves (for a source without `discover`) have stable ids
     * the app words; a plugin's shelves are in its words.
     */
    fun title(shelf: Shelf, plugin: PluginSummary?): ShelfTitle {
        if (plugin?.discover == true) return ShelfTitle.Given(shelf.title)
        return when (shelf.id) {
            "for_you" -> ShelfTitle.Known(KnownShelf.ForYou)
            "recently_added" -> ShelfTitle.Known(KnownShelf.RecentlyAdded)
            "most_played" -> ShelfTitle.Known(KnownShelf.MostPlayed)
            "rediscover" -> ShelfTitle.Known(KnownShelf.Rediscover)
            "browse" -> plugin?.name?.let { ShelfTitle.SourceName(it) } ?: ShelfTitle.Given(shelf.title)
            else -> ShelfTitle.Given(shelf.title)
        }
    }
}
