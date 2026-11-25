package org.jellyfesh.logic.core

// v0.0.8's contract (docs/core-api.md; ADR-0024 to ADR-0026): streaming quality, downloads, the
// mini player's settings, search by source and the sources' likes and playlists. Mirrors of the
// core's types; the app decides nothing with them that the core has not (ADR-0001).

/** The streaming level asked of sources (ADR-0026 §1). */
enum class StreamingQuality {
    /** Lossless, or High on a connection the platform reports as metered. */
    Auto,
    Lossless,
    High,
    Normal,
    Low,
}

/** The quality downloads are asked for (ADR-0025 §2). */
enum class DownloadQuality {
    /** The file exactly as the server keeps it. */
    Original,
    High,
    Normal,
    Low,
}

/** Settings › Downloads (ADR-0025 §2); the core's defaults until its slice arrives. */
data class DownloadSettings(
    /** The folder the person chose; null uses the default ([DownloadsSnapshot.folder]). */
    val folder: String? = null,
    val quality: DownloadQuality = DownloadQuality.Original,
    /** File Names: `{artist}`, `{album_artist}`, `{album}`, `{title}`, `{track}`, `{disc}`, `{year}`, `{source}`. */
    val nameTemplate: String = "{album_artist}/{album}/{track} {title}",
    /** Add Missing Cover and Lyrics (off: original files stay byte for byte). */
    val addMissingCoverAndLyrics: Boolean = false,
    val lyricsInTags: Boolean = true,
    val saveLrcFiles: Boolean = false,
    /** Pause on Metered Connections: downloads wait while the connection is metered (the core's `wifi_only`). */
    val wifiOnly: Boolean = true,
    /** Downloads at Once, 1–4. */
    val atOnce: Int = 2,
)

/** Settings › Appearance › Mini Player (ADR-0026 §4). Android uses [androidPip] only. */
data class MiniPlayerSettings(
    val keepOnTop: Boolean = true,
    val openWhenWindowClosed: Boolean = false,
    val showLyricsLine: Boolean = true,
    /** Mini Player in Picture-in-Picture, off by default. */
    val androidPip: Boolean = false,
)

/** Why the quality menu offers no levels; the app words it. */
sealed interface QualityNote {
    /** "Files play as they are." (library songs, files, downloads) */
    data object FilesPlayAsTheyAre : QualityNote

    /** "Quality is set in {service}." (remote playback, sources that offer no levels) */
    data class SetInService(val service: String) : QualityNote
}

/** One kind of a source's search results, for its tab, with Show More's cursor ([next]). */
data class SearchGroup(val kind: ItemKind, val items: List<SourceItem>, val next: String?, val total: Int?)

/** More of one kind of a source's results (`Command.SearchMore`). */
data class SearchPage(
    /** The id `send` returned for the `SearchMore` command. */
    val request: Long,
    val query: String,
    val source: String,
    val kind: ItemKind,
    val items: List<SourceItem>,
    val next: String?,
    val error: String?,
)

/** One of a source's lists (ADR-0024). */
enum class SourceLibraryPart { LikedSongs, LikedAlbums, LikedArtists, Playlists }

/** An entry of a source's list. */
data class SourceLibraryEntry(
    val item: SourceItem,
    /** Playlists: the person's own (or collaborative), not followed. */
    val owned: Boolean,
    /** Playlists: false when the service lists its songs only in its own app. True for likes. */
    val songsListed: Boolean,
)

/** One list of a source: the first page, kept on disk with its age. */
data class SourceLibraryList(
    val part: SourceLibraryPart,
    val entries: List<SourceLibraryEntry>,
    val total: Int?,
    /** `SourceLibraryMore`'s cursor; null when there is no more. */
    val next: String?,
    /** When it was last loaded, ms since the Unix epoch ("Updated 5 minutes ago"). */
    val refreshedAtMs: Long?,
    val loading: Boolean,
    val error: String?,
)

/** A source's likes and playlists; lists it does not keep are left out. */
data class SourceLibrary(
    val plugin: String,
    val name: String,
    /** "Content from {Service}" when its terms ask for it. */
    val attribution: String?,
    /** Liked Songs, Liked Albums, Liked Artists, Playlists, in that order. */
    val lists: List<SourceLibraryList>,
)

/** The `SourceLibrary` slice: every enabled, signed-in source with the `library` capability. */
data class SourceLibrarySnapshot(val version: Long, val sources: List<SourceLibrary>) {
    companion object {
        val Empty = SourceLibrarySnapshot(0, emptyList())
    }
}

/** More of a source's list (`Command.SourceLibraryMore`). */
data class SourceLibraryPage(
    val request: Long,
    val plugin: String,
    val part: SourceLibraryPart,
    val entries: List<SourceLibraryEntry>,
    val next: String?,
    val error: String?,
)

/** Why downloads wait. */
enum class DownloadsPaused {
    /** No network. */
    Offline,

    /** A metered network with Pause on Metered Connections on. */
    Metered,
}

/** Why a download failed; the app words it. */
sealed interface DownloadError {
    /** "Your {service} server doesn't allow downloads for this account." */
    data class NotAllowed(val service: String) : DownloadError

    /** The source offers no downloads. */
    data class NotOffered(val service: String) : DownloadError

    /** The connection failed after the retries. */
    data class Network(val message: String) : DownloadError

    /** The file could not be written. */
    data class Storage(val message: String) : DownloadError

    data class Other(val message: String) : DownloadError
}

/** Where a download is. */
sealed interface DownloadState {
    /** Waiting for its turn, or for the network ([DownloadsSnapshot.paused]). */
    data object Queued : DownloadState
    data object Downloading : DownloadState
    data object Done : DownloadState

    /** Try Again sends `RetryDownload`. */
    data class Failed(val error: DownloadError) : DownloadState
}

/** One download. */
data class DownloadItem(
    val id: Long,
    val track: SourceTrack,
    val state: DownloadState,
    val bytesDone: Long,
    val bytesTotal: Long?,
    /** The file, once finished. */
    val path: String?,
    /** Its library track, once finished. */
    val libraryId: Long?,
    /** The server's original rather than a conversion. */
    val original: Boolean,
)

/** The `Downloads` slice. */
data class DownloadsSnapshot(
    val version: Long,
    /** Where downloads go: the person's folder, or the default one. */
    val folder: String,
    /** In progress (queue order), then finished by album. */
    val items: List<DownloadItem>,
    /** Space the finished downloads take, in bytes. */
    val bytesUsed: Long,
    val paused: DownloadsPaused?,
) {
    companion object {
        val Empty = DownloadsSnapshot(0, "", emptyList(), 0, null)
    }
}

/** What `Command.Download` downloads. */
sealed interface DownloadTarget {
    /** One song: a source track. */
    data class Track(val track: TrackRef) : DownloadTarget

    /** Every song of an album or playlist a source listed. */
    data class Collection(val plugin: String, val id: String, val kind: ItemKind) : DownloadTarget
}
