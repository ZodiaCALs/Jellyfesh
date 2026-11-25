package org.jellyfesh.logic.library

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DownloadError
import org.jellyfesh.logic.core.DownloadItem
import org.jellyfesh.logic.core.DownloadState
import org.jellyfesh.logic.core.DownloadsSnapshot
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.TrackRef

/** The Downloads page (ADR-0025), on the core's Downloads slice. */
class DownloadsModelTest {
    private val jellyfin = "org.jellyfesh.jellyfin"
    private fun track(id: String, album: String?) = SourceTrack(jellyfin, id, "Song $id", "Glass Harbour", album)
    private fun item(id: Long, state: DownloadState, album: String? = "Slow Tide", done: Long = 0, total: Long? = null, library: Long? = null) =
        DownloadItem(id, track("t-$id", album), state, done, total, if (state == DownloadState.Done) "/m/$id.flac" else null, library, true)

    private val downloads = DownloadsSnapshot(
        version = 3,
        folder = "/storage/emulated/0/Music/Jellyfesh",
        items = listOf(
            item(1, DownloadState.Downloading, done = 250, total = 1_000),
            item(2, DownloadState.Queued),
            item(3, DownloadState.Failed(DownloadError.NotAllowed("Jellyfin"))),
            item(4, DownloadState.Done, library = 40),
            item(5, DownloadState.Done),
            item(6, DownloadState.Done, album = "Harbour Lights", library = 60),
        ),
        bytesUsed = 96_000_000,
        paused = null,
    )

    @Test
    fun inProgressThenTheFinishedSongsByAlbum() {
        assertEquals(listOf(1L, 2L, 3L), DownloadsModel.inProgress(downloads).map { it.id })
        val albums = DownloadsModel.albums(downloads)
        assertEquals(listOf("Slow Tide", "Harbour Lights"), albums.map { it.album })
        assertEquals(listOf(4L, 5L), albums[0].items.map { it.id })
    }

    @Test
    fun eachUnfinishedDownloadSaysWaitingItsPercentageOrWhyItFailed() {
        assertEquals(DownloadProgress.Downloading(25), DownloadsModel.progress(downloads.items[0]))
        assertEquals(DownloadProgress.Waiting, DownloadsModel.progress(downloads.items[1]))
        assertEquals(DownloadProgress.Failed(DownloadError.NotAllowed("Jellyfin")), DownloadsModel.progress(downloads.items[2]))
        assertNull(DownloadsModel.progress(downloads.items[3]))
        // An unknown size: no percentage yet.
        assertEquals(DownloadProgress.Downloading(null), DownloadsModel.progress(item(7, DownloadState.Downloading, done = 10)))
    }

    @Test
    fun playAndShufflePlayTheDownloadedSongs() {
        val songs = listOf(TrackRef.Library(40), TrackRef.Source(track("t-5", "Slow Tide")), TrackRef.Library(60))
        assertEquals(Command.PlayCollection(CollectionRef.Tracks(songs), 0, false), DownloadsModel.play(downloads, shuffle = false))
        assertEquals(Command.PlayCollection(CollectionRef.Tracks(songs), 0, true), DownloadsModel.play(downloads, shuffle = true))
        assertNull(DownloadsModel.play(DownloadsSnapshot.Empty, shuffle = false))
        val album = DownloadsModel.albums(downloads)[0]
        assertEquals(Command.PlayCollection(CollectionRef.Tracks(songs.take(2)), 1, false), DownloadsModel.playFrom(album, 1))
    }

    @Test
    fun removeCancelAndTryAgainNameTheDownloads() {
        assertEquals(listOf(Command.RemoveDownload(4), Command.RemoveDownload(5)), DownloadsModel.remove(DownloadsModel.albums(downloads)[0].items))
        assertEquals(Command.CancelDownload(2), DownloadsModel.cancel(downloads.items[1]))
        assertEquals(Command.RetryDownload(3), DownloadsModel.retry(downloads.items[2]))
    }
}
