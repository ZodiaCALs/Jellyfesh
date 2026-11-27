package org.jellyfesh.logic.library

import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DownloadError
import org.jellyfesh.logic.core.DownloadItem
import org.jellyfesh.logic.core.DownloadState
import org.jellyfesh.logic.core.DownloadsSnapshot
import org.jellyfesh.logic.core.TrackRef

/** What an unfinished download says: "Waiting", its percentage, or why it failed (with Try Again). */
sealed interface DownloadProgress {
    data object Waiting : DownloadProgress

    /** Downloading, [percent] done; null while the size is not known. */
    data class Downloading(val percent: Int?) : DownloadProgress

    data class Failed(val error: DownloadError) : DownloadProgress
}

/** The finished downloads of one album of one source, in the core's order. */
data class DownloadedAlbum(val plugin: String, val album: String?, val items: List<DownloadItem>) {
    /** A key for the list: the source and the album's title. */
    val key: String get() = "$plugin/${album.orEmpty()}"
}

/**
 * The Downloads page (docs/ui.md, "Downloads"; ADR-0025): Play and Shuffle with "{n} songs · {size}
 * used", In Progress (each with "Waiting", its percentage, or the failure and Try Again, and
 * Cancel), the line while the core pauses downloads, then the downloaded songs by album, each song
 * and each album with Remove Download…. The core orders the slice (in progress in queue order,
 * then finished by album); this splits it and words nothing.
 */
object DownloadsModel {
    fun inProgress(downloads: DownloadsSnapshot): List<DownloadItem> = downloads.items.filter { it.state != DownloadState.Done }

    fun finished(downloads: DownloadsSnapshot): List<DownloadItem> = downloads.items.filter { it.state == DownloadState.Done }

    /**
     * Whether a source's song has a finished download, for a row that does not say (a queue entry,
     * Now Playing): its menu then offers no Download.
     */
    fun downloaded(downloads: DownloadsSnapshot, track: TrackRef): Boolean {
        val song = (track as? TrackRef.Source)?.track ?: return false
        return downloads.items.any { it.state == DownloadState.Done && it.track.plugin == song.plugin && it.track.id == song.id }
    }

    /** The finished songs by album (the same title from one source), albums in the core's order. */
    fun albums(downloads: DownloadsSnapshot): List<DownloadedAlbum> =
        finished(downloads).groupBy { it.track.plugin to it.track.album }.map { (key, items) -> DownloadedAlbum(key.first, key.second, items) }

    /** What an unfinished download says; null once it is done. */
    fun progress(item: DownloadItem): DownloadProgress? = when (val state = item.state) {
        DownloadState.Queued -> DownloadProgress.Waiting
        DownloadState.Downloading -> DownloadProgress.Downloading(percent(item))
        is DownloadState.Failed -> DownloadProgress.Failed(state.error)
        DownloadState.Done -> null
    }

    /** How much is downloaded, 0 to 100, when the size is known. */
    fun percent(item: DownloadItem): Int? {
        val total = item.bytesTotal?.takeIf { it > 0 } ?: return null
        return (item.bytesDone.coerceIn(0, total) * 100 / total).toInt()
    }

    /** The song a finished download plays: its library track once the core has one, else the source's (which plays the file). */
    fun track(item: DownloadItem): TrackRef = item.libraryId?.let(TrackRef::Library) ?: TrackRef.Source(item.track)

    /** Play or Shuffle every downloaded song; null with none. */
    fun play(downloads: DownloadsSnapshot, shuffle: Boolean): Command? =
        finished(downloads).map(::track).takeIf { it.isNotEmpty() }?.let { Command.PlayCollection(CollectionRef.Tracks(it), 0, shuffle) }

    /** A downloaded song chosen in its album: the album's songs from it. */
    fun playFrom(album: DownloadedAlbum, index: Int): Command = Command.PlayCollection(CollectionRef.Tracks(album.items.map(::track)), index, false)

    /** Remove Download… on a song, or on a whole album (every downloaded song of it), once the person confirms. */
    fun remove(items: List<DownloadItem>): List<Command> = items.map { Command.RemoveDownload(it.id) }

    fun cancel(item: DownloadItem): Command = Command.CancelDownload(item.id)

    fun retry(item: DownloadItem): Command = Command.RetryDownload(item.id)
}
