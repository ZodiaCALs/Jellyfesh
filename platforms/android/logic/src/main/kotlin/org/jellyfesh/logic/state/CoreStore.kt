package org.jellyfesh.logic.state

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.ArtworkReady
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.SourceLibrarySnapshot
import org.jellyfesh.logic.core.DownloadsSnapshot
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.PlaybackSnapshot
import org.jellyfesh.logic.core.ProSnapshot
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.ScanStatus
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.ThemeSnapshot
import org.jellyfesh.logic.core.UpdateSnapshot

/**
 * The shell's mirror of the core's slices. Nothing here is decided by the shell: every field is
 * the latest slice the core published (ADR-0001), plus whether the core has started.
 */
data class CoreState(
    val ready: Boolean = false,
    val playback: PlaybackSnapshot = PlaybackSnapshot.Initial,
    val queue: QueueSnapshot = QueueSnapshot.Empty,
    val nowPlaying: NowPlaying? = null,
    /** The latest Lyrics slice; see [currentLyrics]. */
    val lyrics: LyricsSnapshot? = null,
    val theme: ThemeSnapshot? = null,
    val libraryRevision: Long = 0,
    val scanning: Boolean = false,
    val lastScan: ScanStatus? = null,
    val settings: Settings = Settings(),
    val devices: DevicesSnapshot = DevicesSnapshot.Empty,
    val sources: SourcesSnapshot = SourcesSnapshot.Empty,
    /** The remote session (ADR-0018); the player views show it while it has a source. */
    val remote: RemoteSnapshot = RemoteSnapshot.Empty,
    val muted: Boolean = false,
    /** The uid a source is buffering, if any. */
    val bufferingUid: Long? = null,
    /** AI Lyrics (ADR-0019): whether to offer it, what it needs, its jobs and models. */
    val aiLyrics: AiLyricsSnapshot = AiLyricsSnapshot.Empty,
    /** The look, every value resolved (ADR-0020); the Luminous defaults until the core sends one. */
    val look: LookSnapshot = LookSnapshot.Default,
    /** Jellyfesh Pro (ADR-0021). */
    val pro: ProSnapshot = ProSnapshot.Inactive,
    /** Update checks for direct downloads. */
    val update: UpdateSnapshot = UpdateSnapshot.Unsupported,
    /** The sources' likes and playlists (ADR-0024). */
    val sourceLibrary: SourceLibrarySnapshot = SourceLibrarySnapshot.Empty,
    /** Downloads (ADR-0025). */
    val downloads: DownloadsSnapshot = DownloadsSnapshot.Empty,
) {
    /** The lyrics of the track playing now (a slice for another track is not shown). */
    val currentLyrics: LyricsSnapshot?
        get() = lyrics?.takeIf { it.uid == nowPlaying?.uid }

    val isBuffering: Boolean
        get() = bufferingUid != null && bufferingUid == nowPlaying?.uid
}

/** Applies events and snapshots to [CoreState]. Pure; tested on the JVM. */
object CoreReducer {
    fun reduce(state: CoreState, event: CoreEvent): CoreState = when (event) {
        is CoreEvent.Playback ->
            if (event.snapshot.version >= state.playback.version) state.copy(playback = event.snapshot) else state
        is CoreEvent.Queue ->
            if (event.snapshot.version >= state.queue.version) state.copy(queue = event.snapshot) else state
        is CoreEvent.NowPlayingChanged -> state.copy(nowPlaying = event.snapshot)
        is CoreEvent.Lyrics -> state.copy(lyrics = event.snapshot)
        is CoreEvent.Theme -> state.copy(theme = event.snapshot)
        is CoreEvent.Scan -> state.copy(
            scanning = event.status is ScanStatus.Progress,
            lastScan = event.status,
        )
        is CoreEvent.LibraryChanged ->
            if (event.revision >= state.libraryRevision) state.copy(libraryRevision = event.revision) else state
        is CoreEvent.SettingsChanged -> state.copy(settings = event.snapshot)
        is CoreEvent.Devices ->
            if (event.snapshot.version >= state.devices.version) state.copy(devices = event.snapshot) else state
        is CoreEvent.Sources ->
            if (event.snapshot.version >= state.sources.version) state.copy(sources = event.snapshot) else state
        is CoreEvent.Remote ->
            if (event.snapshot.version >= state.remote.version) state.copy(remote = event.snapshot) else state
        is CoreEvent.Buffering -> when {
            event.buffering -> state.copy(bufferingUid = event.uid)
            state.bufferingUid == event.uid -> state.copy(bufferingUid = null)
            else -> state
        }
        is CoreEvent.AiLyrics ->
            if (event.snapshot.version >= state.aiLyrics.version) state.copy(aiLyrics = event.snapshot) else state
        is CoreEvent.Look ->
            if (event.snapshot.version >= state.look.version) state.copy(look = event.snapshot) else state
        is CoreEvent.Pro ->
            if (event.snapshot.version >= state.pro.version) state.copy(pro = event.snapshot) else state
        is CoreEvent.Update ->
            if (event.snapshot.version >= state.update.version) state.copy(update = event.snapshot) else state
        is CoreEvent.SourceLibrary ->
            if (event.snapshot.version >= state.sourceLibrary.version) state.copy(sourceLibrary = event.snapshot) else state
        is CoreEvent.Downloads ->
            if (event.snapshot.version >= state.downloads.version) state.copy(downloads = event.snapshot) else state
        is CoreEvent.Auth, is CoreEvent.SearchResults, is CoreEvent.BrowsePage, is CoreEvent.CommandFailed,
        is CoreEvent.Error, is CoreEvent.PlaylistCreated, is CoreEvent.Discover, is CoreEvent.Details,
        is CoreEvent.Comments, is CoreEvent.Artwork, is CoreEvent.SearchPage, is CoreEvent.SourceLibraryPage -> state
    }

    /**
     * Merges a snapshot read after the listener was set: versioned slices keep whichever is
     * newer (an event may already have brought a later one), the rest take the snapshot.
     */
    fun merge(state: CoreState, snapshot: CoreSnapshot): CoreState = state.copy(
        ready = true,
        playback = newer(state.playback, snapshot.playback) { it.version },
        queue = newer(state.queue, snapshot.queue) { it.version },
        nowPlaying = snapshot.nowPlaying,
        lyrics = snapshot.lyrics ?: state.lyrics,
        theme = snapshot.theme ?: state.theme,
        libraryRevision = maxOf(state.libraryRevision, snapshot.libraryRevision),
        scanning = snapshot.scanning,
        settings = snapshot.settings,
        devices = newer(state.devices, snapshot.devices) { it.version },
        sources = newer(state.sources, snapshot.sources) { it.version },
        remote = newer(state.remote, snapshot.remote) { it.version },
        muted = snapshot.muted,
        aiLyrics = newer(state.aiLyrics, snapshot.aiLyrics) { it.version },
        look = newer(state.look, snapshot.look) { it.version },
        pro = newer(state.pro, snapshot.pro) { it.version },
        update = newer(state.update, snapshot.update) { it.version },
        sourceLibrary = newer(state.sourceLibrary, snapshot.sourceLibrary) { it.version },
        downloads = newer(state.downloads, snapshot.downloads) { it.version },
    )

    /**
     * Whether going from [before] to [after] ends a remote session: the slice had a source and
     * now has none. Its `error`, when set ("Sign in to Spotify again."), is shown once.
     */
    fun endsRemoteSession(before: RemoteSnapshot, after: RemoteSnapshot): Boolean =
        before.source != null && after.source == null

    private inline fun <T> newer(current: T, incoming: T, version: (T) -> Long): T =
        if (version(incoming) >= version(current)) incoming else current
}

/**
 * Holds [CoreState] and forwards the events that are not state ([CoreEvent.isSignal]) to whoever
 * listens. Thread-safe: the core calls [onEvent] from its event thread.
 */
class CoreStore {
    private val _state = MutableStateFlow(CoreState())
    val state: StateFlow<CoreState> = _state.asStateFlow()

    private val _signals = MutableSharedFlow<CoreEvent>(
        extraBufferCapacity = 64,
        onBufferOverflow = BufferOverflow.DROP_OLDEST,
    )

    /**
     * Search results, sign-in steps, browse pages, failures and toasts, as they arrive; and the
     * Remote slice that ends a remote session ([CoreReducer.endsRemoteSession]), so its error is
     * shown once even when the session ends right after it starts.
     */
    val signals: SharedFlow<CoreEvent> = _signals.asSharedFlow()

    /** Listens to [client] and loads its snapshot (set the listener first, then read). */
    fun attach(client: CoreClient) {
        client.setListener(::onEvent)
        val snapshot = client.snapshot()
        _state.update { CoreReducer.merge(it, snapshot) }
    }

    /**
     * Takes the core's word that a source picture is cached ([CoreEvent.Artwork]), on the core's
     * event thread. Covers arrive in bursts while lists scroll, so they go straight to the artwork
     * loader instead of through [signals], where they could push out a search or a sign-in step.
     */
    @Volatile
    var artworkListener: ((ArtworkReady) -> Unit)? = null

    fun onEvent(event: CoreEvent) {
        if (event is CoreEvent.Artwork) {
            artworkListener?.invoke(event.ready)
            return
        }
        var endsSession = false
        _state.update { before ->
            CoreReducer.reduce(before, event).also { after -> endsSession = CoreReducer.endsRemoteSession(before.remote, after.remote) }
        }
        if (event.isSignal || endsSession) _signals.tryEmit(event)
    }
}
