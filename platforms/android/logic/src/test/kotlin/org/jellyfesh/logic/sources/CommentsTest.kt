package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CommentEntry
import org.jellyfesh.logic.core.CommentsPage
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef

class CommentsTest {
    private val item = ItemRef.Source("netease", ItemKind.Track, "tr:1")
    private fun c(id: String) = CommentEntry(id, "A", null, "text $id", 3, 1_700_000_000_000, null)

    @Test
    fun pagesFollowOneAnotherAndTheHotOnesComeFirstOnly() {
        val sent = mutableListOf<Command>()
        var id = 10L
        val send: (Command) -> Long = { sent += it; ++id }
        var s = CommentsModel.load(item, send)
        assertEquals(Command.Comments(item, null), sent.last())
        s = CommentsModel.onPage(s, CommentsPage(11, item, listOf(c("h1")), listOf(c("1"), c("2")), 40, "p2", "Comments from NetEase", null))
        assertEquals(listOf("h1"), s.hot.map { it.id })
        assertEquals("p2", s.next)
        s = CommentsModel.more(s, send)
        assertEquals(Command.Comments(item, "p2"), sent.last())
        // Asking twice sends once.
        assertEquals(s, CommentsModel.more(s, send))
        s = CommentsModel.onPage(s, CommentsPage(12, item, listOf(c("h9")), listOf(c("2"), c("3")), null, null, null, null))
        assertEquals(listOf("h1"), s.hot.map { it.id })
        assertEquals(listOf("1", "2", "3"), s.comments.map { it.id })
        assertEquals(40L, s.total)
        assertEquals(s, CommentsModel.more(s, send))
        assertFalse(CommentsModel.empty(s))
    }

    @Test
    fun anotherSongsPageOrAFailureIsHandled() {
        val s = CommentsModel.load(item) { 5 }
        assertEquals(s, CommentsModel.onPage(s, CommentsPage(5, ItemRef.Track(1), emptyList(), emptyList(), null, null, null, null)))
        assertTrue(CommentsModel.empty(CommentsModel.onPage(s, CommentsPage(5, item, emptyList(), emptyList(), 0, null, null, null))))
        assertEquals("offline", CommentsModel.onFailed(s, 5, "offline").error)
    }

    @Test
    fun thePaneIsOfferedForSourcesThatShowComments() {
        val plugin = PluginSummary("netease", "NetEase", "1", "J", PluginBadge.Official, false, emptyList(), emptyList(), true, null, false, "{}", "{}", null, comments = true)
        val sources = SourcesSnapshot(1, listOf(plugin), emptyList(), false)
        val track = TrackRef.Source(SourceTrack("netease", "tr:1", "Song"))
        assertTrue(CommentsModel.offered(sources, track))
        assertFalse(CommentsModel.offered(sources, TrackRef.Library(1)))
        assertFalse(CommentsModel.offered(sources.copy(plugins = listOf(plugin.copy(comments = false))), track))
    }
}
