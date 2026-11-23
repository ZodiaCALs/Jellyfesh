package org.jellyfesh.logic.core

import org.jellyfesh.logic.time.DrawClock

/**
 * The core, as the Android shell sees it: commands in, events and snapshots out, and synchronous
 * library queries (docs/core-api.md). The app implements it over the generated UniFFI bindings
 * (app/src/ffi, the only place that touches them); tests use a fake.
 *
 * Queries run on the caller's thread and touch SQLite: call them off the main thread. They throw
 * [CoreException]; [CoreException.NotAvailable] marks a feature still being integrated in this
 * release, which views show as "not available yet" rather than as an error.
 */
interface CoreClient {
    /** Queues a command; returns its id (0 after shutdown). Failures arrive as events. */
    fun send(command: Command): Long

    /** Every slice, now. Read it after [setListener]. */
    fun snapshot(): CoreSnapshot

    /** Receives every event from now on, on a core thread; null stops the events. */
    fun setListener(listener: ((CoreEvent) -> Unit)?)

    fun albums(offset: Int, limit: Int): List<Album>
    fun albumsSorted(sort: AlbumSort, descending: Boolean, offset: Int, limit: Int): List<Album>
    fun artists(offset: Int, limit: Int): List<Artist>
    fun tracks(query: TrackQuery): List<Track>
    fun track(id: Long): Track?
    fun albumTracks(albumId: Long): List<Track>
    fun artistAlbums(artistId: Long): List<Album>
    fun searchLibrary(text: String, limit: Int): SearchResults
    fun recentlyAdded(limit: Int): List<Track>
    fun recentlyPlayed(limit: Int): List<TrackRow>
    fun mostPlayed(limit: Int): List<TrackRow>
    fun playlists(): List<PlaylistSummary>
    fun playlist(id: Long): PlaylistSummary?
    fun playlistEntries(id: Long, offset: Int, limit: Int): List<PlaylistEntry>
    fun exportPlaylist(id: Long, path: String)
    fun favorites(offset: Int, limit: Int): List<TrackRow>
    fun isFavorite(track: TrackRef): Boolean

    /** A cached JPEG of the artwork with this hash at about [sizePx] (64 to 1024), or null. */
    fun artworkThumbnail(hash: String, sizePx: Int): String?
    fun queueWindow(offset: Int, limit: Int): List<QueueItem>
    fun history(): List<HistoryEntry>
    fun roots(): List<String>
    fun stats(): LibraryStats

    /**
     * The plugin's settings schema in [language] (a BCP 47 tag; English where a word has no
     * translation), the settings in the plugin's order; null when it is not installed.
     */
    fun localizedSettingsSchema(plugin: String, language: String): String?

    /**
     * A source picture (an `ArtworkRef.Url`) as a cached file fitted to about [sizePx]: the path
     * when it is cached, else null while the core fetches it ([CoreEvent.Artwork] follows).
     */
    fun sourceArtwork(url: String, sizePx: Int): String?

    /** Reports what the store says about Jellyfesh Pro, after the store glue verified it. */
    fun setStoreEntitlement(entitlement: StoreEntitlement)

    /** Audio output devices (a free function of the core). */
    fun outputDevices(): List<DeviceInfo>

    /**
     * The equalizer presets, in the order every app lists them (the core's free function
     * `eq_presets()`): Settings › Playback › Equalizer offers these, never a copy of its own.
     */
    fun eqPresets(): List<EqPreset>

    /** The version of the core library the app runs (the free function `library_version()`). */
    fun libraryVersion(): String

    /**
     * Whether [address], typed in Add a Source before it is saved, is on the local network by the
     * core's rule (the free function `address_is_local`; [PluginSummary.localNetwork] says it for
     * saved sources). The app never decides it. False until the generated bindings have it.
     */
    fun addressIsLocal(address: String): Boolean = false

    /** The clock anchors are stamped with, in nanoseconds. Not for per-frame use. */
    fun monotonicNowNs(): Long

    /** A new instance of the ported `DisplayClock`. */
    fun newDrawClock(): DrawClock

    /** Saves the session and stops the core; no event arrives afterwards. */
    fun shutdown()
}

/** Why a core call failed (the bindings' `CoreFfiException` kinds). */
sealed class CoreException(message: String) : Exception(message) {
    /** A library, session, audio or file error. */
    class Failed(message: String) : CoreException(message)

    /** The feature is not in this build yet. */
    class NotAvailable(message: String) : CoreException(message)

    /** The core was shut down. */
    class ShutDown(message: String) : CoreException(message)

    /** An argument could not be used. */
    class InvalidArgument(message: String) : CoreException(message)
}
