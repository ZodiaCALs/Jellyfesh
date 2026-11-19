package org.jellyfesh.logic.core

// Plain Kotlin mirrors of the core's v0.0.7 types (core/jf-ffi/src/types.rs, docs/core-api.md):
// how the build was distributed, Jellyfesh Pro, updates, AI Lyrics, the look, Discover, details,
// comments, source covers and the decoded format. Unsigned values become Long or Int as in
// Model.kt; colours keep the core's 0xRRGGBB.

// ---- distribution and Pro (ADR-0021) -----------------------------------------------------------

/** How this build reached the person: decides where Pro is sold and whether the core checks for updates. */
enum class Distribution { Direct, AppStore, MicrosoftStore, GooglePlay, Web, Source }

/** A store that sells Jellyfesh Pro. */
enum class Store { AppStore, MicrosoftStore, GooglePlay }

/** What a store says about Jellyfesh Pro, as the app's store glue verified it with the platform. */
data class StoreEntitlement(
    val store: Store,
    /** The store's product id (`pro` on Google Play). */
    val product: String,
    /** Whether the person owns it now (false after a refund or revocation). */
    val active: Boolean,
    /** The store's order id, when it gives one. */
    val transaction: String?,
)

/** Where Pro comes from. */
sealed interface ProSource {
    data object None : ProSource

    /** "Pro features are free during the public beta." */
    data object Preview : ProSource

    /** "Jellyfesh Pro · Purchased". */
    data class Purchased(val store: Store) : ProSource

    /** "Jellyfesh Pro · Licence …ABCD". */
    data class Licence(val idSuffix: String, val issuedAtMs: Long) : ProSource
}

/** Why a licence key was not accepted (the apps word them, docs/ui.md). */
enum class LicenceError { Malformed, UnknownKey, BadSignature, WrongProduct, Revoked }

/** The Pro slice. */
data class ProSnapshot(
    val version: Long,
    val active: Boolean,
    val source: ProSource,
    /** A Pro Preview build. */
    val preview: Boolean,
    val licenceError: LicenceError?,
    /** The stores this build sells Pro in: Buy and Restore Purchases only for these. */
    val stores: List<Store>,
    /** Licence keys work in this build: Enter Licence Key… (false in App Store and Google Play builds). */
    val licenceKeys: Boolean,
) {
    companion object {
        val Inactive = ProSnapshot(0, false, ProSource.None, false, null, emptyList(), true)
    }
}

/** A newer release. */
data class UpdateInfo(val version: String, val url: String)

/** The Update slice (direct downloads). */
data class UpdateSnapshot(
    val version: Long,
    /** Show Check for Updates and its setting. */
    val supported: Boolean,
    val checking: Boolean,
    val available: UpdateInfo?,
    val lastCheckedMs: Long?,
    val auto: Boolean,
    val error: String?,
) {
    companion object {
        val Unsupported = UpdateSnapshot(0, false, false, null, null, false, null)
    }
}

/** Where Settings lists a plugin: a music source (Sources) or an extension such as AI Lyrics or Customize. */
enum class PluginKind { Source, Extension }

// ---- AI Lyrics (ADR-0019) ----------------------------------------------------------------------

enum class AiLyricsMode { OnDevice, Online }

/** What making lyrics would do now. */
data class AiLyricsPlan(
    val mode: AiLyricsMode,
    /** In the plugin's words: "Whisper Base on this device". */
    val engine: String,
    /** The origin the online mode sends audio to. */
    val uploadTo: String?,
    val automatic: Boolean,
)

/** What must happen before lyrics can be made. */
sealed interface AiLyricsNeed {
    data object Nothing : AiLyricsNeed
    data object Pro : AiLyricsNeed
    data object SignIn : AiLyricsNeed
    data class Consent(val origin: String) : AiLyricsNeed
    data class Model(val id: String, val name: String, val bytes: Long) : AiLyricsNeed
    data class Unavailable(val reason: String) : AiLyricsNeed
}

/** Where a lyrics job is. */
sealed interface LyricsJobStage {
    data object Waiting : LyricsJobStage
    data object ReadingAudio : LyricsJobStage
    data class Sending(val origin: String) : LyricsJobStage
    data class Recognising(val fraction: Float) : LyricsJobStage
    data object TimingWords : LyricsJobStage
    data object Done : LyricsJobStage
    data class Failed(val message: String) : LyricsJobStage
    data object Cancelled : LyricsJobStage
}

data class LyricsJob(
    val id: Long,
    val title: String,
    val artist: String?,
    /** The queue entry it is for, when it was made for the song playing. */
    val uid: Long?,
    /** The library track it is for. */
    val libraryId: Long?,
    val stage: LyricsJobStage,
)

sealed interface SpeechModelStatus {
    data object NotDownloaded : SpeechModelStatus
    data class Downloading(val fraction: Float) : SpeechModelStatus
    data object Ready : SpeechModelStatus
    data class Failed(val message: String) : SpeechModelStatus
}

data class SpeechModel(val id: String, val name: String, val bytes: Long, val state: SpeechModelStatus)

/** An origin the person allowed a plugin to send audio to. */
data class AudioConsent(val plugin: String, val origin: String)

/** The AiLyrics slice. */
data class AiLyricsSnapshot(
    val version: Long,
    /** Offer Create Synced Lyrics ([need] may still ask for something first). */
    val available: Boolean,
    val plugin: String?,
    val plan: AiLyricsPlan?,
    val need: AiLyricsNeed,
    val jobs: List<LyricsJob>,
    val models: List<SpeechModel>,
    val consents: List<AudioConsent>,
) {
    companion object {
        val Empty = AiLyricsSnapshot(0, false, null, null, AiLyricsNeed.Nothing, emptyList(), emptyList(), emptyList())
    }
}

/** A song to make lyrics for. */
sealed interface SongRef {
    /** The song playing now. */
    data object Current : SongRef

    data class Library(val id: Long) : SongRef
}

// ---- the look (ADR-0020) -----------------------------------------------------------------------

enum class Typeface { System, Rounded, Serif, Mono }

enum class Density { Compact, Standard, Roomy }

enum class LyricsWeight { Regular, Medium, Semibold, Bold, Heavy }

/** The colour of the sung part of lyrics. */
sealed interface LyricsHighlight {
    data object Artwork : LyricsHighlight
    data object Accent : LyricsHighlight

    /** `0xRRGGBB`. */
    data class Custom(val rgb: Int) : LyricsHighlight
}

enum class Place { Home, Albums, Artists, Songs, Playlists, Favourites, Search, Sources }

enum class HomeSection { Summary, RecentlyAdded, RecentlyPlayed, MostPlayed, Favourites }

enum class NowPlayingLayout { Stage, Artwork, Lyrics }

enum class MiniPlayerStyle { Bar, Compact, Floating }

/** The Look slice: every value resolved, so the app applies one thing (ADR-0020 §2). */
data class LookSnapshot(
    val version: Long,
    /** Radii ×, 0–2. */
    val cornerScale: Float,
    /** Glass alpha and blur ×, 0–1 (Reduce Transparency and the Transparency setting still win). */
    val glass: Float,
    /** The field's strength, 0–1. */
    val aurora: Float,
    val typeface: Typeface,
    /** Every type size ×, 0.85–1.30. */
    val textScale: Float,
    val density: Density,
    val lyricsWeight: LyricsWeight,
    /** 0–1. */
    val lyricsGlow: Float,
    val lyricsHighlight: LyricsHighlight,
    /** The places shown, in order (Settings is never listed). */
    val sidebar: List<Place>,
    /** Home's sections, in order. */
    val home: List<HomeSection>,
    val nowPlaying: NowPlayingLayout,
    val miniPlayer: MiniPlayerStyle,
    /** An appearance plugin sets the look; false: every value is the Luminous default. */
    val customized: Boolean,
    val plugin: String?,
) {
    companion object {
        /** The Luminous defaults (`LookSnapshot::defaults`), before the core's first Look slice. */
        val Default = LookSnapshot(
            version = 0,
            cornerScale = 1f,
            glass = 1f,
            aurora = 0.75f,
            typeface = Typeface.System,
            textScale = 1f,
            density = Density.Standard,
            lyricsWeight = LyricsWeight.Bold,
            lyricsGlow = 0.6f,
            lyricsHighlight = LyricsHighlight.Artwork,
            sidebar = Place.entries,
            home = listOf(HomeSection.Summary, HomeSection.RecentlyAdded, HomeSection.RecentlyPlayed, HomeSection.MostPlayed),
            nowPlaying = NowPlayingLayout.Stage,
            miniPlayer = MiniPlayerStyle.Bar,
            customized = false,
            plugin = null,
        )
    }
}

// ---- quality, details, comments, Discover and covers (ADR-0022) ---------------------------------

/** How good what plays is, as the core judges it (the apps word it, never compute it). */
enum class QualityLabel { HiResLossless, Lossless, High, Standard }

/** The format of what plays (as decoded) or of a file. */
data class AudioFormat(
    val codec: String?,
    val sampleRateHz: Int?,
    val bitDepth: Int?,
    val channels: Int?,
    val bitrateKbps: Int?,
    val lossless: Boolean?,
    val quality: QualityLabel?,
)

/** ReplayGain values: gains in dB, peaks as linear amplitude. */
data class ReplayGainValues(val trackGainDb: Float?, val trackPeak: Float?, val albumGainDb: Float?, val albumPeak: Float?)

data class TechnicalInfo(
    val format: AudioFormat,
    val container: String?,
    val fileSizeBytes: Long?,
    val durationMs: Long?,
    val replaygain: ReplayGainValues?,
    val path: String?,
)

/** What a fact is: known keys the app words, or the service's own label. */
sealed interface FactKey {
    data object Version : FactKey
    data object Genre : FactKey
    data object Language : FactKey
    data object Released : FactKey
    data object Label : FactKey
    data object Composer : FactKey
    data object Lyricist : FactKey
    data object Arranger : FactKey
    data object Producer : FactKey
    data object Popularity : FactKey
    data object Quality : FactKey
    data object Isrc : FactKey
    data object Disc : FactKey
    data object Track : FactKey
    data class Other(val label: String) : FactKey
}

data class Fact(val key: FactKey, val value: String)

/** An item to show details or comments for. */
sealed interface ItemRef {
    data class Track(val id: Long) : ItemRef
    data class Album(val id: Long) : ItemRef
    data class Artist(val id: Long) : ItemRef

    /** An audio file that is not in the library. */
    data class File(val path: String) : ItemRef

    /** An item a source listed: its `SourceItem.source`, kind and id. */
    data class Source(val plugin: String, val kind: ItemKind, val id: String) : ItemRef
}

/** An item's details (`Command.Details`). */
data class DetailsPage(
    val request: Long,
    val item: ItemRef,
    val title: String,
    val subtitle: String?,
    /** Album notes, an artist's biography. */
    val description: String?,
    val image: ArtworkRef?,
    val facts: List<Fact>,
    val technical: TechnicalInfo?,
    val commentCount: Long?,
    val attribution: String?,
    val error: String?,
)

data class CommentEntry(
    val id: String,
    val author: String,
    val avatar: ArtworkRef?,
    val text: String,
    val likes: Long?,
    val timeMs: Long?,
    val replies: Long?,
)

/** A page of comments (`Command.Comments`): the hot ones on the first page only. */
data class CommentsPage(
    val request: Long,
    val item: ItemRef,
    val hot: List<CommentEntry>,
    val comments: List<CommentEntry>,
    val total: Long?,
    /** The page to ask for next. */
    val next: String?,
    val attribution: String?,
    val error: String?,
)

enum class ShelfStyle { Row, Grid, Chart }

/** One shelf of a Discover page; [more] is a node to browse for See All. */
data class Shelf(val id: String, val title: String, val style: ShelfStyle, val items: List<SourceItem>, val more: String?)

/** A Discover page (`Command.Discover`). */
data class DiscoverPage(val request: Long, val source: String, val shelves: List<Shelf>, val error: String?)

/** A source picture asked for with `sourceArtwork` is cached ([path]), or cannot be had (null). */
data class ArtworkReady(val url: String, val sizePx: Int, val path: String?)
