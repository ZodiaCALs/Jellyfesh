package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DiscoverPage
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.Shelf
import org.jellyfesh.logic.core.ShelfStyle
import org.jellyfesh.logic.core.SourceItem

class DiscoverTest {
    private val item = SourceItem("subsonic", ItemKind.Album, "al:1", "Album", "Artist", null, null, null, true, null, null, emptyList())
    private val plugin = PluginSummary(
        "subsonic", "Navidrome", "0.0.7", "Jellyfesh", PluginBadge.Official, false, listOf("browse"), emptyList(),
        true, null, false, "{}", "{}", null,
    )

    @Test
    fun thePageAnswersItsOwnRequest() {
        val sent = mutableListOf<Command>()
        val loading = DiscoverModel.load(DiscoverState(), "subsonic") { sent += it; 12 }
        assertEquals(listOf<Command>(Command.Discover("subsonic")), sent)
        assertTrue(loading.loading)
        val shelves = listOf(Shelf("for_you", "For You", ShelfStyle.Row, listOf(item), null), Shelf("empty", "Empty", ShelfStyle.Grid, emptyList(), null))
        assertEquals(loading, DiscoverModel.onPage(loading, DiscoverPage(11, "subsonic", shelves, null)))
        val shown = DiscoverModel.onPage(loading, DiscoverPage(12, "subsonic", shelves, null))
        assertFalse(shown.loading)
        assertNull(shown.error)
        // Empty shelves are left out.
        assertEquals(listOf("for_you"), DiscoverModel.shelves(shown).map { it.id })
        assertEquals("offline", DiscoverModel.onPage(loading, DiscoverPage(12, "subsonic", emptyList(), "offline")).error)
        assertEquals("no route", DiscoverModel.onFailed(loading, 12, "no route").error)
        assertEquals(loading, DiscoverModel.onFailed(loading, 3, "other"))
    }

    @Test
    fun theCoresShelvesAreWordedByIdAndAPluginsKeepTheirWords() {
        fun shelf(id: String) = Shelf(id, "Title From Core", ShelfStyle.Grid, listOf(item), null)
        assertEquals(ShelfTitle.Known(KnownShelf.ForYou), DiscoverModel.title(shelf("for_you"), plugin))
        assertEquals(ShelfTitle.Known(KnownShelf.Rediscover), DiscoverModel.title(shelf("rediscover"), plugin))
        assertEquals(ShelfTitle.SourceName("Navidrome"), DiscoverModel.title(shelf("browse"), plugin))
        assertEquals(ShelfTitle.Given("Title From Core"), DiscoverModel.title(shelf("pl:7"), plugin))
        assertEquals(ShelfTitle.Given("Title From Core"), DiscoverModel.title(shelf("for_you"), plugin.copy(discover = true)))
    }
}
