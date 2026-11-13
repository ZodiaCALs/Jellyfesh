package org.jellyfesh.logic.player

import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.PlaybackAnchor
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.state.CoreState

/** Small derived values the player views share. */
object PlayerModel {
    /** The repeat button cycles Off → All → One → Off. */
    fun nextRepeat(mode: RepeatMode): RepeatMode = when (mode) {
        RepeatMode.Off -> RepeatMode.All
        RepeatMode.All -> RepeatMode.One
        RepeatMode.One -> RepeatMode.Off
    }

    fun isPlaying(status: Status): Boolean = status == Status.Playing

    /** The track's duration: the anchor's once the engine knows it, else the tags'. */
    fun durationUs(state: CoreState): Long? =
        state.playback.anchor.durationUs ?: state.nowPlaying?.durationMs?.let { it * 1_000 }

    /** The chosen output device's name, or null when following the system default. */
    fun deviceName(devices: DevicesSnapshot): String? {
        val selected = devices.selected ?: return null
        return devices.devices.firstOrNull { it.id == selected || it.name == selected }?.name ?: selected
    }

    /** Whether [id] is the device in use (null is the system default). */
    fun isSelected(devices: DevicesSnapshot, id: String?): Boolean =
        if (id == null) devices.selected == null else devices.selected == id

    /** The play/pause button's command. */
    fun playPause(status: Status): Command = if (status == Status.Playing) Command.Pause else Command.Play
}

/** What the system media session shows (Media3 `SimpleBasePlayer` state), from the core's slices. */
enum class SessionPlaybackState { Idle, Buffering, Ready, Ended }

data class SessionItem(
    val uid: Long,
    val title: String?,
    val artist: String?,
    val album: String?,
    /** A `file://` or `https://` URI for the lock screen and notification. */
    val artworkUri: String?,
    val durationMs: Long?,
)

data class SessionModel(
    val playWhenReady: Boolean,
    val state: SessionPlaybackState,
    /** The current entry first, then Up Next (so Next is always offered while there is one). */
    val items: List<SessionItem>,
    val currentIndex: Int,
    val shuffle: Boolean,
    val repeat: RepeatMode,
    val anchor: PlaybackAnchor,
    val buffering: Boolean,
)

/** How the system asked to seek (Media3's seek commands). */
enum class SeekKind { InCurrentItem, ToItem, Next, Previous, DefaultPosition }

object SessionModels {
    fun from(state: CoreState): SessionModel {
        val now = state.nowPlaying
        val current = now?.let {
            SessionItem(
                uid = it.uid,
                title = it.title,
                artist = it.artist,
                album = it.album,
                artworkUri = when (val art = it.artwork) {
                    is ArtworkRef.File -> "file://" + art.path
                    is ArtworkRef.Url -> art.url
                    null -> null
                },
                durationMs = it.durationMs ?: state.playback.anchor.durationUs?.div(1_000),
            )
        }
        val upNext = state.queue.upNext.map { item ->
            SessionItem(item.uid, Labels.provisionalTitle(item.track), null, null, null, null)
        }
        val items = listOfNotNull(current) + upNext
        val status = state.playback.status
        val sessionState = when {
            current == null -> SessionPlaybackState.Idle
            status == Status.Ended -> SessionPlaybackState.Ended
            state.isBuffering -> SessionPlaybackState.Buffering
            status == Status.Stopped -> SessionPlaybackState.Idle
            else -> SessionPlaybackState.Ready
        }
        return SessionModel(
            playWhenReady = status == Status.Playing,
            state = sessionState,
            items = items,
            currentIndex = 0,
            shuffle = state.queue.shuffle,
            repeat = state.queue.repeat,
            anchor = state.playback.anchor,
            buffering = state.isBuffering,
        )
    }

    /** The core command for a seek the system asked for. */
    fun seekCommand(model: SessionModel, kind: SeekKind, index: Int, positionMs: Long): Command? = when (kind) {
        SeekKind.Next -> Command.Next
        SeekKind.Previous -> Command.Previous
        SeekKind.InCurrentItem -> Command.Seek(positionMs.coerceAtLeast(0))
        SeekKind.DefaultPosition -> if (index == model.currentIndex) Command.Seek(0) else model.items.getOrNull(index)?.let { Command.JumpTo(it.uid) }
        SeekKind.ToItem -> when {
            index == model.currentIndex -> Command.Seek(positionMs.coerceAtLeast(0))
            else -> model.items.getOrNull(index)?.let { Command.JumpTo(it.uid) }
        }
    }
}

/** Audio focus changes Android reports. */
enum class FocusChange { Gain, Loss, LossTransient, LossTransientCanDuck }

/**
 * What playback does when another app takes or returns audio focus: pause on loss, resume after a
 * transient loss if it was playing, and let the system duck on "can duck" (Android 8+ ducks
 * automatically).
 */
class AudioFocusPolicy {
    var resumeOnGain: Boolean = false
        private set

    fun onChange(change: FocusChange, playing: Boolean): Command? = when (change) {
        FocusChange.Gain -> if (resumeOnGain) {
            resumeOnGain = false
            Command.Play
        } else {
            null
        }
        FocusChange.Loss -> {
            resumeOnGain = false
            if (playing) Command.Pause else null
        }
        FocusChange.LossTransient -> {
            if (playing) {
                resumeOnGain = true
                Command.Pause
            } else {
                null
            }
        }
        FocusChange.LossTransientCanDuck -> null
    }

    /** The person paused or played themselves: a later focus gain must not resume on its own. */
    fun userActed() {
        resumeOnGain = false
    }
}
