package org.jellyfesh.logic.core

import org.jellyfesh.logic.time.LyricTimelineLookup

// Plain Kotlin mirrors of the core's types (core/jf-ffi/src/types.rs, docs/core-api.md).
//
// The shell's state machines work on these, not on the generated UniFFI classes, so they build
// and are tested without the native library. The app's FFI adapter (app/src/ffi) is the only code
// that converts between the two. Unsigned core values become Long (ids, uids, versions, times)
// or Int (counts, indices, sample rates); times keep the core's units in their names.

// ---- tracks, collections, the queue ------------------------------------------------------------

/** What a queue entry plays. */
sealed interface TrackRef {
    data class Library(val id: Long) : TrackRef
    data class File(val path: String) : TrackRef
    data class Source(val track: SourceTrack) : TrackRef
}

/** A track from an online source, with the display data the source gave. */
data class SourceTrack(
    val plugin: String,
    val id: String,
    val title: String,
    val artist: String? = null,
    val album: String? = null,
    val durationMs: Long? = null,
    val artworkUrl: String? = null,
    val isrc: String? = null,
)

/** One queue entry; `uid` is unique for the entry's whole life (rows are keyed by it). */
data class QueueItem(val uid: Long, val track: TrackRef)

enum class RepeatMode { Off, All, One }

data class HistoryEntry(val track: TrackRef, val playedAtMs: Long)

/** A group of tracks that plays as a unit (the core's `Collection`). */
sealed interface CollectionRef {
    data class Album(val id: Long) : CollectionRef
    data class Artist(val id: Long) : CollectionRef
    data class Playlist(val id: Long) : CollectionRef
    data object Favorites : CollectionRef

    /** Every library song in the Songs page's order: Play and Shuffle for all songs, with no paging. */
    data class Songs(val sort: TrackSort, val descending: Boolean) : CollectionRef
    data class Tracks(val tracks: List<TrackRef>) : CollectionRef
    data class Source(val plugin: String, val id: String) : CollectionRef
}

sealed interface SearchScope {
    data object All : SearchScope
    data object Library : SearchScope
    data class Plugin(val id: String) : SearchScope
}

enum class AuthMethod { Qr, Oauth, CookieImport, PasswordExchange, ApiKey }

// ---- settings ----------------------------------------------------------------------------------

enum class AppearancePref { System, Light, Dark }

/** The Now Playing backdrop (ADR-0013: both are built so they can be compared). */
enum class BackdropStyle { Artwork, Misty }

sealed interface HueSource {
    data object Artwork : HueSource
    data class Fixed(val hue: Float) : HueSource
}

/** The size of lyrics on Now Playing: lines, translations and romanizations scale together. */
enum class LyricsSize { Small, Medium, Large }

/** How lyrics lines line up on Now Playing: the start edge (left in left-to-right text) or centred. */
enum class LyricsAlign { Start, Center }

enum class ReplayGainMode { Off, Track, Album }

enum class BandKind { Peaking, LowShelf, HighShelf }

data class EqBand(
    val kind: BandKind,
    val freqHz: Float,
    val gainDb: Float,
    val q: Float,
    val enabled: Boolean,
)

data class EqSettings(val enabled: Boolean, val preampDb: Float, val bands: List<EqBand>) {
    companion object {
        /** Band centres of the core's ten-band graphic presets (`EqSettings::flat`). */
        val GRAPHIC_FREQS = listOf(31f, 62f, 125f, 250f, 500f, 1_000f, 2_000f, 4_000f, 8_000f, 16_000f)

        /** The core's default: the flat preset, switched off. */
        val Default = EqSettings(
            enabled = false,
            preampDb = 0f,
            bands = GRAPHIC_FREQS.map { EqBand(BandKind.Peaking, it, 0f, 1.41f, true) },
        )
    }
}

/**
 * An equalizer preset from the core's `eq_presets()`: every app offers the same ones, in the
 * core's order. Applying one sets [Settings.eq] to its bands.
 */
data class EqPreset(
    /** Stable name (`flat`, `bass_boost`, `vocal`, `treble`); the app shows it translated. */
    val id: String,
    /** Its bands, switched on, with a 0 dB preamp. */
    val settings: EqSettings,
)

data class CrossfadeSettings(val enabled: Boolean, val durationMs: Int, val albumContinuity: Boolean) {
    companion object {
        const val MIN_MS = 1_000
        const val MAX_MS = 12_000
        val Default = CrossfadeSettings(enabled = false, durationMs = 6_000, albumContinuity = true)
    }
}

/** Everything the person sets; the shell applies the appearance parts, the core the rest. */
data class Settings(
    val appearance: AppearancePref = AppearancePref.System,
    val style: BackdropStyle = BackdropStyle.Artwork,
    /** `automatic` or one of the ten icon ids (ADR-0017). */
    val appIcon: String = "automatic",
    val hueSource: HueSource = HueSource.Artwork,
    val reduceEffects: Boolean = false,
    /** How see-through glass is, 0 (solid) to 1 (clearest); 0.5 keeps the tokens' alphas. */
    val transparency: Float = DEFAULT_TRANSPARENCY,
    val lyricsSize: LyricsSize = LyricsSize.Medium,
    val lyricsAlign: LyricsAlign = LyricsAlign.Start,
    val showTranslation: Boolean = true,
    val showRomanization: Boolean = true,
    val crossfade: CrossfadeSettings = CrossfadeSettings.Default,
    val replaygainMode: ReplayGainMode = ReplayGainMode.Off,
    val preampDb: Float = 0f,
    val eq: EqSettings = EqSettings.Default,
    /** Device id from the Devices slice; null follows the system default. */
    val outputDevice: String? = null,
    val resumeOnLaunch: Boolean = false,
    val watchFolders: Boolean = true,
    val allowCommunityPlugins: Boolean = false,
    /** Check for Updates Automatically (direct downloads only). */
    val checkForUpdates: Boolean = true,
    /** Show Lyrics Bar: the current line above the mini-player (Settings › Appearance › Lyrics). */
    val showLyricsBar: Boolean = true,
    /** Desktop Lyrics, which phones do not have; kept so a changed copy sends it back as it was. */
    val desktopLyrics: Boolean = false,
) {
    companion object {
        /** The core's default transparency: the glass tokens' own alphas. */
        const val DEFAULT_TRANSPARENCY = 0.5f
    }
}

// ---- devices and sources -----------------------------------------------------------------------

data class DeviceInfo(val id: String, val name: String, val isDefault: Boolean, val sampleRates: List<Int>)

data class DevicesSnapshot(val version: Long, val devices: List<DeviceInfo>, val selected: String?) {
    companion object {
        val Empty = DevicesSnapshot(0, emptyList(), null)
    }
}

enum class PluginBadge { Official, Community }

data class PluginSummary(
    val id: String,
    val name: String,
    val version: String,
    val author: String,
    val badge: PluginBadge,
    val vendorPlayback: Boolean,
    val capabilities: List<String>,
    val authMethods: List<AuthMethod>,
    val enabled: Boolean,
    val account: String?,
    val needsDisclosure: Boolean,
    val settingsSchemaJson: String,
    val settingsJson: String,
    val icon: String?,
    /**
     * Remote playback (ADR-0018): the service plays on the person's own devices and Jellyfesh is
     * its remote control. Playing its songs starts a remote session; they never join the queue.
     */
    val remotePlayback: Boolean = false,
    /**
     * The credit its views must show, when the service's terms ask for one: the views say
     * "Content from {Service}" (docs/ui.md), with no logos until they are approved.
     */
    val attribution: String? = null,
    /** Settings › Sources for a music source; Settings › Extensions for AI Lyrics, Customize and the like. */
    val kind: PluginKind = PluginKind.Source,
    /** It has a Jellyfesh Pro capability: its Pro calls wait for Pro. */
    val requiresPro: Boolean = false,
    /** The host imports its manifest declares (`audio`, `speech`). */
    val permissions: List<String> = emptyList(),
    /** It has a Discover page of its own (others get one the core makes). */
    val discover: Boolean = false,
    /** It describes items (biography, notes, facts). */
    val details: Boolean = false,
    /** It shows comments: Now Playing offers a Comments pane. */
    val comments: Boolean = false,
    /**
     * The core's word that the source's address is on the local network (Android's definition:
     * private, CGNAT and link-local addresses, multicast and `.local` names). On Android 17 the
     * app reaches it only with the Local Network permission ([org.jellyfesh.logic.sources.LocalNetwork]).
     * False until the generated bindings carry the core's flag.
     */
    val localNetwork: Boolean = false,
)

/**
 * A service offered in Sources → Add a Source. The core lists one only while its official plugin is
 * installed, so the list grows with the plugins and every app offers the same services.
 */
data class SourcePreset(
    /** Stable name (`jellyfin`, `navidrome`, `subsonic`): the app keys the service's description by it. */
    val id: String,
    val name: String,
    /** The plugin that connects to it. Several services can share one (Navidrome and Subsonic do). */
    val plugin: String,
    /** The plugin's sign-in methods, in its order. */
    val authMethods: List<AuthMethod>,
    /** Whether the person types the server's address (a self-hosted server). */
    val needsAddress: Boolean,
    /** The address set now, to fill the field with. */
    val address: String?,
    /** An address to show as an example in that field. */
    val addressExample: String?,
    /** The plugin's signed-in account, if any. */
    val account: String?,
)

data class SourcesSnapshot(
    val version: Long,
    val plugins: List<PluginSummary>,
    /** The services offered in Add a Source, in the core's order. */
    val presets: List<SourcePreset>,
    val allowCommunity: Boolean,
    /** Plugins may be installed from files and community plugins allowed (false in store builds). */
    val canInstallPlugins: Boolean = true,
) {
    companion object {
        val Empty = SourcesSnapshot(0, emptyList(), emptyList(), false)
    }
}

/** How a sign-in field is typed (the core's `AuthFieldKind`): the keyboard, or lines for a pasted cookie header. */
enum class AuthFieldKind { Text, Email, Url, Number, CookieHeader }

/**
 * A field a sign-in step asks for, as the plugin describes it: [secret] masks it, [required]
 * fields must be filled before Sign In (optional ones may stay empty), [help] is a sentence under
 * the input and [kind] picks how it is typed.
 */
data class AuthField(
    val name: String,
    val label: String,
    val secret: Boolean,
    val placeholder: String?,
    val kind: AuthFieldKind = AuthFieldKind.Text,
    val required: Boolean = true,
    val help: String? = null,
)

sealed interface AuthStep {
    /** A QR code for the service's app to scan; [hint] is the plugin's own sentence for it, if any. */
    data class ShowQr(val data: String, val expiresAtMs: Long?, val hint: String? = null) : AuthStep

    /** A code shown large, to type on another device ([hint] says where), as with Jellyfin's Quick Connect. */
    data class ShowCode(val code: String, val hint: String, val expiresAtMs: Long? = null) : AuthStep
    data class OpenUrl(val url: String) : AuthStep
    data class AskFields(val fields: List<AuthField>) : AuthStep
    data class Waiting(val message: String) : AuthStep
    data class Done(val account: String) : AuthStep
    data class Failed(val message: String) : AuthStep
}

data class AuthUpdate(val plugin: String, val flow: Long, val step: AuthStep)

// ---- search, browse and library rows -------------------------------------------------------------

enum class ItemKind { Track, Album, Artist, Playlist, Folder }

/** Artwork a view can load: a cached local file, or a URL from an online source. */
sealed interface ArtworkRef {
    data class File(val path: String) : ArtworkRef
    data class Url(val url: String) : ArtworkRef
}

data class SourceItem(
    val source: String,
    val kind: ItemKind,
    val id: String,
    val title: String,
    val subtitle: String?,
    val album: String?,
    val durationMs: Long?,
    val artwork: ArtworkRef?,
    val playable: Boolean,
    val unplayableReason: String?,
    val track: TrackRef?,
    val alsoIn: List<String>,
    /**
     * An `https` link that opens the item in its service ("Open in {Service}"), when its source
     * declares one; the main action of an item that cannot play here.
     */
    val link: String? = null,
    /** The source's id of a track's or album's artist: Go to Artist (`Browse` and `Details`). */
    val artistId: String? = null,
    /** The source's id of a track's album: Go to Album. */
    val albumId: String? = null,
)

data class SearchUpdate(
    val request: Long,
    val query: String,
    val source: String,
    val items: List<SourceItem>,
    val error: String?,
    val done: Boolean,
)

data class BrowseUpdate(
    val request: Long,
    val plugin: String,
    val node: String?,
    val items: List<SourceItem>,
    val nextPage: Int?,
    val error: String?,
)

/** A row of a playlist, favourites or history list. */
data class TrackRow(
    val track: TrackRef,
    val title: String,
    val artist: String?,
    val album: String?,
    val durationMs: Long?,
    val artwork: ArtworkRef?,
    val artworkHash: String?,
    val favorite: Boolean,
    val missing: Boolean,
)

data class PlaylistSummary(
    val id: Long,
    val name: String,
    val entryCount: Int,
    val durationMs: Long,
    val updatedAtMs: Long,
    val artworkHash: String?,
)

data class PlaylistEntry(val entryId: Long, val position: Int, val row: TrackRow)

data class Track(
    val id: Long,
    val path: String,
    val title: String,
    val artist: String?,
    val album: String?,
    val albumArtist: String?,
    val albumId: Long?,
    val trackNo: Int?,
    val discNo: Int?,
    val year: Int?,
    val genre: String?,
    val durationMs: Long?,
    val sampleRate: Int?,
    val channels: Int?,
    val bitDepth: Int?,
    val codec: String?,
    val artworkHash: String?,
)

data class Album(
    val id: Long,
    val title: String,
    val albumArtist: String?,
    val year: Int?,
    val trackCount: Int,
    val artworkHash: String?,
)

data class Artist(val id: Long, val name: String, val trackCount: Int)

enum class TrackSort { Artist, Title, Album, Year, Duration, Path, Added }

enum class AlbumSort { Title, Artist, Year, Added }

data class TrackQuery(
    val offset: Int = 0,
    val limit: Int = 100,
    val sort: TrackSort = TrackSort.Artist,
    val descending: Boolean = false,
    val albumId: Long? = null,
    val artistId: Long? = null,
)

data class SearchResults(val tracks: List<Track>, val albums: List<Album>, val artists: List<Artist>) {
    val isEmpty: Boolean get() = tracks.isEmpty() && albums.isEmpty() && artists.isEmpty()

    companion object {
        val Empty = SearchResults(emptyList(), emptyList(), emptyList())
    }
}

data class LibraryStats(
    val roots: Long,
    val tracks: Long,
    val albums: Long,
    val artists: Long,
    val artwork: Long,
    val totalDurationMs: Long,
    val totalBytes: Long,
)

sealed interface ScanStatus {
    data class Progress(val filesSeen: Long, val filesChanged: Long, val current: String) : ScanStatus
    data class Finished(val added: Long, val updated: Long, val removed: Long, val unchanged: Long, val errors: Int) :
        ScanStatus
    data class Failed(val message: String) : ScanStatus
}

// ---- playback ------------------------------------------------------------------------------------

enum class Status { Stopped, Playing, Paused, Ended }

/**
 * The time base for progress bars and lyrics (architecture §5): views extrapolate from it with
 * the ported `DisplayClock`, never by asking the core per frame.
 */
data class PlaybackAnchor(
    val trackGeneration: Long,
    val positionUs: Long,
    val anchorMonoNs: Long,
    val rate: Float,
    val durationUs: Long?,
) {
    companion object {
        val Zero = PlaybackAnchor(0, 0, 0, 0f, null)
    }
}

data class PlaybackSnapshot(
    val version: Long,
    val status: Status,
    val anchor: PlaybackAnchor,
    val volume: Float,
    val eq: EqSettings,
    val replaygainMode: ReplayGainMode,
    val preampDb: Float,
) {
    companion object {
        val Initial = PlaybackSnapshot(0, Status.Stopped, PlaybackAnchor.Zero, 1f, EqSettings.Default, ReplayGainMode.Off, 0f)
    }
}

data class QueueSnapshot(
    val version: Long,
    val len: Int,
    val position: Int?,
    val current: QueueItem?,
    val shuffle: Boolean,
    val repeat: RepeatMode,
    /** The next entries in playback order, at most 50; `queueWindow` pages the rest. */
    val upNext: List<QueueItem>,
) {
    companion object {
        val Empty = QueueSnapshot(0, 0, null, null, false, RepeatMode.Off, emptyList())
    }
}

data class TrackInfo(
    val codec: String,
    val sampleRate: Int,
    val channels: Int,
    val durationUs: Long?,
    val gapless: Boolean,
    val bitrate: Int?,
    /** Bits per sample of the source (PCM, FLAC, ALAC); none for lossy codecs. */
    val bitsPerSample: Int? = null,
)

data class NowPlaying(
    val uid: Long,
    val track: TrackRef,
    val path: String?,
    val title: String,
    val artist: String?,
    val album: String?,
    val durationMs: Long?,
    val artwork: ArtworkRef?,
    val format: TrackInfo?,
    /** Opens the track in its service, for source tracks whose plugin declares links. */
    val link: String? = null,
    /** What plays as decoded, with the core's quality label, once the engine has opened it. */
    val quality: AudioFormat? = null,
)

// ---- lyrics --------------------------------------------------------------------------------------

enum class Timing { Unsynced, Line, Word }

enum class AgentKind { Person, Group, Other }

data class LyricAgent(val id: String, val name: String?, val kind: AgentKind)

enum class TextDir { Ltr, Rtl, Auto }

data class LyricWord(val startMs: Long, val endMs: Long, val text: String, val joinsNext: Boolean)

data class LyricLine(
    val id: Int,
    val startMs: Long?,
    val endMs: Long?,
    val agent: String?,
    val text: String,
    val words: List<LyricWord>,
    val background: List<LyricWord>,
    val direction: TextDir,
)

enum class AuxKind { Translation, Romanization }

data class AuxLine(val text: String, val words: List<LyricWord>)

data class AuxTrack(val kind: AuxKind, val language: String?, val lines: Map<Int, AuxLine>)

enum class LyricFormat { Lrc, EnhancedLrc, Ttml, WebVtt, Plain }

data class LyricDoc(
    val language: String?,
    val timing: Timing,
    val offsetMs: Int,
    val agents: List<LyricAgent>,
    val lines: List<LyricLine>,
    val tracks: List<AuxTrack>,
    val format: LyricFormat,
    val origin: String?,
)

sealed interface LyricsSource {
    data class Sidecar(val path: String) : LyricsSource
    data object Embedded : LyricsSource

    /** From a source's plugin (its attribution is in the document's origin). */
    data class Plugin(val id: String) : LyricsSource

    /** Made by AI Lyrics: "Made by AI Lyrics · {engine}". */
    data class Made(val plugin: String, val engine: String) : LyricsSource
}

/**
 * The Lyrics slice for one track. [timeline] is the core's cursor arrays behind the ported
 * `cursor`/`nextChange` (the adapter wraps the generated `LyricTimeline`), so a view does two
 * binary searches per frame and never calls the core.
 */
data class LyricsSnapshot(
    val uid: Long,
    val doc: LyricDoc?,
    val timeline: LyricTimelineLookup?,
    val source: LyricsSource?,
)

// ---- remote sessions (ADR-0018) -------------------------------------------------------------------

/** What a remote device is; the Devices sheet shows its kind's icon. */
enum class RemoteDeviceKind { Computer, Smartphone, Speaker, Tv, Car, Other }

/** A device a remote source can play on. */
data class RemoteDevice(
    val id: String,
    val name: String,
    val kind: RemoteDeviceKind,
    /** The device playing (or last playing) for this account. */
    val active: Boolean,
    /** Its volume, 0–100, when it lets Jellyfesh change it; null hides the volume control. */
    val volumePercent: Int?,
)

/**
 * The Remote slice: the remote session while Jellyfesh controls a service's own player (a source
 * with [PluginSummary.remotePlayback]). The player views show it instead of the engine's track
 * while [source] is set.
 */
data class RemoteSnapshot(
    val version: Long,
    /** The source in control; null when there is no remote session. */
    val source: String?,
    /** That source's name, for "Open in {Service}" and the notes in Lyrics and Up Next. */
    val sourceName: String?,
    val status: Status,
    /** The track playing, as the service reports it. */
    val item: SourceItem?,
    /** The album or playlist it plays from, when the service says. */
    val context: SourceItem?,
    /** Time base for the progress bar, like the Playback slice's. */
    val anchor: PlaybackAnchor,
    val shuffle: Boolean,
    val repeat: RepeatMode,
    /** The device playing. */
    val device: RemoteDevice?,
    /** Every device the service lists, for the Devices sheet. */
    val devices: List<RemoteDevice>,
    /** What went wrong last talking to the service, until a call succeeds. */
    val error: String?,
) {
    /** Whether a remote session runs (the player views show it). */
    val isActive: Boolean get() = source != null

    companion object {
        val Empty = RemoteSnapshot(
            version = 0,
            source = null,
            sourceName = null,
            status = Status.Stopped,
            item = null,
            context = null,
            anchor = PlaybackAnchor.Zero,
            shuffle = false,
            repeat = RepeatMode.Off,
            device = null,
            devices = emptyList(),
            error = null,
        )
    }
}

// ---- theme -------------------------------------------------------------------------------------

enum class Appearance { Light, Dark }

/** Every theme colour as ARGB (`0xAARRGGBB`), keyed by token name such as `ink.ink-90`. */
data class Theme(val hue: Float, val appearance: Appearance, val colors: Map<String, Int>)

data class ThemeSnapshot(
    val uid: Long?,
    val hue: Float,
    val fromArtwork: Boolean,
    val light: Theme,
    val dark: Theme,
    val backdropLight: String?,
    val backdropDark: String?,
) {
    fun theme(dark: Boolean): Theme = if (dark) this.dark else light
    fun backdrop(dark: Boolean): String? = if (dark) backdropDark else backdropLight
}

/** Everything a UI mirrors, as `snapshot()` returns it. */
data class CoreSnapshot(
    val playback: PlaybackSnapshot,
    val queue: QueueSnapshot,
    val nowPlaying: NowPlaying?,
    val lyrics: LyricsSnapshot?,
    val theme: ThemeSnapshot?,
    val libraryRevision: Long,
    val scanning: Boolean,
    val settings: Settings,
    val devices: DevicesSnapshot,
    val sources: SourcesSnapshot,
    val muted: Boolean,
    val remote: RemoteSnapshot = RemoteSnapshot.Empty,
    val aiLyrics: AiLyricsSnapshot = AiLyricsSnapshot.Empty,
    val look: LookSnapshot = LookSnapshot.Default,
    val pro: ProSnapshot = ProSnapshot.Inactive,
    val update: UpdateSnapshot = UpdateSnapshot.Unsupported,
)
