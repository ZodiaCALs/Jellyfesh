package org.jellyfesh.logic.state

import org.jellyfesh.logic.LinearClock
import org.jellyfesh.logic.core.Album
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.Artist
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreException
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.DeviceInfo
import org.jellyfesh.logic.core.EqPreset
import org.jellyfesh.logic.core.HistoryEntry
import org.jellyfesh.logic.core.LibraryStats
import org.jellyfesh.logic.core.PlaylistEntry
import org.jellyfesh.logic.core.PlaylistSummary
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.StoreEntitlement
import org.jellyfesh.logic.core.Track
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackRow
import org.jellyfesh.logic.time.DrawClock

/** A core that records commands and replays events; queries not set up answer "not available". */
class FakeCoreClient(private var snapshot: CoreSnapshot) : CoreClient {
    val sent = mutableListOf<Command>()
    private var listener: ((CoreEvent) -> Unit)? = null
    var albums: List<Album> = emptyList()
    var presets: List<EqPreset> = emptyList()
    var version: String = "0.0.0-test"

    fun emit(event: CoreEvent) {
        listener?.invoke(event)
    }

    override fun send(command: Command): Long {
        sent += command
        return sent.size.toLong()
    }

    override fun snapshot(): CoreSnapshot = snapshot

    override fun setListener(listener: ((CoreEvent) -> Unit)?) {
        this.listener = listener
    }

    override fun albums(offset: Int, limit: Int): List<Album> = albums.drop(offset).take(limit)
    override fun albumsSorted(sort: AlbumSort, descending: Boolean, offset: Int, limit: Int): List<Album> = albums(offset, limit)
    override fun artists(offset: Int, limit: Int): List<Artist> = emptyList()
    override fun tracks(query: TrackQuery): List<Track> = emptyList()
    override fun track(id: Long): Track? = null
    override fun albumTracks(albumId: Long): List<Track> = emptyList()
    override fun artistAlbums(artistId: Long): List<Album> = unavailable("artist pages")
    override fun searchLibrary(text: String, limit: Int): SearchResults = SearchResults.Empty
    override fun recentlyAdded(limit: Int): List<Track> = emptyList()
    override fun recentlyPlayed(limit: Int): List<TrackRow> = unavailable("play statistics")
    override fun mostPlayed(limit: Int): List<TrackRow> = unavailable("play statistics")
    override fun playlists(): List<PlaylistSummary> = emptyList()
    override fun playlist(id: Long): PlaylistSummary? = null
    override fun playlistEntries(id: Long, offset: Int, limit: Int): List<PlaylistEntry> = emptyList()
    override fun exportPlaylist(id: Long, path: String) = Unit
    override fun favorites(offset: Int, limit: Int): List<TrackRow> = emptyList()
    override fun isFavorite(track: TrackRef): Boolean = false
    override fun artworkThumbnail(hash: String, sizePx: Int): String? = null
    override fun queueWindow(offset: Int, limit: Int): List<QueueItem> = emptyList()
    override fun history(): List<HistoryEntry> = emptyList()
    override fun roots(): List<String> = emptyList()
    override fun stats(): LibraryStats = LibraryStats(0, 0, 0, 0, 0, 0, 0)
    /** The schemas [localizedSettingsSchema] answers, by plugin and language. */
    val schemas = mutableMapOf<Pair<String, String>, String>()

    /** The cached pictures [sourceArtwork] answers, by URL; others are "being fetched" (null). */
    val covers = mutableMapOf<String, String>()
    val entitlements = mutableListOf<StoreEntitlement>()

    override fun localizedSettingsSchema(plugin: String, language: String): String? = schemas[plugin to language]
    override fun sourceArtwork(url: String, sizePx: Int): String? = covers[url]
    override fun setStoreEntitlement(entitlement: StoreEntitlement) {
        entitlements += entitlement
    }
    override fun outputDevices(): List<DeviceInfo> = emptyList()
    override fun eqPresets(): List<EqPreset> = presets
    override fun libraryVersion(): String = version
    override fun monotonicNowNs(): Long = 0
    override fun newDrawClock(): DrawClock = LinearClock()
    override fun shutdown() = Unit

    private fun unavailable(what: String): Nothing = throw CoreException.NotAvailable("$what are not available in this build yet")
}
