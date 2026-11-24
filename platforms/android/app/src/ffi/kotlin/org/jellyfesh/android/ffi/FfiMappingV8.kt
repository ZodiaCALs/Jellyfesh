package org.jellyfesh.android.ffi

// Conversions for the core's v0.0.8 types (docs/core-api.md: streaming quality, downloads, the
// mini player's settings, search by source and the sources' likes and playlists), next to
// FfiMapping. Like it, the only code that names these generated types, so a renamed field is
// fixed here.

import org.jellyfesh.ffi.DownloadError as FDownloadError
import org.jellyfesh.ffi.DownloadItem as FDownloadItem
import org.jellyfesh.ffi.DownloadQuality as FDownloadQuality
import org.jellyfesh.ffi.DownloadSettings as FDownloadSettings
import org.jellyfesh.ffi.DownloadState as FDownloadState
import org.jellyfesh.ffi.DownloadTarget as FDownloadTarget
import org.jellyfesh.ffi.DownloadsPaused as FDownloadsPaused
import org.jellyfesh.ffi.DownloadsSnapshot as FDownloadsSnapshot
import org.jellyfesh.ffi.MiniPlayerSettings as FMiniPlayerSettings
import org.jellyfesh.ffi.QualityNote as FQualityNote
import org.jellyfesh.ffi.SearchGroup as FSearchGroup
import org.jellyfesh.ffi.SearchPage as FSearchPage
import org.jellyfesh.ffi.SourceLibrary as FSourceLibrary
import org.jellyfesh.ffi.SourceLibraryEntry as FSourceLibraryEntry
import org.jellyfesh.ffi.SourceLibraryList as FSourceLibraryList
import org.jellyfesh.ffi.SourceLibraryPage as FSourceLibraryPage
import org.jellyfesh.ffi.SourceLibraryPart as FSourceLibraryPart
import org.jellyfesh.ffi.SourceLibrarySnapshot as FSourceLibrarySnapshot
import org.jellyfesh.ffi.StreamingQuality as FStreamingQuality
import org.jellyfesh.android.ffi.FfiMapping.toLogic
import org.jellyfesh.android.ffi.FfiMappingV7.toLogicKind
import org.jellyfesh.logic.core.DownloadError
import org.jellyfesh.logic.core.DownloadItem
import org.jellyfesh.logic.core.DownloadQuality
import org.jellyfesh.logic.core.DownloadSettings
import org.jellyfesh.logic.core.DownloadState
import org.jellyfesh.logic.core.DownloadTarget
import org.jellyfesh.logic.core.DownloadsPaused
import org.jellyfesh.logic.core.DownloadsSnapshot
import org.jellyfesh.logic.core.MiniPlayerSettings
import org.jellyfesh.logic.core.QualityNote
import org.jellyfesh.logic.core.SearchGroup
import org.jellyfesh.logic.core.SearchPage
import org.jellyfesh.logic.core.SourceLibrary
import org.jellyfesh.logic.core.SourceLibraryEntry
import org.jellyfesh.logic.core.SourceLibraryList
import org.jellyfesh.logic.core.SourceLibraryPage
import org.jellyfesh.logic.core.SourceLibraryPart
import org.jellyfesh.logic.core.SourceLibrarySnapshot
import org.jellyfesh.logic.core.StreamingQuality

object FfiMappingV8 {
    // ---- shell -> core ---------------------------------------------------------------------------

    fun ffi(quality: StreamingQuality): FStreamingQuality = when (quality) {
        StreamingQuality.Auto -> FStreamingQuality.AUTO
        StreamingQuality.Lossless -> FStreamingQuality.LOSSLESS
        StreamingQuality.High -> FStreamingQuality.HIGH
        StreamingQuality.Normal -> FStreamingQuality.NORMAL
        StreamingQuality.Low -> FStreamingQuality.LOW
    }

    fun ffi(quality: DownloadQuality): FDownloadQuality = when (quality) {
        DownloadQuality.Original -> FDownloadQuality.ORIGINAL
        DownloadQuality.High -> FDownloadQuality.HIGH
        DownloadQuality.Normal -> FDownloadQuality.NORMAL
        DownloadQuality.Low -> FDownloadQuality.LOW
    }

    fun ffi(settings: DownloadSettings): FDownloadSettings = FDownloadSettings(
        folder = settings.folder,
        quality = ffi(settings.quality),
        nameTemplate = settings.nameTemplate,
        addMissingCoverAndLyrics = settings.addMissingCoverAndLyrics,
        lyricsInTags = settings.lyricsInTags,
        saveLrcFiles = settings.saveLrcFiles,
        wifiOnly = settings.wifiOnly,
        atOnce = settings.atOnce.coerceIn(1, 4).toUInt(),
    )

    fun ffi(settings: MiniPlayerSettings): FMiniPlayerSettings = FMiniPlayerSettings(
        keepOnTop = settings.keepOnTop,
        openWhenWindowClosed = settings.openWhenWindowClosed,
        showLyricsLine = settings.showLyricsLine,
        androidPip = settings.androidPip,
    )

    fun ffi(part: SourceLibraryPart): FSourceLibraryPart = when (part) {
        SourceLibraryPart.LikedSongs -> FSourceLibraryPart.LIKED_SONGS
        SourceLibraryPart.LikedAlbums -> FSourceLibraryPart.LIKED_ALBUMS
        SourceLibraryPart.LikedArtists -> FSourceLibraryPart.LIKED_ARTISTS
        SourceLibraryPart.Playlists -> FSourceLibraryPart.PLAYLISTS
    }

    fun ffi(target: DownloadTarget): FDownloadTarget = when (target) {
        is DownloadTarget.Track -> FDownloadTarget.Track(track = FfiMapping.ffi(target.track))
        is DownloadTarget.Collection -> FDownloadTarget.Collection(plugin = target.plugin, id = target.id, itemKind = FfiMappingV7.ffi(target.kind))
    }

    // ---- core -> shell ---------------------------------------------------------------------------

    fun FStreamingQuality.toLogic(): StreamingQuality = when (this) {
        FStreamingQuality.AUTO -> StreamingQuality.Auto
        FStreamingQuality.LOSSLESS -> StreamingQuality.Lossless
        FStreamingQuality.HIGH -> StreamingQuality.High
        FStreamingQuality.NORMAL -> StreamingQuality.Normal
        FStreamingQuality.LOW -> StreamingQuality.Low
    }

    fun FDownloadQuality.toLogic(): DownloadQuality = when (this) {
        FDownloadQuality.ORIGINAL -> DownloadQuality.Original
        FDownloadQuality.HIGH -> DownloadQuality.High
        FDownloadQuality.NORMAL -> DownloadQuality.Normal
        FDownloadQuality.LOW -> DownloadQuality.Low
    }

    fun FDownloadSettings.toLogic(): DownloadSettings = DownloadSettings(
        folder = folder,
        quality = quality.toLogic(),
        nameTemplate = nameTemplate,
        addMissingCoverAndLyrics = addMissingCoverAndLyrics,
        lyricsInTags = lyricsInTags,
        saveLrcFiles = saveLrcFiles,
        wifiOnly = wifiOnly,
        atOnce = atOnce.toInt(),
    )

    fun FMiniPlayerSettings.toLogic(): MiniPlayerSettings =
        MiniPlayerSettings(keepOnTop = keepOnTop, openWhenWindowClosed = openWhenWindowClosed, showLyricsLine = showLyricsLine, androidPip = androidPip)

    fun FQualityNote.toLogic(): QualityNote = when (this) {
        is FQualityNote.FilesPlayAsTheyAre -> QualityNote.FilesPlayAsTheyAre
        is FQualityNote.SetInService -> QualityNote.SetInService(service)
    }

    fun FSearchGroup.toLogic(): SearchGroup = SearchGroup(kind.toLogicKind(), items.map { it.toLogic() }, next, total?.toInt())

    fun FSearchPage.toLogic(): SearchPage =
        SearchPage(request.toLong(), query, source, kind.toLogicKind(), items.map { it.toLogic() }, next, error)

    fun FSourceLibraryPart.toLogic(): SourceLibraryPart = when (this) {
        FSourceLibraryPart.LIKED_SONGS -> SourceLibraryPart.LikedSongs
        FSourceLibraryPart.LIKED_ALBUMS -> SourceLibraryPart.LikedAlbums
        FSourceLibraryPart.LIKED_ARTISTS -> SourceLibraryPart.LikedArtists
        FSourceLibraryPart.PLAYLISTS -> SourceLibraryPart.Playlists
    }

    fun FSourceLibraryEntry.toLogic(): SourceLibraryEntry = SourceLibraryEntry(item.toLogic(), owned, songsListed)

    fun FSourceLibraryList.toLogic(): SourceLibraryList = SourceLibraryList(
        part = part.toLogic(),
        entries = entries.map { it.toLogic() },
        total = total?.toInt(),
        next = next,
        refreshedAtMs = refreshedAtMs,
        loading = loading,
        error = error,
    )

    fun FSourceLibrary.toLogic(): SourceLibrary = SourceLibrary(plugin, name, attribution, lists.map { it.toLogic() })

    fun FSourceLibrarySnapshot.toLogic(): SourceLibrarySnapshot = SourceLibrarySnapshot(version.toLong(), sources.map { it.toLogic() })

    fun FSourceLibraryPage.toLogic(): SourceLibraryPage =
        SourceLibraryPage(request.toLong(), plugin, part.toLogic(), entries.map { it.toLogic() }, next, error)

    fun FDownloadsPaused.toLogic(): DownloadsPaused = when (this) {
        FDownloadsPaused.OFFLINE -> DownloadsPaused.Offline
        FDownloadsPaused.METERED -> DownloadsPaused.Metered
    }

    fun FDownloadError.toLogic(): DownloadError = when (this) {
        is FDownloadError.NotAllowed -> DownloadError.NotAllowed(service)
        is FDownloadError.NotOffered -> DownloadError.NotOffered(service)
        is FDownloadError.Network -> DownloadError.Network(message)
        is FDownloadError.Storage -> DownloadError.Storage(message)
        is FDownloadError.Other -> DownloadError.Other(message)
    }

    fun FDownloadState.toLogic(): DownloadState = when (this) {
        is FDownloadState.Queued -> DownloadState.Queued
        is FDownloadState.Downloading -> DownloadState.Downloading
        is FDownloadState.Done -> DownloadState.Done
        is FDownloadState.Failed -> DownloadState.Failed(error.toLogic())
    }

    fun FDownloadItem.toLogic(): DownloadItem = DownloadItem(
        id = id.toLong(),
        track = track.toLogic(),
        state = state.toLogic(),
        bytesDone = bytesDone.toLong(),
        bytesTotal = bytesTotal?.toLong(),
        path = path,
        libraryId = libraryId,
        original = original,
    )

    fun FDownloadsSnapshot.toLogic(): DownloadsSnapshot =
        DownloadsSnapshot(version.toLong(), folder, items.map { it.toLogic() }, bytesUsed.toLong(), paused?.toLogic())
}
