package org.jellyfesh.logic.library

import java.util.Locale
import kotlin.math.abs
import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DetailsPage
import org.jellyfesh.logic.core.Fact
import org.jellyfesh.logic.core.FactKey
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.ReplayGainValues
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.TrackRef

/** An item's details as they load (`Command.Details`, answered by `Event::Details`). */
data class DetailsState(val request: Long = 0, val page: DetailsPage? = null, val loading: Boolean = false, val error: String? = null)

/** "FLAC · 24-bit · 96 kHz · Stereo": the parts the app words. */
data class FormatParts(val codec: String?, val bitDepth: Int?, val sampleRate: String?, val channels: Int?)

/**
 * Details (docs/ui.md, "Details"; ADR-0022): Get Info and the Info pane. The core gathers the
 * facts (tags and files for the library, the plugin's `details` for sources); this matches the
 * answer to the question and orders what the sheet shows. The app words the known keys.
 */
object DetailsModel {
    fun load(item: ItemRef, send: (Command) -> Long): DetailsState = DetailsState(request = send(Command.Details(item)), loading = true)

    fun onPage(state: DetailsState, page: DetailsPage): DetailsState {
        if (page.request != state.request) return state
        return state.copy(page = page, loading = false, error = page.error?.takeIf { it.isNotBlank() })
    }

    fun onFailed(state: DetailsState, id: Long, message: String): DetailsState =
        if (id == state.request && state.loading) state.copy(loading = false, error = message) else state

    /** What to ask about a song: its library id, its file, or its source's item. */
    fun itemFor(track: TrackRef): ItemRef = when (track) {
        is TrackRef.Library -> ItemRef.Track(track.id)
        is TrackRef.File -> ItemRef.File(track.path)
        is TrackRef.Source -> ItemRef.Source(track.track.plugin, ItemKind.Track, track.track.id)
    }

    /** What to ask about an item a source listed. */
    fun itemFor(item: SourceItem): ItemRef = ItemRef.Source(item.source, item.kind, item.id)

    /** The song playing, to ask about (null while nothing plays). */
    fun itemFor(now: NowPlaying?): ItemRef? = now?.track?.let(::itemFor)

    /** A known fact's value, or null. */
    fun fact(page: DetailsPage?, key: FactKey): String? = page?.facts?.firstOrNull { it.key == key }?.value?.takeIf { it.isNotBlank() }

    /** The known keys About This Song shows, in its order (docs/ui.md): Version, Year, Genre, Composer, Lyricist, Label. */
    val ABOUT_KEYS: List<FactKey> = listOf(FactKey.Version, FactKey.Released, FactKey.Genre, FactKey.Composer, FactKey.Lyricist, FactKey.Label)

    /** The facts shown after those: the other known ones and the service's own labels, in the core's order. */
    fun moreFacts(page: DetailsPage?): List<Fact> =
        page?.facts.orEmpty().filter { it.key !in ABOUT_KEYS && it.key != FactKey.Quality && it.value.isNotBlank() }

    fun formatParts(format: AudioFormat): FormatParts = FormatParts(
        codec = format.codec?.takeIf { it.isNotBlank() }?.uppercase(Locale.ROOT),
        bitDepth = format.bitDepth?.takeIf { it > 0 },
        sampleRate = format.sampleRateHz?.takeIf { it > 0 }?.let(org.jellyfesh.logic.format.Labels::sampleRate),
        channels = format.channels?.takeIf { it > 0 },
    )

    /** "−6.2 dB" with a true minus sign, for ReplayGain. */
    fun gain(db: Float): String {
        val text = String.format(Locale.ROOT, "%.1f", abs(db))
        return when {
            db < -0.05f -> "−$text dB"
            db > 0.05f -> "+$text dB"
            else -> "0.0 dB"
        }
    }

    /** The track's and the album's gain, when known: "−6.2 dB", "−7.0 dB". */
    fun gains(values: ReplayGainValues?): Pair<String?, String?> =
        (values?.trackGainDb?.takeIf { it.isFinite() }?.let(::gain)) to (values?.albumGainDb?.takeIf { it.isFinite() }?.let(::gain))
}
