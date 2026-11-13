package org.jellyfesh.logic.format

import java.util.Locale
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.TrackInfo
import org.jellyfesh.logic.core.TrackRef

/** Short labels built from what the core sent; never translated text (the app adds words). */
object Labels {
    /** `Artist · Album`, either part alone, or null. */
    fun artistAlbum(artist: String?, album: String?): String? {
        val parts = listOfNotNull(artist?.takeIf { it.isNotBlank() }, album?.takeIf { it.isNotBlank() })
        return parts.takeIf { it.isNotEmpty() }?.joinToString(" · ")
    }

    fun nowPlayingSubtitle(now: NowPlaying): String? = artistAlbum(now.artist, now.album)

    /**
     * What a queue row shows before the library has answered: a file's name without its
     * extension, or a source track's title.
     */
    fun provisionalTitle(track: TrackRef): String? = when (track) {
        is TrackRef.File -> track.path.substringAfterLast('/').substringBeforeLast('.').ifBlank { track.path }
        is TrackRef.Source -> track.track.title
        is TrackRef.Library -> null
    }

    /** `FLAC · 44.1 kHz · 2 ch` for the Info view (the app adds "gapless" in words). */
    fun format(info: TrackInfo): String {
        val parts = mutableListOf(info.codec.uppercase(Locale.ROOT))
        if (info.sampleRate > 0) parts += sampleRate(info.sampleRate)
        info.bitrate?.takeIf { it > 0 }?.let { parts += "${(it + 500) / 1_000} kbps" }
        if (info.channels > 0) parts += "${info.channels} ch"
        return parts.joinToString(" · ")
    }

    /** `44.1 kHz`, `48 kHz`, `96 kHz`. */
    fun sampleRate(hz: Int): String {
        val khz = hz / 1_000.0
        return if (hz % 1_000 == 0) "${hz / 1_000} kHz" else String.format(Locale.ROOT, "%.1f kHz", khz)
    }

    /** The first character to show on a placeholder tile: a letter or digit, upper-cased. */
    fun initial(title: String): String {
        val first = title.trim().codePoints().filter { Character.isLetterOrDigit(it) }.findFirst()
        return if (first.isPresent) String(Character.toChars(first.asInt)).uppercase(Locale.ROOT) else "♪"
    }
}
