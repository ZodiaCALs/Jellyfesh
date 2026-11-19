package org.jellyfesh.android.ffi

import org.jellyfesh.android.ffi.FfiMapping.toLogic
import org.jellyfesh.ffi.AndroidContext
import org.jellyfesh.ffi.CoreConfig
import org.jellyfesh.ffi.CoreFfiException
import org.jellyfesh.ffi.CoreListener
import org.jellyfesh.ffi.Event
import org.jellyfesh.ffi.JellyfeshCore
import org.jellyfesh.logic.core.Album
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.Artist
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreException
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.DeviceInfo
import org.jellyfesh.logic.core.Distribution
import org.jellyfesh.logic.core.StoreEntitlement
import org.jellyfesh.logic.core.EqPreset
import org.jellyfesh.logic.core.HistoryEntry
import org.jellyfesh.logic.core.LibraryStats
import org.jellyfesh.logic.core.PlaylistEntry
import org.jellyfesh.logic.core.PlaylistSummary
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.Track
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackRow
import org.jellyfesh.logic.time.DrawClock

/**
 * [CoreClient] over the generated UniFFI bindings (`org.jellyfesh.ffi`, docs/ffi.md). Together
 * with [FfiMapping] it is the only code in the app that touches generated types.
 */
class UniffiCoreClient private constructor(private val core: JellyfeshCore) : CoreClient {
    override fun send(command: Command): Long = core.send(FfiMapping.ffi(command)).toLong()

    override fun snapshot(): CoreSnapshot = core.snapshot().toLogic(::PortLyricTimeline)

    override fun setListener(listener: ((CoreEvent) -> Unit)?) {
        core.setListener(
            listener?.let { deliver ->
                // Called on the core's event thread, one event at a time (JNA attaches that thread
                // afresh for each call, so nothing may rely on the Thread object).
                object : CoreListener {
                    override fun onEvent(event: Event) {
                        deliver(event.toLogic(::PortLyricTimeline))
                    }
                }
            },
        )
    }

    override fun albums(offset: Int, limit: Int): List<Album> = query { core.albums(offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun albumsSorted(sort: AlbumSort, descending: Boolean, offset: Int, limit: Int): List<Album> =
        query { core.albumsSorted(FfiMapping.ffi(sort), descending, offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun artists(offset: Int, limit: Int): List<Artist> = query { core.artists(offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun tracks(query: TrackQuery): List<Track> = query { core.tracks(FfiMapping.ffi(query)).map { it.toLogic() } }

    override fun track(id: Long): Track? = query { core.track(id)?.toLogic() }

    override fun albumTracks(albumId: Long): List<Track> = query { core.albumTracks(albumId).map { it.toLogic() } }

    override fun artistAlbums(artistId: Long): List<Album> = query { core.artistAlbums(artistId).map { it.toLogic() } }

    override fun searchLibrary(text: String, limit: Int): SearchResults = query { core.searchLibrary(text, limit.u32()).toLogic() }

    override fun recentlyAdded(limit: Int): List<Track> = query { core.recentlyAdded(limit.u32()).map { it.toLogic() } }

    override fun recentlyPlayed(limit: Int): List<TrackRow> = query { core.recentlyPlayed(limit.u32()).map { it.toLogic() } }

    override fun mostPlayed(limit: Int): List<TrackRow> = query { core.mostPlayed(limit.u32()).map { it.toLogic() } }

    override fun playlists(): List<PlaylistSummary> = query { core.playlists().map { it.toLogic() } }

    override fun playlist(id: Long): PlaylistSummary? = query { core.playlist(id)?.toLogic() }

    override fun playlistEntries(id: Long, offset: Int, limit: Int): List<PlaylistEntry> =
        query { core.playlistEntries(id, offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun exportPlaylist(id: Long, path: String) = query { core.exportPlaylist(id, path) }

    override fun favorites(offset: Int, limit: Int): List<TrackRow> = query { core.favorites(offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun isFavorite(track: TrackRef): Boolean = query { core.isFavorite(FfiMapping.ffi(track)) }

    override fun artworkThumbnail(hash: String, sizePx: Int): String? = query { core.artworkThumbnail(hash, sizePx.u32()) }

    override fun queueWindow(offset: Int, limit: Int): List<QueueItem> = query { core.queueWindow(offset.u32(), limit.u32()).map { it.toLogic() } }

    override fun history(): List<HistoryEntry> = query { core.history().map { it.toLogic() } }

    override fun roots(): List<String> = query { core.roots() }

    override fun stats(): LibraryStats = query { core.stats().toLogic() }

    override fun localizedSettingsSchema(plugin: String, language: String): String? =
        query { core.localizedSettingsSchema(plugin, language) }

    override fun sourceArtwork(url: String, sizePx: Int): String? = query { core.sourceArtwork(url, sizePx.u32()) }

    override fun setStoreEntitlement(entitlement: StoreEntitlement) = query { core.setStoreEntitlement(FfiMappingV7.ffi(entitlement)) }

    // Qualified: the members of the same name would shadow the bindings' top-level functions.
    override fun outputDevices(): List<DeviceInfo> = query { org.jellyfesh.ffi.outputDevices().map { it.toLogic() } }

    override fun eqPresets(): List<EqPreset> = query { org.jellyfesh.ffi.eqPresets().map { it.toLogic() } }

    override fun libraryVersion(): String = query { org.jellyfesh.ffi.libraryVersion() }

    override fun monotonicNowNs(): Long = org.jellyfesh.ffi.monotonicNowNs().toLong()

    override fun newDrawClock(): DrawClock = PortDrawClock()

    /**
     * Saves the session and stops the core. The native object stays allocated until the
     * client is collected, so later calls fail with [CoreException.ShutDown] instead of crashing.
     */
    override fun shutdown() {
        core.setListener(null)
        core.shutdown()
    }

    companion object {
        /**
         * Hands the Android `Context` to the core (ndk-context, needed by the audio output and the
         * keystore before first use; docs/ffi.md) and starts the core with the official plugins
         * that ship in the APK ([bundledPlugins], `.jfpkg` paths), telling it how this build was
         * distributed ([distribution]: where Pro is sold, whether it checks for updates). Pass the
         * application context. Throws [CoreException] when the library or session cannot be opened.
         */
        fun start(
            context: Any,
            dataDir: String,
            cacheDir: String,
            bundledPlugins: List<String>,
            distribution: Distribution,
            watchFolders: Boolean = true,
        ): UniffiCoreClient {
            AndroidContext.initialize(context)
            return startWithoutContext(androidCoreConfig(dataDir, cacheDir, watchFolders, bundledPlugins, FfiMappingV7.ffi(distribution)))
        }

        /** Starts a core from a ready config (JVM tests use a null output). */
        fun startWithoutContext(config: CoreConfig): UniffiCoreClient = UniffiCoreClient(query { JellyfeshCore(config) })

        /** Runs a core call, turning the bindings' errors into :logic's [CoreException]. */
        private inline fun <T> query(call: () -> T): T = try {
            call()
        } catch (e: IllegalStateException) {
            // The generated object was already freed (close()).
            throw CoreException.ShutDown(e.message.orEmpty())
        } catch (e: CoreFfiException) {
            throw when (e) {
                is CoreFfiException.NotAvailable -> CoreException.NotAvailable(e.message.orEmpty())
                is CoreFfiException.ShutDown -> CoreException.ShutDown(e.message.orEmpty())
                is CoreFfiException.InvalidArgument -> CoreException.InvalidArgument(e.message.orEmpty())
                is CoreFfiException.Core -> CoreException.Failed(e.message.orEmpty())
            }
        }

        private fun Int.u32(): UInt = coerceAtLeast(0).toUInt()
    }
}
