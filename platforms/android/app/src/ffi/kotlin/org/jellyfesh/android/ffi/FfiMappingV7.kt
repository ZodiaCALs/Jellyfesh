package org.jellyfesh.android.ffi

// Conversions for the core's v0.0.7 types (docs/core-api.md: AI Lyrics, the look, Pro, updates,
// Discover, details, comments, covers and the decoded format), next to FfiMapping. Like it, the
// only code that names these generated types, so a renamed field is fixed here.

import org.jellyfesh.ffi.AiLyricsMode as FAiLyricsMode
import org.jellyfesh.ffi.AiLyricsNeed as FAiLyricsNeed
import org.jellyfesh.ffi.AiLyricsSnapshot as FAiLyricsSnapshot
import org.jellyfesh.ffi.ArtworkReady as FArtworkReady
import org.jellyfesh.ffi.AudioFormat as FAudioFormat
import org.jellyfesh.ffi.CommentEntry as FCommentEntry
import org.jellyfesh.ffi.CommentsPage as FCommentsPage
import org.jellyfesh.ffi.Density as FDensity
import org.jellyfesh.ffi.DetailsPage as FDetailsPage
import org.jellyfesh.ffi.DiscoverPage as FDiscoverPage
import org.jellyfesh.ffi.Distribution as FDistribution
import org.jellyfesh.ffi.FactKey as FFactKey
import org.jellyfesh.ffi.HomeSection as FHomeSection
import org.jellyfesh.ffi.ItemKind as FItemKind
import org.jellyfesh.ffi.ItemRef as FItemRef
import org.jellyfesh.ffi.LicenceError as FLicenceError
import org.jellyfesh.ffi.LookSnapshot as FLookSnapshot
import org.jellyfesh.ffi.LyricsHighlight as FLyricsHighlight
import org.jellyfesh.ffi.LyricsJobStage as FLyricsJobStage
import org.jellyfesh.ffi.LyricsWeight as FLyricsWeight
import org.jellyfesh.ffi.MiniPlayerStyle as FMiniPlayerStyle
import org.jellyfesh.ffi.NowPlayingLayout as FNowPlayingLayout
import org.jellyfesh.ffi.Place as FPlace
import org.jellyfesh.ffi.PluginKind as FPluginKind
import org.jellyfesh.ffi.ProSnapshot as FProSnapshot
import org.jellyfesh.ffi.ProSource as FProSource
import org.jellyfesh.ffi.QualityLabel as FQualityLabel
import org.jellyfesh.ffi.ShelfStyle as FShelfStyle
import org.jellyfesh.ffi.SongRef as FSongRef
import org.jellyfesh.ffi.SpeechModelStatus as FSpeechModelStatus
import org.jellyfesh.ffi.Store as FStore
import org.jellyfesh.ffi.StoreEntitlement as FStoreEntitlement
import org.jellyfesh.ffi.TechnicalInfo as FTechnicalInfo
import org.jellyfesh.ffi.Typeface as FTypeface
import org.jellyfesh.ffi.UpdateSnapshot as FUpdateSnapshot
import org.jellyfesh.android.ffi.FfiMapping.toLogic
import org.jellyfesh.logic.core.AiLyricsMode
import org.jellyfesh.logic.core.AiLyricsNeed
import org.jellyfesh.logic.core.AiLyricsPlan
import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.ArtworkReady
import org.jellyfesh.logic.core.AudioConsent
import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.CommentEntry
import org.jellyfesh.logic.core.CommentsPage
import org.jellyfesh.logic.core.Density
import org.jellyfesh.logic.core.DetailsPage
import org.jellyfesh.logic.core.DiscoverPage
import org.jellyfesh.logic.core.Distribution
import org.jellyfesh.logic.core.Fact
import org.jellyfesh.logic.core.FactKey
import org.jellyfesh.logic.core.HomeSection
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.LicenceError
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.LyricsHighlight
import org.jellyfesh.logic.core.LyricsJob
import org.jellyfesh.logic.core.LyricsJobStage
import org.jellyfesh.logic.core.LyricsWeight
import org.jellyfesh.logic.core.MiniPlayerStyle
import org.jellyfesh.logic.core.NowPlayingLayout
import org.jellyfesh.logic.core.Place
import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.ProSnapshot
import org.jellyfesh.logic.core.ProSource
import org.jellyfesh.logic.core.QualityLabel
import org.jellyfesh.logic.core.ReplayGainValues
import org.jellyfesh.logic.core.Shelf
import org.jellyfesh.logic.core.ShelfStyle
import org.jellyfesh.logic.core.SongRef
import org.jellyfesh.logic.core.SpeechModel
import org.jellyfesh.logic.core.SpeechModelStatus
import org.jellyfesh.logic.core.Store
import org.jellyfesh.logic.core.StoreEntitlement
import org.jellyfesh.logic.core.TechnicalInfo
import org.jellyfesh.logic.core.Typeface
import org.jellyfesh.logic.core.UpdateInfo
import org.jellyfesh.logic.core.UpdateSnapshot

object FfiMappingV7 {
    // ---- shell -> core ---------------------------------------------------------------------------

    fun ffi(distribution: Distribution): FDistribution = when (distribution) {
        Distribution.Direct -> FDistribution.DIRECT
        Distribution.AppStore -> FDistribution.APP_STORE
        Distribution.MicrosoftStore -> FDistribution.MICROSOFT_STORE
        Distribution.GooglePlay -> FDistribution.GOOGLE_PLAY
        Distribution.Web -> FDistribution.WEB
        Distribution.Source -> FDistribution.SOURCE
    }

    fun ffi(store: Store): FStore = when (store) {
        Store.AppStore -> FStore.APP_STORE
        Store.MicrosoftStore -> FStore.MICROSOFT_STORE
        Store.GooglePlay -> FStore.GOOGLE_PLAY
    }

    fun ffi(entitlement: StoreEntitlement): FStoreEntitlement =
        FStoreEntitlement(store = ffi(entitlement.store), product = entitlement.product, active = entitlement.active, transaction = entitlement.transaction)

    fun ffi(song: SongRef): FSongRef = when (song) {
        SongRef.Current -> FSongRef.Current
        is SongRef.Library -> FSongRef.Library(id = song.id)
    }

    fun ffi(kind: ItemKind): FItemKind = when (kind) {
        ItemKind.Track -> FItemKind.TRACK
        ItemKind.Album -> FItemKind.ALBUM
        ItemKind.Artist -> FItemKind.ARTIST
        ItemKind.Playlist -> FItemKind.PLAYLIST
        ItemKind.Folder -> FItemKind.FOLDER
    }

    fun ffi(item: ItemRef): FItemRef = when (item) {
        is ItemRef.Track -> FItemRef.Track(id = item.id)
        is ItemRef.Album -> FItemRef.Album(id = item.id)
        is ItemRef.Artist -> FItemRef.Artist(id = item.id)
        is ItemRef.File -> FItemRef.File(path = item.path)
        is ItemRef.Source -> FItemRef.Source(plugin = item.plugin, itemKind = ffi(item.kind), id = item.id)
    }

    // ---- core -> shell ---------------------------------------------------------------------------

    fun FPluginKind.toLogic(): PluginKind = when (this) {
        FPluginKind.SOURCE -> PluginKind.Source
        FPluginKind.EXTENSION -> PluginKind.Extension
    }

    fun FStore.toLogic(): Store = when (this) {
        FStore.APP_STORE -> Store.AppStore
        FStore.MICROSOFT_STORE -> Store.MicrosoftStore
        FStore.GOOGLE_PLAY -> Store.GooglePlay
    }

    fun FItemKind.toLogicKind(): ItemKind = when (this) {
        FItemKind.TRACK -> ItemKind.Track
        FItemKind.ALBUM -> ItemKind.Album
        FItemKind.ARTIST -> ItemKind.Artist
        FItemKind.PLAYLIST -> ItemKind.Playlist
        FItemKind.FOLDER -> ItemKind.Folder
    }

    fun FItemRef.toLogic(): ItemRef = when (this) {
        is FItemRef.Track -> ItemRef.Track(id)
        is FItemRef.Album -> ItemRef.Album(id)
        is FItemRef.Artist -> ItemRef.Artist(id)
        is FItemRef.File -> ItemRef.File(path)
        is FItemRef.Source -> ItemRef.Source(plugin, itemKind.toLogicKind(), id)
    }

    fun FQualityLabel.toLogic(): QualityLabel = when (this) {
        FQualityLabel.HI_RES_LOSSLESS -> QualityLabel.HiResLossless
        FQualityLabel.LOSSLESS -> QualityLabel.Lossless
        FQualityLabel.HIGH -> QualityLabel.High
        FQualityLabel.STANDARD -> QualityLabel.Standard
    }

    fun FAudioFormat.toLogic(): AudioFormat = AudioFormat(
        codec = codec,
        sampleRateHz = sampleRateHz?.toInt(),
        bitDepth = bitDepth?.toInt(),
        channels = channels?.toInt(),
        bitrateKbps = bitrateKbps?.toInt(),
        lossless = lossless,
        quality = quality?.toLogic(),
    )

    fun FTechnicalInfo.toLogic(): TechnicalInfo = TechnicalInfo(
        format = format.toLogic(),
        container = container,
        fileSizeBytes = fileSizeBytes?.toLong(),
        durationMs = durationMs?.toLong(),
        replaygain = replaygain?.let { ReplayGainValues(it.trackGainDb, it.trackPeak, it.albumGainDb, it.albumPeak) },
        path = path,
    )

    fun FAiLyricsSnapshot.toLogic(): AiLyricsSnapshot = AiLyricsSnapshot(
        version = version.toLong(),
        available = available,
        plugin = plugin,
        plan = plan?.let {
            AiLyricsPlan(
                mode = when (it.mode) {
                    FAiLyricsMode.ON_DEVICE -> AiLyricsMode.OnDevice
                    FAiLyricsMode.ONLINE -> AiLyricsMode.Online
                },
                engine = it.engine,
                uploadTo = it.uploadTo,
                automatic = it.automatic,
            )
        },
        need = when (val n = need) {
            is FAiLyricsNeed.Nothing -> AiLyricsNeed.Nothing
            is FAiLyricsNeed.Pro -> AiLyricsNeed.Pro
            is FAiLyricsNeed.SignIn -> AiLyricsNeed.SignIn
            is FAiLyricsNeed.Consent -> AiLyricsNeed.Consent(n.origin)
            is FAiLyricsNeed.Model -> AiLyricsNeed.Model(n.id, n.name, n.bytes.toLong())
            is FAiLyricsNeed.Unavailable -> AiLyricsNeed.Unavailable(n.reason)
        },
        jobs = jobs.map { job ->
            LyricsJob(
                id = job.id.toLong(),
                title = job.title,
                artist = job.artist,
                uid = job.uid?.toLong(),
                libraryId = job.libraryId,
                stage = when (val st = job.stage) {
                    is FLyricsJobStage.Waiting -> LyricsJobStage.Waiting
                    is FLyricsJobStage.ReadingAudio -> LyricsJobStage.ReadingAudio
                    is FLyricsJobStage.Sending -> LyricsJobStage.Sending(st.origin)
                    is FLyricsJobStage.Recognising -> LyricsJobStage.Recognising(st.fraction)
                    is FLyricsJobStage.TimingWords -> LyricsJobStage.TimingWords
                    is FLyricsJobStage.Done -> LyricsJobStage.Done
                    is FLyricsJobStage.Failed -> LyricsJobStage.Failed(st.message)
                    is FLyricsJobStage.Cancelled -> LyricsJobStage.Cancelled
                },
            )
        },
        models = models.map { m ->
            SpeechModel(
                id = m.id,
                name = m.name,
                bytes = m.bytes.toLong(),
                state = when (val st = m.state) {
                    is FSpeechModelStatus.NotDownloaded -> SpeechModelStatus.NotDownloaded
                    is FSpeechModelStatus.Downloading -> SpeechModelStatus.Downloading(st.fraction)
                    is FSpeechModelStatus.Ready -> SpeechModelStatus.Ready
                    is FSpeechModelStatus.Failed -> SpeechModelStatus.Failed(st.message)
                },
            )
        },
        consents = consents.map { AudioConsent(it.plugin, it.origin) },
    )

    fun FLookSnapshot.toLogic(): LookSnapshot = LookSnapshot(
        version = version.toLong(),
        cornerScale = cornerScale,
        glass = glass,
        aurora = aurora,
        typeface = when (typeface) {
            FTypeface.SYSTEM -> Typeface.System
            FTypeface.ROUNDED -> Typeface.Rounded
            FTypeface.SERIF -> Typeface.Serif
            FTypeface.MONO -> Typeface.Mono
        },
        textScale = textScale,
        density = when (density) {
            FDensity.COMPACT -> Density.Compact
            FDensity.STANDARD -> Density.Standard
            FDensity.ROOMY -> Density.Roomy
        },
        lyricsWeight = when (lyricsWeight) {
            FLyricsWeight.REGULAR -> LyricsWeight.Regular
            FLyricsWeight.MEDIUM -> LyricsWeight.Medium
            FLyricsWeight.SEMIBOLD -> LyricsWeight.Semibold
            FLyricsWeight.BOLD -> LyricsWeight.Bold
            FLyricsWeight.HEAVY -> LyricsWeight.Heavy
        },
        lyricsGlow = lyricsGlow,
        lyricsHighlight = when (val h = lyricsHighlight) {
            is FLyricsHighlight.Artwork -> LyricsHighlight.Artwork
            is FLyricsHighlight.Accent -> LyricsHighlight.Accent
            is FLyricsHighlight.Custom -> LyricsHighlight.Custom(h.rgb.toInt() and 0xFFFFFF)
        },
        sidebar = sidebar.map {
            when (it) {
                FPlace.HOME -> Place.Home
                FPlace.ALBUMS -> Place.Albums
                FPlace.ARTISTS -> Place.Artists
                FPlace.SONGS -> Place.Songs
                FPlace.PLAYLISTS -> Place.Playlists
                FPlace.FAVOURITES -> Place.Favourites
                FPlace.SEARCH -> Place.Search
                FPlace.SOURCES -> Place.Sources
            }
        },
        home = home.map {
            when (it) {
                FHomeSection.SUMMARY -> HomeSection.Summary
                FHomeSection.RECENTLY_ADDED -> HomeSection.RecentlyAdded
                FHomeSection.RECENTLY_PLAYED -> HomeSection.RecentlyPlayed
                FHomeSection.MOST_PLAYED -> HomeSection.MostPlayed
                FHomeSection.FAVOURITES -> HomeSection.Favourites
            }
        },
        nowPlaying = when (nowPlaying) {
            FNowPlayingLayout.STAGE -> NowPlayingLayout.Stage
            FNowPlayingLayout.ARTWORK -> NowPlayingLayout.Artwork
            FNowPlayingLayout.LYRICS -> NowPlayingLayout.Lyrics
        },
        miniPlayer = when (miniPlayer) {
            FMiniPlayerStyle.BAR -> MiniPlayerStyle.Bar
            FMiniPlayerStyle.COMPACT -> MiniPlayerStyle.Compact
            FMiniPlayerStyle.FLOATING -> MiniPlayerStyle.Floating
        },
        customized = customized,
        plugin = plugin,
    )

    fun FProSnapshot.toLogic(): ProSnapshot = ProSnapshot(
        version = version.toLong(),
        active = active,
        source = when (val s = source) {
            is FProSource.None -> ProSource.None
            is FProSource.Preview -> ProSource.Preview
            is FProSource.Store -> ProSource.Purchased(s.store.toLogic())
            is FProSource.Licence -> ProSource.Licence(s.idSuffix, s.issuedAtMs)
        },
        preview = preview,
        licenceError = licenceError?.let {
            when (it) {
                FLicenceError.MALFORMED -> LicenceError.Malformed
                FLicenceError.UNKNOWN_KEY -> LicenceError.UnknownKey
                FLicenceError.BAD_SIGNATURE -> LicenceError.BadSignature
                FLicenceError.WRONG_PRODUCT -> LicenceError.WrongProduct
                FLicenceError.REVOKED -> LicenceError.Revoked
            }
        },
        stores = stores.map { it.toLogic() },
        licenceKeys = licenceKeys,
    )

    fun FUpdateSnapshot.toLogic(): UpdateSnapshot = UpdateSnapshot(
        version = version.toLong(),
        supported = supported,
        checking = checking,
        available = available?.let { UpdateInfo(it.version, it.url) },
        lastCheckedMs = lastCheckedMs,
        auto = auto,
        error = error,
    )

    fun FDiscoverPage.toLogic(): DiscoverPage = DiscoverPage(
        request = request.toLong(),
        source = source,
        shelves = shelves.map { shelf ->
            Shelf(
                id = shelf.id,
                title = shelf.title,
                style = when (shelf.style) {
                    FShelfStyle.ROW -> ShelfStyle.Row
                    FShelfStyle.GRID -> ShelfStyle.Grid
                    FShelfStyle.CHART -> ShelfStyle.Chart
                },
                items = shelf.items.map { it.toLogic() },
                more = shelf.more,
            )
        },
        error = error,
    )

    fun FDetailsPage.toLogic(): DetailsPage = DetailsPage(
        request = request.toLong(),
        item = item.toLogic(),
        title = title,
        subtitle = subtitle,
        description = description,
        image = image?.toLogic(),
        facts = facts.map { fact ->
            Fact(
                key = when (val k = fact.key) {
                    is FFactKey.Version -> FactKey.Version
                    is FFactKey.Genre -> FactKey.Genre
                    is FFactKey.Language -> FactKey.Language
                    is FFactKey.Released -> FactKey.Released
                    is FFactKey.Label -> FactKey.Label
                    is FFactKey.Composer -> FactKey.Composer
                    is FFactKey.Lyricist -> FactKey.Lyricist
                    is FFactKey.Arranger -> FactKey.Arranger
                    is FFactKey.Producer -> FactKey.Producer
                    is FFactKey.Popularity -> FactKey.Popularity
                    is FFactKey.Quality -> FactKey.Quality
                    is FFactKey.Isrc -> FactKey.Isrc
                    is FFactKey.Disc -> FactKey.Disc
                    is FFactKey.Track -> FactKey.Track
                    is FFactKey.Other -> FactKey.Other(k.label)
                },
                value = fact.value,
            )
        },
        technical = technical?.toLogic(),
        commentCount = commentCount?.toLong(),
        attribution = attribution,
        error = error,
    )

    private fun FCommentEntry.toLogic(): CommentEntry =
        CommentEntry(id, author, avatar?.toLogic(), text, likes?.toLong(), timeMs, replies?.toLong())

    fun FCommentsPage.toLogic(): CommentsPage = CommentsPage(
        request = request.toLong(),
        item = item.toLogic(),
        hot = hot.map { it.toLogic() },
        comments = comments.map { it.toLogic() },
        total = total?.toLong(),
        next = next,
        attribution = attribution,
        error = error,
    )

    fun FArtworkReady.toLogic(): ArtworkReady = ArtworkReady(url, sizePx.toInt(), path)
}
