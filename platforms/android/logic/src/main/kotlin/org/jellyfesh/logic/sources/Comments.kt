package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CommentEntry
import org.jellyfesh.logic.core.CommentsPage
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef

/** A song's comments as they load page by page (docs/ui.md, "Comments"; ADR-0022). Read-only, never stored. */
data class CommentsState(
    val item: ItemRef? = null,
    /** The `Comments` command waiting for its page, 0 when none. */
    val request: Long = 0,
    val hot: List<CommentEntry> = emptyList(),
    val comments: List<CommentEntry> = emptyList(),
    val total: Long? = null,
    val next: String? = null,
    val attribution: String? = null,
    val error: String? = null,
    /** At least one page came. */
    val loaded: Boolean = false,
)

object CommentsModel {
    /** Whether Now Playing offers the Comments pane: the song's source shows comments. */
    fun offered(sources: SourcesSnapshot, track: TrackRef?): Boolean = plugin(sources, track)?.comments == true

    fun plugin(sources: SourcesSnapshot, track: TrackRef?): PluginSummary? {
        val id = (track as? TrackRef.Source)?.track?.plugin ?: return null
        return sources.plugins.firstOrNull { it.id == id && it.enabled }
    }

    /** The first page for [item] (a new song starts over). */
    fun load(item: ItemRef, send: (Command) -> Long): CommentsState = CommentsState(item = item, request = send(Command.Comments(item, null)))

    /** The next page, when there is one and none is on its way. */
    fun more(state: CommentsState, send: (Command) -> Long): CommentsState {
        val item = state.item ?: return state
        val next = state.next ?: return state
        if (state.request != 0L) return state
        return state.copy(request = send(Command.Comments(item, next)))
    }

    /** A page: ours when it answers our request; the hot comments come with the first page only. */
    fun onPage(state: CommentsState, page: CommentsPage): CommentsState {
        if (page.request != state.request || page.item != state.item) return state
        val first = !state.loaded
        return state.copy(
            request = 0,
            hot = if (first) page.hot else state.hot,
            comments = (state.comments + page.comments).distinctBy { it.id },
            total = page.total ?: state.total,
            next = page.next,
            attribution = page.attribution ?: state.attribution,
            error = page.error?.takeIf { it.isNotBlank() },
            loaded = true,
        )
    }

    fun onFailed(state: CommentsState, id: Long, message: String): CommentsState =
        if (id != 0L && id == state.request) state.copy(request = 0, error = message, loaded = true) else state

    /** "No Comments Yet": a page came with nothing in it. */
    fun empty(state: CommentsState): Boolean = state.loaded && state.error == null && state.hot.isEmpty() && state.comments.isEmpty()
}
