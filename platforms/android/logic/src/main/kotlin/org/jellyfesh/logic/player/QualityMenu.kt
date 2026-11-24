package org.jellyfesh.logic.player

import org.jellyfesh.logic.core.AudioFormat
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.QualityLabel
import org.jellyfesh.logic.core.QualityNote
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.StreamingQuality
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.state.CoreState

/** What the quality badge says: the core's label for what plays, else the chosen level's name. */
sealed interface QualityBadgeText {
    data class Label(val label: QualityLabel) : QualityBadgeText

    data class Level(val level: StreamingQuality) : QualityBadgeText
}

/**
 * The format line's facts ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"): the codec, then the bit
 * depth and the sample rate for lossless audio, the bitrate for the rest. The app words the bits
 * and the bitrate.
 */
data class QualityFormat(val codec: String?, val bitDepth: Int?, val sampleRate: String?, val bitrateKbps: Int?)

/** The quality badge and its menu for what plays (ADR-0026 §3). */
data class QualityMenu(
    val badge: QualityBadgeText,
    /** Null before the engine has opened the song, and while a remote session runs. */
    val format: QualityFormat?,
    /** The levels to choose from, Auto first; empty when the source offers none. */
    val levels: List<StreamingQuality>,
    /** Streaming Quality, checked among [levels]. */
    val chosen: StreamingQuality,
    /** Why [levels] is empty: "Files play as they are." or "Quality is set in {Service}."; null when it is not. */
    val note: QualityNote?,
)

/**
 * The quality badge and its menu (ADR-0026 §3; docs/ui.md, "The player bar and the mini player"),
 * as Jellyfesh Web and the macOS app show them: the words the app puts on what the core decides.
 * The badge shows the playing quality's label (the core's `QualityLabel`, as in Get Info), else
 * the chosen level. The menu has the format line, then the levels the core offers for what plays
 * (`NowPlaying.qualities`, Auto first) with Streaming Quality checked, or why there are none.
 * While a remote session runs the service sets the quality.
 */
object QualityMenus {
    /** The badge and menu for what plays, or null with nothing playing. */
    fun of(state: CoreState): QualityMenu? {
        val chosen = state.settings.streamingQuality
        RemoteModel.session(state)?.let { session ->
            return QualityMenu(
                badge = QualityBadgeText.Level(chosen),
                format = null,
                levels = emptyList(),
                chosen = chosen,
                note = note(emptyList(), state.remote.qualityNote ?: QualityNote.SetInService(session.service)),
            )
        }
        val now = state.nowPlaying ?: return null
        val levels = levels(now.qualities)
        val label = now.quality?.quality
        return QualityMenu(
            badge = if (label != null) QualityBadgeText.Label(label) else QualityBadgeText.Level(chosen),
            format = now.quality?.let(::format),
            levels = levels,
            chosen = chosen,
            note = note(levels, now.qualityNote),
        )
    }

    /** Auto, then the levels a source offers, best first; none when it offers none. */
    fun levels(offered: List<StreamingQuality>): List<StreamingQuality> {
        val levels = offered.filter { it != StreamingQuality.Auto }.distinct()
        return if (levels.isEmpty()) emptyList() else listOf(StreamingQuality.Auto) + levels
    }

    /** Why there are no levels; a note naming no service says files play as they are. */
    fun note(levels: List<StreamingQuality>, note: QualityNote?): QualityNote? = when {
        levels.isNotEmpty() -> null
        note is QualityNote.SetInService && note.service.isNotBlank() -> note
        else -> QualityNote.FilesPlayAsTheyAre
    }

    /** The format line's facts, or null when the core knows none of them yet. */
    fun format(format: AudioFormat): QualityFormat? {
        val codec = format.codec?.trim()?.takeIf { it.isNotEmpty() }?.uppercase(java.util.Locale.ROOT)
        val lossless = format.lossless ?: (format.bitDepth != null)
        val facts = if (lossless) {
            QualityFormat(codec, format.bitDepth?.takeIf { it > 0 }, format.sampleRateHz?.takeIf { it > 0 }?.let(Labels::sampleRate), null)
        } else {
            QualityFormat(codec, null, null, format.bitrateKbps?.takeIf { it > 0 })
        }
        return facts.takeUnless { it == QualityFormat(null, null, null, null) }
    }

    /** Choosing a level: the same setting as Settings › Playback › Streaming; null when it is already chosen. */
    fun choose(settings: Settings, level: StreamingQuality): Command? =
        if (level == settings.streamingQuality) null else Command.SetSettings(settings.copy(streamingQuality = level))
}
