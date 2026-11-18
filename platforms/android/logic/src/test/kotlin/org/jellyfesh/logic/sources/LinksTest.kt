package org.jellyfesh.logic.sources

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef

/** Open in {Service} and the main action of source rows (docs/ui.md, "Links and remote playback"). */
class LinksTest {
    private fun item(id: String, kind: ItemKind = ItemKind.Track, playable: Boolean = true, link: String? = null) = SourceItem(
        "qq", kind, id, "Title $id", "Artist", null, 200_000, null, playable,
        if (playable) null else "Not available in your region",
        if (playable && kind == ItemKind.Track) TrackRef.Source(SourceTrack("qq", id, "Title $id")) else null,
        emptyList(), link,
    )

    @Test
    fun onlyHttpsLinksAreOpened() {
        assertEquals("https://y.qq.example/song/1", Links.openable("https://y.qq.example/song/1"))
        assertEquals("https://open.example/t", Links.openable("HTTPS://open.example/t"), "the scheme in lower case")
        assertEquals("https://open.example/t", Links.openable("  https://open.example/t "))
        assertNull(Links.openable("http://open.example/t"))
        assertNull(Links.openable("intent://open#Intent;scheme=x;end"))
        assertNull(Links.openable("javascript:alert(1)"))
        assertNull(Links.openable("file:///sdcard/a"))
        assertNull(Links.openable("https:///no-host"))
        assertNull(Links.openable("/relative"))
        assertNull(Links.openable("https://bad host/"))
        assertNull(Links.openable(""))
        assertNull(Links.openable(null))
    }

    @Test
    fun theServiceIsThePluginsName() {
        val sources = SourcesSnapshot.Empty.copy(
            plugins = listOf(PluginSummary("qq", "QQ Music", "1.1.0", "x", PluginBadge.Community, false, emptyList(), emptyList(), true, null, false, "{}", "{}", null)),
        )
        assertEquals("QQ Music", Links.serviceName(sources, "qq"))
        assertEquals("gone", Links.serviceName(sources, "gone"))
    }

    @Test
    fun onlyASourceThatAsksForAttributionIsCredited() {
        val spotify = PluginSummary("sp", "Spotify", "1.0.0", "x", PluginBadge.Official, false, emptyList(), emptyList(), true, null, false, "{}", "{}", null, attribution = "Spotify")
        val sources = SourcesSnapshot.Empty.copy(plugins = listOf(spotify, spotify.copy(id = "plain", name = "Plain", attribution = null), spotify.copy(id = "blank", attribution = " ")))
        assertEquals("Spotify", Links.attributedService(sources, "sp"))
        assertNull(Links.attributedService(sources, "plain"))
        assertNull(Links.attributedService(sources, "blank"))
        assertNull(Links.attributedService(sources, "gone"))
    }

    @Test
    fun aPlayableSongPlaysWithTheListsOtherPlayableSongs() {
        val list = listOf(item("1"), item("2", playable = false, link = "https://y.qq.example/song/2"), item("3"))
        val action = SourceItems.mainAction(list[2], list)
        assertEquals(ItemAction.Play(listOf(list[0].track!!, list[2].track!!), 1), action)
    }

    @Test
    fun aSongThatCannotPlayHereOpensItsLink() {
        val locked = item("2", playable = false, link = "https://y.qq.example/song/2")
        assertEquals(ItemAction.OpenLink("https://y.qq.example/song/2"), SourceItems.mainAction(locked, listOf(locked)))
        val noLink = item("4", playable = false)
        assertEquals(ItemAction.None, SourceItems.mainAction(noLink, listOf(noLink)), "it shows its reason and does nothing")
        val badLink = item("5", playable = false, link = "intent://x")
        assertEquals(ItemAction.None, SourceItems.mainAction(badLink, listOf(badLink)))
    }

    @Test
    fun containersOpenInTheBrowserUnlessOnlyTheirLinkCanPlayThem() {
        val album = item("al", kind = ItemKind.Album, link = "https://y.qq.example/album/al")
        assertEquals(ItemAction.Browse("qq", "al", "Title al", "https://y.qq.example/album/al"), SourceItems.mainAction(album, listOf(album)))
        val locked = item("pl", kind = ItemKind.Playlist, playable = false, link = "https://y.qq.example/pl")
        assertEquals(ItemAction.OpenLink("https://y.qq.example/pl"), SourceItems.mainAction(locked, listOf(locked)))
        val lockedNoLink = item("ar", kind = ItemKind.Artist, playable = false)
        assertEquals(ItemAction.Browse("qq", "ar", "Title ar", null), SourceItems.mainAction(lockedNoLink, listOf(lockedNoLink)))
    }
}
