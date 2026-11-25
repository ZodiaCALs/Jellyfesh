package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DownloadTarget
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef

/** Download in the menus (ADR-0025). */
class DownloadMenusTest {
    private fun plugin(id: String, download: Boolean, enabled: Boolean = true) = PluginSummary(
        id, id, "1.3.0", "Jellyfesh", PluginBadge.Official, false, listOf("search", "browse"), listOf(AuthMethod.PasswordExchange),
        enabled, "listener", false, "{}", "{}", null, download = download,
    )

    private val sources = SourcesSnapshot.Empty.copy(
        plugins = listOf(plugin("org.jellyfesh.jellyfin", download = true), plugin("org.jellyfesh.spotify", download = false), plugin("off", download = true, enabled = false)),
    )
    private val song = TrackRef.Source(SourceTrack("org.jellyfesh.jellyfin", "t-1", "Morning Harbour"))

    @Test
    fun aSongDownloadsFromASourceThatOffersItUntilItIsDownloaded() {
        assertTrue(DownloadMenus.canDownload(sources, song, downloaded = false))
        assertFalse(DownloadMenus.canDownload(sources, song, downloaded = true))
        assertFalse(DownloadMenus.canDownload(sources, TrackRef.Source(SourceTrack("org.jellyfesh.spotify", "t-2", "Gulls")), downloaded = false))
        assertFalse(DownloadMenus.canDownload(sources, TrackRef.Source(SourceTrack("off", "t-3", "Off")), downloaded = false))
        assertFalse(DownloadMenus.canDownload(sources, TrackRef.Library(4), downloaded = false))
        assertEquals(Command.Download(DownloadTarget.Track(song)), DownloadMenus.download(song))
    }

    @Test
    fun albumsAndPlaylistsOfSuchASourceDownloadWhole() {
        val album = CollectionRef.Source("org.jellyfesh.jellyfin", "al-1", ItemKind.Album)
        assertTrue(DownloadMenus.canDownload(sources, album))
        assertTrue(DownloadMenus.canDownload(sources, album.copy(kind = ItemKind.Playlist)))
        assertFalse(DownloadMenus.canDownload(sources, album.copy(kind = ItemKind.Artist)))
        assertFalse(DownloadMenus.canDownload(sources, CollectionRef.Album(1)))
        assertEquals(Command.Download(DownloadTarget.Collection("org.jellyfesh.jellyfin", "al-1", ItemKind.Album)), DownloadMenus.download(album))
    }

    @Test
    fun downloadSitsAfterFavouritesInTheSongMenuAndBeforeTheLinkInMore() {
        // A source's songs and lists never go in a playlist (no Add to Playlist…).
        assertEquals(
            listOf(TrackMenuItem.PlayNext, TrackMenuItem.PlayLast, TrackMenuItem.RemoveFromFavourites, TrackMenuItem.Download, TrackMenuItem.GoToAlbum),
            TrackMenu.items(song, favourite = true, hasAlbum = true, hasArtist = false, download = true),
        )
        assertEquals(
            listOf(CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.Download, CollectionMenuItem.OpenLink),
            CollectionMenu.items(CollectionRef.Source("org.jellyfesh.jellyfin", "pl-1", ItemKind.Playlist), withPlay = false, hasLink = true, download = true),
        )
        assertFalse(CollectionMenuItem.Download in CollectionMenu.items(CollectionRef.Album(1), withPlay = true))
        // A source playlist's Save as Playlist… comes before Download (ADR-0024).
        assertEquals(
            listOf(CollectionMenuItem.PlayNext, CollectionMenuItem.PlayLast, CollectionMenuItem.SaveAsPlaylist, CollectionMenuItem.Download),
            CollectionMenu.items(CollectionRef.Source("org.jellyfesh.jellyfin", "pl-1", ItemKind.Playlist), withPlay = false, download = true, saveAsPlaylist = true),
        )
    }
}
