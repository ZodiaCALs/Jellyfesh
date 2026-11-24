package org.jellyfesh.logic.core

/** What the core reports (docs/core-api.md). Slices arrive whole whenever they change. */
sealed interface CoreEvent {
    data class Playback(val snapshot: PlaybackSnapshot) : CoreEvent
    data class Queue(val snapshot: QueueSnapshot) : CoreEvent
    data class NowPlayingChanged(val snapshot: NowPlaying?) : CoreEvent
    data class Lyrics(val snapshot: LyricsSnapshot) : CoreEvent
    data class Theme(val snapshot: ThemeSnapshot) : CoreEvent
    data class Scan(val status: ScanStatus) : CoreEvent
    data class LibraryChanged(val revision: Long) : CoreEvent
    data class PlaylistCreated(val request: Long, val id: Long) : CoreEvent
    data class SettingsChanged(val snapshot: Settings) : CoreEvent
    data class Devices(val snapshot: DevicesSnapshot) : CoreEvent
    data class Sources(val snapshot: SourcesSnapshot) : CoreEvent
    /** The remote session changed (ADR-0018). */
    data class Remote(val snapshot: RemoteSnapshot) : CoreEvent
    data class Auth(val update: AuthUpdate) : CoreEvent
    data class SearchResults(val update: SearchUpdate) : CoreEvent
    data class BrowsePage(val update: BrowseUpdate) : CoreEvent
    /** A source is buffering the current track (rate-0 anchors meanwhile). */
    data class Buffering(val uid: Long, val buffering: Boolean) : CoreEvent
    data class CommandFailed(val id: Long, val error: String) : CoreEvent
    /** A problem not tied to a command, such as an unreadable file during playback. */
    data class Error(val message: String) : CoreEvent

    // v0.0.7
    data class AiLyrics(val snapshot: AiLyricsSnapshot) : CoreEvent
    data class Look(val snapshot: LookSnapshot) : CoreEvent
    data class Pro(val snapshot: ProSnapshot) : CoreEvent
    data class Update(val snapshot: UpdateSnapshot) : CoreEvent
    data class Discover(val page: DiscoverPage) : CoreEvent
    data class Details(val page: DetailsPage) : CoreEvent
    data class Comments(val page: CommentsPage) : CoreEvent

    /** A source picture is cached, or cannot be had; the artwork loader takes it, not the state. */
    data class Artwork(val ready: ArtworkReady) : CoreEvent

    // v0.0.8
    /** More of one kind of a source's search results ([Command.SearchMore]). */
    data class SearchPage(val page: org.jellyfesh.logic.core.SearchPage) : CoreEvent

    /** The SourceLibrary slice changed (ADR-0024). */
    data class SourceLibrary(val snapshot: SourceLibrarySnapshot) : CoreEvent

    /** More of a source's list ([Command.SourceLibraryMore]). */
    data class SourceLibraryPage(val page: org.jellyfesh.logic.core.SourceLibraryPage) : CoreEvent

    /** The Downloads slice changed (ADR-0025). */
    data class Downloads(val snapshot: DownloadsSnapshot) : CoreEvent

    /**
     * True for events that are not state: they matter to whoever is waiting for them (a search,
     * a sign-in, a toast), and the store forwards them on its signal stream.
     */
    val isSignal: Boolean
        get() = when (this) {
            is Auth, is SearchResults, is BrowsePage, is CommandFailed, is Error, is PlaylistCreated, is Scan,
            is Discover, is Details, is Comments, is SearchPage, is SourceLibraryPage -> true
            else -> false
        }
}
