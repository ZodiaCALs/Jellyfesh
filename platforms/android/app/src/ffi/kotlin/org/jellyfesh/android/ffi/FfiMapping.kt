package org.jellyfesh.android.ffi

// Conversions between the generated UniFFI types (org.jellyfesh.ffi, docs/ffi.md) and the
// shell's own model (:logic). With UniffiCoreClient this is the only code that names generated
// types, so a renamed field after a core change is fixed here. Compiled and tested on the JVM by
// :ffi-check against the real library.

import org.jellyfesh.ffi.Album as FAlbum
import org.jellyfesh.ffi.AlbumSort as FAlbumSort
import org.jellyfesh.ffi.AgentKind as FAgentKind
import org.jellyfesh.ffi.Appearance as FAppearance
import org.jellyfesh.ffi.AppearancePref as FAppearancePref
import org.jellyfesh.ffi.Artist as FArtist
import org.jellyfesh.ffi.ArtworkRef as FArtworkRef
import org.jellyfesh.ffi.AuthField as FAuthField
import org.jellyfesh.ffi.AuthFieldKind as FAuthFieldKind
import org.jellyfesh.ffi.AuthMethod as FAuthMethod
import org.jellyfesh.ffi.AuthStep as FAuthStep
import org.jellyfesh.ffi.AuthUpdate as FAuthUpdate
import org.jellyfesh.ffi.AuxKind as FAuxKind
import org.jellyfesh.ffi.BackdropStyle as FBackdropStyle
import org.jellyfesh.ffi.BandKind as FBandKind
import org.jellyfesh.ffi.BrowseUpdate as FBrowseUpdate
import org.jellyfesh.ffi.Collection as FCollection
import org.jellyfesh.ffi.Command as FCommand
import org.jellyfesh.ffi.CrossfadeSettings as FCrossfadeSettings
import org.jellyfesh.ffi.DeviceInfo as FDeviceInfo
import org.jellyfesh.ffi.DevicesSnapshot as FDevicesSnapshot
import org.jellyfesh.ffi.Direction as FDirection
import org.jellyfesh.ffi.EqBand as FEqBand
import org.jellyfesh.ffi.EqPreset as FEqPreset
import org.jellyfesh.ffi.EqSettings as FEqSettings
import org.jellyfesh.ffi.Event as FEvent
import org.jellyfesh.ffi.Format as FFormat
import org.jellyfesh.ffi.HistoryEntry as FHistoryEntry
import org.jellyfesh.ffi.HueSource as FHueSource
import org.jellyfesh.ffi.ItemKind as FItemKind
import org.jellyfesh.ffi.LibraryStats as FLibraryStats
import org.jellyfesh.ffi.Line as FLine
import org.jellyfesh.ffi.LyricDoc as FLyricDoc
import org.jellyfesh.ffi.LyricsAlign as FLyricsAlign
import org.jellyfesh.ffi.LyricsSize as FLyricsSize
import org.jellyfesh.ffi.LyricsSnapshot as FLyricsSnapshot
import org.jellyfesh.ffi.LyricsSource as FLyricsSource
import org.jellyfesh.ffi.NowPlaying as FNowPlaying
import org.jellyfesh.ffi.PlaybackAnchor as FPlaybackAnchor
import org.jellyfesh.ffi.PlaybackSnapshot as FPlaybackSnapshot
import org.jellyfesh.ffi.PlaylistEntry as FPlaylistEntry
import org.jellyfesh.ffi.PlaylistSummary as FPlaylistSummary
import org.jellyfesh.ffi.PluginBadge as FPluginBadge
import org.jellyfesh.ffi.PluginSummary as FPluginSummary
import org.jellyfesh.ffi.QueueItem as FQueueItem
import org.jellyfesh.ffi.QueueSnapshot as FQueueSnapshot
import org.jellyfesh.ffi.RemoteAction as FRemoteAction
import org.jellyfesh.ffi.RemoteDevice as FRemoteDevice
import org.jellyfesh.ffi.RemoteDeviceKind as FRemoteDeviceKind
import org.jellyfesh.ffi.RemoteSnapshot as FRemoteSnapshot
import org.jellyfesh.ffi.RepeatMode as FRepeatMode
import org.jellyfesh.ffi.ReplayGainMode as FReplayGainMode
import org.jellyfesh.ffi.ScanStatus as FScanStatus
import org.jellyfesh.ffi.SearchResults as FSearchResults
import org.jellyfesh.ffi.SearchScope as FSearchScope
import org.jellyfesh.ffi.SearchUpdate as FSearchUpdate
import org.jellyfesh.ffi.Settings as FSettings
import org.jellyfesh.ffi.Snapshot as FSnapshot
import org.jellyfesh.ffi.SourceItem as FSourceItem
import org.jellyfesh.ffi.SourcePreset as FSourcePreset
import org.jellyfesh.ffi.SourceTrack as FSourceTrack
import org.jellyfesh.ffi.SourcesSnapshot as FSourcesSnapshot
import org.jellyfesh.ffi.Status as FStatus
import org.jellyfesh.ffi.Theme as FTheme
import org.jellyfesh.ffi.ThemeSnapshot as FThemeSnapshot
import org.jellyfesh.ffi.Timing as FTiming
import org.jellyfesh.ffi.Track as FTrack
import org.jellyfesh.ffi.TrackInfo as FTrackInfo
import org.jellyfesh.ffi.TrackQuery as FTrackQuery
import org.jellyfesh.ffi.TrackRef as FTrackRef
import org.jellyfesh.ffi.TrackRow as FTrackRow
import org.jellyfesh.ffi.TrackSort as FTrackSort
import org.jellyfesh.ffi.Word as FWord
import org.jellyfesh.logic.core.Album
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.AgentKind
import org.jellyfesh.logic.core.Appearance
import org.jellyfesh.logic.core.AppearancePref
import org.jellyfesh.logic.core.Artist
import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.AuthField
import org.jellyfesh.logic.core.AuthFieldKind
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.AuthStep
import org.jellyfesh.logic.core.AuthUpdate
import org.jellyfesh.logic.core.AuxKind
import org.jellyfesh.logic.core.AuxLine
import org.jellyfesh.logic.core.AuxTrack
import org.jellyfesh.logic.core.BackdropStyle
import org.jellyfesh.logic.core.BandKind
import org.jellyfesh.logic.core.BrowseUpdate
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.CoreSnapshot
import org.jellyfesh.logic.core.CrossfadeSettings
import org.jellyfesh.logic.core.DeviceInfo
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.EqBand
import org.jellyfesh.logic.core.EqPreset
import org.jellyfesh.logic.core.EqSettings
import org.jellyfesh.logic.core.HistoryEntry
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.LibraryStats
import org.jellyfesh.logic.core.LyricAgent
import org.jellyfesh.logic.core.LyricDoc
import org.jellyfesh.logic.core.LyricFormat
import org.jellyfesh.logic.core.LyricLine
import org.jellyfesh.logic.core.LyricWord
import org.jellyfesh.logic.core.LyricsAlign
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.LyricsSource
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.PlaybackAnchor
import org.jellyfesh.logic.core.PlaybackSnapshot
import org.jellyfesh.logic.core.PlaylistEntry
import org.jellyfesh.logic.core.PlaylistSummary
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.RemoteAction
import org.jellyfesh.logic.core.RemoteDevice
import org.jellyfesh.logic.core.RemoteDeviceKind
import org.jellyfesh.logic.core.RemoteSnapshot
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.ReplayGainMode
import org.jellyfesh.logic.core.ScanStatus
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.logic.core.SearchScope
import org.jellyfesh.logic.core.SearchUpdate
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourcePreset
import org.jellyfesh.logic.core.SourceTrack
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Theme
import org.jellyfesh.logic.core.ThemeSnapshot
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.core.Track
import org.jellyfesh.logic.core.TrackInfo
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackRow
import org.jellyfesh.logic.core.TrackSort
import org.jellyfesh.logic.theme.ColorMath
import org.jellyfesh.android.ffi.FfiMappingV7.toLogic

object FfiMapping {
    // ---- shell -> core ---------------------------------------------------------------------------

    fun ffi(command: Command): FCommand = when (command) {
        Command.Play -> FCommand.Play
        Command.Pause -> FCommand.Pause
        Command.TogglePlayPause -> FCommand.TogglePlayPause
        Command.Stop -> FCommand.Stop
        Command.Next -> FCommand.Next
        Command.Previous -> FCommand.Previous
        is Command.Seek -> FCommand.Seek(positionMs = command.positionMs.coerceAtLeast(0).toULong())
        is Command.SetVolume -> FCommand.SetVolume(volume = command.volume.coerceIn(0f, 1f))
        is Command.SetMuted -> FCommand.SetMuted(muted = command.muted)
        is Command.PlayTracks -> FCommand.PlayTracks(tracks = command.tracks.map(::ffi), start = command.start.u32())
        is Command.PlayCollection -> FCommand.PlayCollection(collection = ffi(command.collection), start = command.start.u32(), shuffle = command.shuffle)
        is Command.Enqueue -> FCommand.Enqueue(tracks = command.tracks.map(::ffi), playNext = command.playNext)
        is Command.EnqueueCollection -> FCommand.EnqueueCollection(collection = ffi(command.collection), playNext = command.playNext)
        is Command.RemoveFromQueue -> FCommand.RemoveFromQueue(uid = command.uid.toULong())
        is Command.MoveInQueue -> FCommand.MoveInQueue(uid = command.uid.toULong(), to = command.to.u32())
        is Command.JumpTo -> FCommand.JumpTo(uid = command.uid.toULong())
        Command.ClearQueue -> FCommand.ClearQueue
        is Command.SetShuffle -> FCommand.SetShuffle(on = command.on)
        is Command.SetRepeat -> FCommand.SetRepeat(mode = ffi(command.mode))
        is Command.SetSettings -> FCommand.SetSettings(settings = ffi(command.settings))
        is Command.SetEq -> FCommand.SetEq(eq = ffi(command.eq))
        is Command.SetReplayGain -> FCommand.SetReplayGain(mode = ffi(command.mode), preampDb = command.preampDb)
        is Command.SetCrossfade -> FCommand.SetCrossfade(crossfade = ffi(command.crossfade))
        is Command.SetOutputDevice -> FCommand.SetOutputDevice(name = command.id)
        Command.RefreshDevices -> FCommand.RefreshDevices
        is Command.AddRoot -> FCommand.AddRoot(path = command.path)
        is Command.RemoveRoot -> FCommand.RemoveRoot(path = command.path)
        Command.Rescan -> FCommand.Rescan
        is Command.CreatePlaylist -> FCommand.CreatePlaylist(name = command.name, tracks = command.tracks.map(::ffi))
        is Command.RenamePlaylist -> FCommand.RenamePlaylist(id = command.id, name = command.name)
        is Command.DeletePlaylist -> FCommand.DeletePlaylist(id = command.id)
        is Command.AddToPlaylist -> FCommand.AddToPlaylist(id = command.id, tracks = command.tracks.map(::ffi))
        is Command.RemoveFromPlaylist -> FCommand.RemoveFromPlaylist(id = command.id, entries = command.entries)
        is Command.MovePlaylistEntry -> FCommand.MovePlaylistEntry(id = command.id, entry = command.entry, to = command.to.u32())
        is Command.ImportPlaylist -> FCommand.ImportPlaylist(path = command.path)
        is Command.SetFavorite -> FCommand.SetFavorite(track = ffi(command.track), favorite = command.favorite)
        is Command.InstallPlugin -> FCommand.InstallPlugin(path = command.path)
        is Command.UninstallPlugin -> FCommand.UninstallPlugin(id = command.id)
        is Command.SetPluginEnabled -> FCommand.SetPluginEnabled(id = command.id, enabled = command.enabled)
        is Command.SetPluginSettings -> FCommand.SetPluginSettings(id = command.id, settingsJson = command.settingsJson)
        is Command.AcknowledgeUnofficial -> FCommand.AcknowledgeUnofficial(id = command.id)
        is Command.BeginLogin -> FCommand.BeginLogin(plugin = command.plugin, method = ffi(command.method))
        is Command.ConnectSource -> FCommand.ConnectSource(preset = command.preset, address = command.address, method = ffi(command.method))
        is Command.SubmitLogin -> FCommand.SubmitLogin(plugin = command.plugin, flow = command.flow.toULong(), fields = command.fields)
        is Command.CancelLogin -> FCommand.CancelLogin(plugin = command.plugin, flow = command.flow.toULong())
        is Command.Logout -> FCommand.Logout(plugin = command.plugin)
        is Command.Search -> FCommand.Search(query = command.query, scope = ffi(command.scope))
        is Command.Browse -> FCommand.Browse(plugin = command.plugin, node = command.node, page = command.page.u32())
        is Command.Remote -> FCommand.Remote(action = ffi(command.action))
        is Command.ResetPluginSettings -> FCommand.ResetPluginSettings(plugin = command.plugin)
        is Command.MakeLyrics -> FCommand.MakeLyrics(song = FfiMappingV7.ffi(command.song))
        is Command.CancelLyrics -> FCommand.CancelLyrics(job = command.job.toULong())
        is Command.DeleteMadeLyrics -> FCommand.DeleteMadeLyrics(song = FfiMappingV7.ffi(command.song))
        is Command.AllowAudioUpload -> FCommand.AllowAudioUpload(plugin = command.plugin, origin = command.origin)
        is Command.RevokeAudioUpload -> FCommand.RevokeAudioUpload(plugin = command.plugin, origin = command.origin)
        is Command.DownloadSpeechModel -> FCommand.DownloadSpeechModel(id = command.id)
        is Command.CancelSpeechModelDownload -> FCommand.CancelSpeechModelDownload(id = command.id)
        is Command.DeleteSpeechModel -> FCommand.DeleteSpeechModel(id = command.id)
        is Command.ActivateLicence -> FCommand.ActivateLicence(key = command.key)
        Command.RemoveLicence -> FCommand.RemoveLicence
        Command.CheckForUpdates -> FCommand.CheckForUpdates
        is Command.Discover -> FCommand.Discover(source = command.source)
        is Command.Details -> FCommand.Details(item = FfiMappingV7.ffi(command.item))
        is Command.Comments -> FCommand.Comments(item = FfiMappingV7.ffi(command.item), page = command.page)
    }

    fun ffi(action: RemoteAction): FRemoteAction = when (action) {
        RemoteAction.Refresh -> FRemoteAction.Refresh
        is RemoteAction.Transfer -> FRemoteAction.Transfer(device = action.device)
        is RemoteAction.SetVolume -> FRemoteAction.SetVolume(percent = action.percent.coerceIn(0, 100).toUInt())
        RemoteAction.End -> FRemoteAction.End
    }

    fun ffi(track: TrackRef): FTrackRef = when (track) {
        is TrackRef.Library -> FTrackRef.Library(id = track.id)
        is TrackRef.File -> FTrackRef.File(path = track.path)
        is TrackRef.Source -> FTrackRef.Source(
            track = FSourceTrack(
                plugin = track.track.plugin,
                id = track.track.id,
                title = track.track.title,
                artist = track.track.artist,
                album = track.track.album,
                durationMs = track.track.durationMs?.toULong(),
                artworkUrl = track.track.artworkUrl,
                isrc = track.track.isrc,
            ),
        )
    }

    fun ffi(collection: CollectionRef): FCollection = when (collection) {
        is CollectionRef.Album -> FCollection.Album(id = collection.id)
        is CollectionRef.Artist -> FCollection.Artist(id = collection.id)
        is CollectionRef.Playlist -> FCollection.Playlist(id = collection.id)
        CollectionRef.Favorites -> FCollection.Favorites
        is CollectionRef.Songs -> FCollection.Songs(sort = ffi(collection.sort), descending = collection.descending)
        is CollectionRef.Tracks -> FCollection.Tracks(tracks = collection.tracks.map(::ffi))
        is CollectionRef.Source -> FCollection.Source(plugin = collection.plugin, id = collection.id)
    }

    fun ffi(scope: SearchScope): FSearchScope = when (scope) {
        SearchScope.All -> FSearchScope.All
        SearchScope.Library -> FSearchScope.Library
        is SearchScope.Plugin -> FSearchScope.Plugin(id = scope.id)
    }

    fun ffi(mode: RepeatMode): FRepeatMode = when (mode) {
        RepeatMode.Off -> FRepeatMode.OFF
        RepeatMode.All -> FRepeatMode.ALL
        RepeatMode.One -> FRepeatMode.ONE
    }

    fun ffi(mode: ReplayGainMode): FReplayGainMode = when (mode) {
        ReplayGainMode.Off -> FReplayGainMode.OFF
        ReplayGainMode.Track -> FReplayGainMode.TRACK
        ReplayGainMode.Album -> FReplayGainMode.ALBUM
    }

    fun ffi(method: AuthMethod): FAuthMethod = when (method) {
        AuthMethod.Qr -> FAuthMethod.QR
        AuthMethod.Oauth -> FAuthMethod.OAUTH
        AuthMethod.CookieImport -> FAuthMethod.COOKIE_IMPORT
        AuthMethod.PasswordExchange -> FAuthMethod.PASSWORD_EXCHANGE
        AuthMethod.ApiKey -> FAuthMethod.API_KEY
    }

    fun ffi(eq: EqSettings): FEqSettings = FEqSettings(
        enabled = eq.enabled,
        preampDb = eq.preampDb,
        bands = eq.bands.map {
            FEqBand(
                kind = when (it.kind) {
                    BandKind.Peaking -> FBandKind.PEAKING
                    BandKind.LowShelf -> FBandKind.LOW_SHELF
                    BandKind.HighShelf -> FBandKind.HIGH_SHELF
                },
                freqHz = it.freqHz,
                gainDb = it.gainDb,
                q = it.q,
                enabled = it.enabled,
            )
        },
    )

    fun ffi(crossfade: CrossfadeSettings): FCrossfadeSettings = FCrossfadeSettings(
        enabled = crossfade.enabled,
        durationMs = crossfade.durationMs.coerceAtLeast(0).toUInt(),
        albumContinuity = crossfade.albumContinuity,
    )

    fun ffi(settings: Settings): FSettings = FSettings(
        appearance = when (settings.appearance) {
            AppearancePref.System -> FAppearancePref.SYSTEM
            AppearancePref.Light -> FAppearancePref.LIGHT
            AppearancePref.Dark -> FAppearancePref.DARK
        },
        style = when (settings.style) {
            BackdropStyle.Artwork -> FBackdropStyle.ARTWORK
            BackdropStyle.Misty -> FBackdropStyle.MISTY
        },
        appIcon = settings.appIcon,
        hueSource = when (val hue = settings.hueSource) {
            HueSource.Artwork -> FHueSource.Artwork
            is HueSource.Fixed -> FHueSource.Fixed(hue = hue.hue)
        },
        reduceEffects = settings.reduceEffects,
        transparency = settings.transparency,
        lyricsSize = when (settings.lyricsSize) {
            LyricsSize.Small -> FLyricsSize.SMALL
            LyricsSize.Medium -> FLyricsSize.MEDIUM
            LyricsSize.Large -> FLyricsSize.LARGE
        },
        lyricsAlign = when (settings.lyricsAlign) {
            LyricsAlign.Start -> FLyricsAlign.START
            LyricsAlign.Center -> FLyricsAlign.CENTER
        },
        showTranslation = settings.showTranslation,
        showRomanization = settings.showRomanization,
        crossfade = ffi(settings.crossfade),
        replaygainMode = ffi(settings.replaygainMode),
        preampDb = settings.preampDb,
        eq = ffi(settings.eq),
        outputDevice = settings.outputDevice,
        resumeOnLaunch = settings.resumeOnLaunch,
        watchFolders = settings.watchFolders,
        allowCommunityPlugins = settings.allowCommunityPlugins,
        checkForUpdates = settings.checkForUpdates,
        showLyricsBar = settings.showLyricsBar,
        desktopLyrics = settings.desktopLyrics,
    )

    fun ffi(sort: TrackSort): FTrackSort = when (sort) {
        TrackSort.Artist -> FTrackSort.ARTIST
        TrackSort.Title -> FTrackSort.TITLE
        TrackSort.Album -> FTrackSort.ALBUM
        TrackSort.Year -> FTrackSort.YEAR
        TrackSort.Duration -> FTrackSort.DURATION
        TrackSort.Path -> FTrackSort.PATH
        TrackSort.Added -> FTrackSort.ADDED
    }

    fun ffi(query: TrackQuery): FTrackQuery = FTrackQuery(
        offset = query.offset.u32(),
        limit = query.limit.u32(),
        sort = ffi(query.sort),
        descending = query.descending,
        albumId = query.albumId,
        artistId = query.artistId,
    )

    fun ffi(sort: AlbumSort): FAlbumSort = when (sort) {
        AlbumSort.Title -> FAlbumSort.TITLE
        AlbumSort.Artist -> FAlbumSort.ARTIST
        AlbumSort.Year -> FAlbumSort.YEAR
        AlbumSort.Added -> FAlbumSort.ADDED
    }

    fun ffi(anchor: PlaybackAnchor): FPlaybackAnchor = FPlaybackAnchor(
        trackGeneration = anchor.trackGeneration.toULong(),
        positionUs = anchor.positionUs,
        anchorMonoNs = anchor.anchorMonoNs.toULong(),
        rate = anchor.rate,
        durationUs = anchor.durationUs,
    )

    private fun Int.u32(): UInt = coerceAtLeast(0).toUInt()

    // ---- core -> shell ---------------------------------------------------------------------------

    fun FEvent.toLogic(lyrics: (org.jellyfesh.ffi.LyricTimeline) -> org.jellyfesh.logic.time.LyricTimelineLookup): CoreEvent = when (this) {
        is FEvent.Playback -> CoreEvent.Playback(snapshot.toLogic())
        is FEvent.Queue -> CoreEvent.Queue(snapshot.toLogic())
        is FEvent.NowPlaying -> CoreEvent.NowPlayingChanged(snapshot?.toLogic())
        is FEvent.Lyrics -> CoreEvent.Lyrics(snapshot.toLogic(lyrics))
        is FEvent.Theme -> CoreEvent.Theme(snapshot.toLogic())
        is FEvent.Scan -> CoreEvent.Scan(status.toLogic())
        is FEvent.LibraryChanged -> CoreEvent.LibraryChanged(revision.toLong())
        is FEvent.PlaylistCreated -> CoreEvent.PlaylistCreated(request.toLong(), id)
        is FEvent.Settings -> CoreEvent.SettingsChanged(snapshot.toLogic())
        is FEvent.Devices -> CoreEvent.Devices(snapshot.toLogic())
        is FEvent.Sources -> CoreEvent.Sources(snapshot.toLogic())
        is FEvent.Remote -> CoreEvent.Remote(snapshot.toLogic())
        is FEvent.Auth -> CoreEvent.Auth(update.toLogic())
        is FEvent.SearchResults -> CoreEvent.SearchResults(update.toLogic())
        is FEvent.BrowsePage -> CoreEvent.BrowsePage(update.toLogic())
        is FEvent.Buffering -> CoreEvent.Buffering(uid.toLong(), buffering)
        is FEvent.CommandFailed -> CoreEvent.CommandFailed(id.toLong(), error)
        is FEvent.Error -> CoreEvent.Error(message)
        is FEvent.AiLyrics -> CoreEvent.AiLyrics(snapshot.toLogic())
        is FEvent.Look -> CoreEvent.Look(snapshot.toLogic())
        is FEvent.Pro -> CoreEvent.Pro(snapshot.toLogic())
        is FEvent.Update -> CoreEvent.Update(snapshot.toLogic())
        is FEvent.Discover -> CoreEvent.Discover(page.toLogic())
        is FEvent.Details -> CoreEvent.Details(page.toLogic())
        is FEvent.Comments -> CoreEvent.Comments(page.toLogic())
        is FEvent.Artwork -> CoreEvent.Artwork(ready.toLogic())
    }

    fun FSnapshot.toLogic(lyrics: (org.jellyfesh.ffi.LyricTimeline) -> org.jellyfesh.logic.time.LyricTimelineLookup): CoreSnapshot = CoreSnapshot(
        playback = playback.toLogic(),
        queue = queue.toLogic(),
        nowPlaying = nowPlaying?.toLogic(),
        lyrics = this.lyrics?.toLogic(lyrics),
        theme = theme?.toLogic(),
        libraryRevision = libraryRevision.toLong(),
        scanning = scanning,
        settings = settings.toLogic(),
        devices = devices.toLogic(),
        sources = sources.toLogic(),
        muted = muted,
        remote = remote.toLogic(),
        aiLyrics = aiLyrics.toLogic(),
        look = look.toLogic(),
        pro = pro.toLogic(),
        update = update.toLogic(),
    )

    fun FTrackRef.toLogic(): TrackRef = when (this) {
        is FTrackRef.Library -> TrackRef.Library(id)
        is FTrackRef.File -> TrackRef.File(path)
        is FTrackRef.Source -> TrackRef.Source(track.toLogic())
    }

    fun FSourceTrack.toLogic(): SourceTrack =
        SourceTrack(plugin, id, title, artist, album, durationMs?.toLong(), artworkUrl, isrc)

    fun FQueueItem.toLogic(): QueueItem = QueueItem(uid.toLong(), track.toLogic())

    fun FPlaybackAnchor.toLogic(): PlaybackAnchor =
        PlaybackAnchor(trackGeneration.toLong(), positionUs, anchorMonoNs.toLong(), rate, durationUs)

    fun FStatus.toLogic(): Status = when (this) {
        FStatus.STOPPED -> Status.Stopped
        FStatus.PLAYING -> Status.Playing
        FStatus.PAUSED -> Status.Paused
        FStatus.ENDED -> Status.Ended
    }

    fun FRepeatMode.toLogic(): RepeatMode = when (this) {
        FRepeatMode.OFF -> RepeatMode.Off
        FRepeatMode.ALL -> RepeatMode.All
        FRepeatMode.ONE -> RepeatMode.One
    }

    fun FReplayGainMode.toLogic(): ReplayGainMode = when (this) {
        FReplayGainMode.OFF -> ReplayGainMode.Off
        FReplayGainMode.TRACK -> ReplayGainMode.Track
        FReplayGainMode.ALBUM -> ReplayGainMode.Album
    }

    fun FEqSettings.toLogic(): EqSettings = EqSettings(
        enabled,
        preampDb,
        bands.map { b: FEqBand ->
            EqBand(
                kind = when (b.kind) {
                    FBandKind.PEAKING -> BandKind.Peaking
                    FBandKind.LOW_SHELF -> BandKind.LowShelf
                    FBandKind.HIGH_SHELF -> BandKind.HighShelf
                },
                freqHz = b.freqHz,
                gainDb = b.gainDb,
                q = b.q,
                enabled = b.enabled,
            )
        },
    )

    fun FPlaybackSnapshot.toLogic(): PlaybackSnapshot = PlaybackSnapshot(
        version = version.toLong(),
        status = status.toLogic(),
        anchor = anchor.toLogic(),
        volume = volume,
        eq = eq.toLogic(),
        replaygainMode = replaygainMode.toLogic(),
        preampDb = preampDb,
    )

    fun FQueueSnapshot.toLogic(): QueueSnapshot = QueueSnapshot(
        version = version.toLong(),
        len = len.toInt(),
        position = position?.toInt(),
        current = current?.toLogic(),
        shuffle = shuffle,
        repeat = repeat.toLogic(),
        upNext = upNext.map { it.toLogic() },
    )

    fun FArtworkRef.toLogic(): ArtworkRef = when (this) {
        is FArtworkRef.File -> ArtworkRef.File(path)
        is FArtworkRef.Url -> ArtworkRef.Url(url)
    }

    fun FTrackInfo.toLogic(): TrackInfo =
        TrackInfo(codec, sampleRate.toInt(), channels.toInt(), durationUs, gapless, bitrate?.toInt(), bitsPerSample?.toInt())

    fun FNowPlaying.toLogic(): NowPlaying = NowPlaying(
        uid = uid.toLong(),
        track = track.toLogic(),
        path = path,
        title = title,
        artist = artist,
        album = album,
        durationMs = durationMs?.toLong(),
        artwork = artwork?.toLogic(),
        format = format?.toLogic(),
        link = link,
        quality = quality?.toLogic(),
    )

    private fun FWord.toLogic(): LyricWord = LyricWord(startMs.toLong(), endMs.toLong(), text, joinsNext)

    private fun FLine.toLogic(): LyricLine = LyricLine(
        id = id.toInt(),
        startMs = startMs?.toLong(),
        endMs = endMs?.toLong(),
        agent = agent,
        text = text,
        words = words.map { it.toLogic() },
        background = background.map { it.toLogic() },
        direction = when (direction) {
            FDirection.LTR -> TextDir.Ltr
            FDirection.RTL -> TextDir.Rtl
            FDirection.AUTO -> TextDir.Auto
        },
    )

    fun FLyricDoc.toLogic(): LyricDoc = LyricDoc(
        language = language,
        timing = when (timing) {
            FTiming.UNSYNCED -> Timing.Unsynced
            FTiming.LINE -> Timing.Line
            FTiming.WORD -> Timing.Word
        },
        offsetMs = offsetMs,
        agents = agents.map {
            LyricAgent(
                it.id,
                it.name,
                when (it.kind) {
                    FAgentKind.PERSON -> AgentKind.Person
                    FAgentKind.GROUP -> AgentKind.Group
                    FAgentKind.OTHER -> AgentKind.Other
                },
            )
        },
        lines = lines.map { it.toLogic() },
        tracks = tracks.map { track ->
            AuxTrack(
                kind = when (track.kind) {
                    FAuxKind.TRANSLATION -> AuxKind.Translation
                    FAuxKind.ROMANIZATION -> AuxKind.Romanization
                },
                language = track.language,
                lines = track.lines.entries.associate { (id, line) -> id.toInt() to AuxLine(line.text, line.words.map { it.toLogic() }) },
            )
        },
        format = when (source.format) {
            FFormat.LRC -> LyricFormat.Lrc
            FFormat.ENHANCED_LRC -> LyricFormat.EnhancedLrc
            FFormat.TTML -> LyricFormat.Ttml
            FFormat.WEB_VTT -> LyricFormat.WebVtt
            FFormat.PLAIN -> LyricFormat.Plain
        },
        origin = source.origin,
    )

    fun FLyricsSnapshot.toLogic(lookup: (org.jellyfesh.ffi.LyricTimeline) -> org.jellyfesh.logic.time.LyricTimelineLookup): LyricsSnapshot =
        LyricsSnapshot(
            uid = uid.toLong(),
            doc = doc?.toLogic(),
            timeline = timeline?.let(lookup),
            source = when (val s = source) {
                is FLyricsSource.Sidecar -> LyricsSource.Sidecar(s.path)
                FLyricsSource.Embedded -> LyricsSource.Embedded
                // Lyrics from a plugin: its attribution is in the document's origin.
                is FLyricsSource.Plugin -> LyricsSource.Plugin(s.id)
                is FLyricsSource.Made -> LyricsSource.Made(s.plugin, s.engine)
                null -> null
            },
        )

    fun FTheme.toLogic(): Theme = Theme(
        hue = hue,
        appearance = when (appearance) {
            FAppearance.LIGHT -> Appearance.Light
            FAppearance.DARK -> Appearance.Dark
        },
        colors = colors.associate { it.name to ColorMath.rgbaToArgb(it.rgba.toLong()) },
    )

    fun FThemeSnapshot.toLogic(): ThemeSnapshot = ThemeSnapshot(
        uid = uid?.toLong(),
        hue = hue,
        fromArtwork = fromArtwork,
        light = light.toLogic(),
        dark = dark.toLogic(),
        backdropLight = backdropLight,
        backdropDark = backdropDark,
    )

    fun FScanStatus.toLogic(): ScanStatus = when (this) {
        is FScanStatus.Progress -> ScanStatus.Progress(progress.filesSeen.toLong(), progress.filesChanged.toLong(), progress.current)
        is FScanStatus.Finished -> ScanStatus.Finished(
            report.added.toLong(),
            report.updated.toLong(),
            report.removed.toLong(),
            report.unchanged.toLong(),
            report.errors.size,
        )
        is FScanStatus.Failed -> ScanStatus.Failed(message)
    }

    fun FSettings.toLogic(): Settings = Settings(
        appearance = when (appearance) {
            FAppearancePref.SYSTEM -> AppearancePref.System
            FAppearancePref.LIGHT -> AppearancePref.Light
            FAppearancePref.DARK -> AppearancePref.Dark
        },
        style = when (style) {
            FBackdropStyle.ARTWORK -> BackdropStyle.Artwork
            FBackdropStyle.MISTY -> BackdropStyle.Misty
        },
        appIcon = appIcon,
        hueSource = when (val hue = hueSource) {
            FHueSource.Artwork -> HueSource.Artwork
            is FHueSource.Fixed -> HueSource.Fixed(hue.hue)
        },
        reduceEffects = reduceEffects,
        transparency = transparency,
        lyricsSize = when (lyricsSize) {
            FLyricsSize.SMALL -> LyricsSize.Small
            FLyricsSize.MEDIUM -> LyricsSize.Medium
            FLyricsSize.LARGE -> LyricsSize.Large
        },
        lyricsAlign = when (lyricsAlign) {
            FLyricsAlign.START -> LyricsAlign.Start
            FLyricsAlign.CENTER -> LyricsAlign.Center
        },
        showTranslation = showTranslation,
        showRomanization = showRomanization,
        crossfade = CrossfadeSettings(crossfade.enabled, crossfade.durationMs.toInt(), crossfade.albumContinuity),
        replaygainMode = replaygainMode.toLogic(),
        preampDb = preampDb,
        eq = eq.toLogic(),
        outputDevice = outputDevice,
        resumeOnLaunch = resumeOnLaunch,
        watchFolders = watchFolders,
        allowCommunityPlugins = allowCommunityPlugins,
        checkForUpdates = checkForUpdates,
        showLyricsBar = showLyricsBar,
        desktopLyrics = desktopLyrics,
    )

    fun FDeviceInfo.toLogic(): DeviceInfo = DeviceInfo(id, name, isDefault, sampleRates.map { it.toInt() })

    fun FEqPreset.toLogic(): EqPreset = EqPreset(id = id, settings = settings.toLogic())

    fun FDevicesSnapshot.toLogic(): DevicesSnapshot =
        DevicesSnapshot(version.toLong(), devices.map { it.toLogic() }, selected)

    fun FAuthMethod.toLogic(): AuthMethod = when (this) {
        FAuthMethod.QR -> AuthMethod.Qr
        FAuthMethod.OAUTH -> AuthMethod.Oauth
        FAuthMethod.COOKIE_IMPORT -> AuthMethod.CookieImport
        FAuthMethod.PASSWORD_EXCHANGE -> AuthMethod.PasswordExchange
        FAuthMethod.API_KEY -> AuthMethod.ApiKey
    }

    fun FPluginSummary.toLogic(): PluginSummary = PluginSummary(
        id = id,
        name = name,
        version = version,
        author = author,
        badge = when (badge) {
            FPluginBadge.OFFICIAL -> PluginBadge.Official
            FPluginBadge.COMMUNITY -> PluginBadge.Community
        },
        vendorPlayback = vendorPlayback,
        capabilities = capabilities,
        authMethods = authMethods.map { it.toLogic() },
        enabled = enabled,
        account = account,
        needsDisclosure = needsDisclosure,
        settingsSchemaJson = settingsSchemaJson,
        settingsJson = settingsJson,
        icon = icon,
        remotePlayback = remotePlayback,
        attribution = attribution,
        kind = kind.toLogic(),
        requiresPro = requiresPro,
        permissions = permissions,
        discover = discover,
        details = details,
        comments = comments,
    )

    fun FSourcePreset.toLogic(): SourcePreset = SourcePreset(
        id = id,
        name = name,
        plugin = plugin,
        authMethods = authMethods.map { it.toLogic() },
        needsAddress = needsAddress,
        address = address,
        addressExample = addressExample,
        account = account,
    )

    fun FSourcesSnapshot.toLogic(): SourcesSnapshot =
        SourcesSnapshot(version.toLong(), plugins.map { it.toLogic() }, presets.map { it.toLogic() }, allowCommunity, canInstallPlugins)

    private fun FAuthField.toLogic(): AuthField = AuthField(
        name = name,
        label = label,
        secret = secret,
        placeholder = placeholder,
        kind = when (kind) {
            FAuthFieldKind.TEXT -> AuthFieldKind.Text
            FAuthFieldKind.EMAIL -> AuthFieldKind.Email
            FAuthFieldKind.URL -> AuthFieldKind.Url
            FAuthFieldKind.NUMBER -> AuthFieldKind.Number
            FAuthFieldKind.COOKIE_HEADER -> AuthFieldKind.CookieHeader
        },
        required = required,
        help = help,
    )

    fun FAuthUpdate.toLogic(): AuthUpdate = AuthUpdate(
        plugin = plugin,
        flow = flow.toLong(),
        step = when (val s = step) {
            is FAuthStep.ShowQr -> AuthStep.ShowQr(s.data, s.expiresAtMs, s.hint)
            // A code to type on another device (Jellyfin's Quick Connect), shown large.
            is FAuthStep.ShowCode -> AuthStep.ShowCode(s.code, s.hint, s.expiresAtMs)
            is FAuthStep.OpenUrl -> AuthStep.OpenUrl(s.url)
            is FAuthStep.AskFields -> AuthStep.AskFields(s.fields.map { it.toLogic() })
            is FAuthStep.Waiting -> AuthStep.Waiting(s.message)
            is FAuthStep.Done -> AuthStep.Done(s.account)
            is FAuthStep.Failed -> AuthStep.Failed(s.message)
        },
    )

    fun FSourceItem.toLogic(): SourceItem = SourceItem(
        source = source,
        kind = when (kind) {
            FItemKind.TRACK -> ItemKind.Track
            FItemKind.ALBUM -> ItemKind.Album
            FItemKind.ARTIST -> ItemKind.Artist
            FItemKind.PLAYLIST -> ItemKind.Playlist
            FItemKind.FOLDER -> ItemKind.Folder
        },
        id = id,
        title = title,
        subtitle = subtitle,
        album = album,
        durationMs = durationMs?.toLong(),
        artwork = artwork?.toLogic(),
        playable = playable,
        unplayableReason = unplayableReason,
        track = track?.toLogic(),
        alsoIn = alsoIn,
        link = link,
        artistId = artistId,
        albumId = albumId,
    )

    fun FRemoteDevice.toLogic(): RemoteDevice = RemoteDevice(
        id = id,
        name = name,
        kind = when (kind) {
            FRemoteDeviceKind.COMPUTER -> RemoteDeviceKind.Computer
            FRemoteDeviceKind.SMARTPHONE -> RemoteDeviceKind.Smartphone
            FRemoteDeviceKind.SPEAKER -> RemoteDeviceKind.Speaker
            FRemoteDeviceKind.TV -> RemoteDeviceKind.Tv
            FRemoteDeviceKind.CAR -> RemoteDeviceKind.Car
            FRemoteDeviceKind.OTHER -> RemoteDeviceKind.Other
        },
        active = active,
        volumePercent = volumePercent?.toInt(),
    )

    fun FRemoteSnapshot.toLogic(): RemoteSnapshot = RemoteSnapshot(
        version = version.toLong(),
        source = source,
        sourceName = sourceName,
        status = status.toLogic(),
        item = item?.toLogic(),
        context = context?.toLogic(),
        anchor = anchor.toLogic(),
        shuffle = shuffle,
        repeat = repeat.toLogic(),
        device = device?.toLogic(),
        devices = devices.map { it.toLogic() },
        error = error,
    )

    fun FSearchUpdate.toLogic(): SearchUpdate =
        SearchUpdate(request.toLong(), query, source, items.map { it.toLogic() }, error, done)

    fun FBrowseUpdate.toLogic(): BrowseUpdate =
        BrowseUpdate(request.toLong(), plugin, node, items.map { it.toLogic() }, nextPage?.toInt(), error)

    fun FTrack.toLogic(): Track = Track(
        id = id,
        path = path,
        title = title,
        artist = artist,
        album = album,
        albumArtist = albumArtist,
        albumId = albumId,
        trackNo = trackNo?.toInt(),
        discNo = discNo?.toInt(),
        year = year,
        genre = genre,
        durationMs = durationMs?.toLong(),
        sampleRate = sampleRate?.toInt(),
        channels = channels?.toInt(),
        bitDepth = bitDepth?.toInt(),
        codec = codec,
        artworkHash = artworkHash,
    )

    fun FAlbum.toLogic(): Album = Album(id, title, albumArtist, year, trackCount.toInt(), artworkHash)

    fun FArtist.toLogic(): Artist = Artist(id, name, trackCount.toInt())

    fun FSearchResults.toLogic(): SearchResults =
        SearchResults(tracks.map { it.toLogic() }, albums.map { it.toLogic() }, artists.map { it.toLogic() })

    fun FLibraryStats.toLogic(): LibraryStats = LibraryStats(
        roots.toLong(),
        tracks.toLong(),
        albums.toLong(),
        artists.toLong(),
        artwork.toLong(),
        totalDurationMs.toLong(),
        totalBytes.toLong(),
    )

    fun FTrackRow.toLogic(): TrackRow = TrackRow(
        track = track.toLogic(),
        title = title,
        artist = artist,
        album = album,
        durationMs = durationMs?.toLong(),
        artwork = artwork?.toLogic(),
        artworkHash = artworkHash,
        favorite = favorite,
        missing = missing,
    )

    fun FPlaylistSummary.toLogic(): PlaylistSummary =
        PlaylistSummary(id, name, entryCount.toInt(), durationMs.toLong(), updatedAtMs, artworkHash)

    fun FPlaylistEntry.toLogic(): PlaylistEntry = PlaylistEntry(entryId, position.toInt(), row.toLogic())

    fun FHistoryEntry.toLogic(): HistoryEntry = HistoryEntry(track.toLogic(), playedAtMs)
}
