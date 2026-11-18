package org.jellyfesh.logic.player

import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.DevicesSnapshot
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.PlaybackAnchor
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.sources.Links
import org.jellyfesh.logic.state.CoreState

/**
 * The track the player views show (the mini-player, the player bar and Now Playing): the engine's,
 * or the remote session's while one runs (docs/ui.md, "Links and remote playback").
 */
data class PlayerTrack(
    /** Changes with the track: what the views remember per track keys on it. */
    val key: String,
    val title: String,
    val artist: String?,
    val album: String?,
    /** A cached file for library tracks; a URL (shown as the placeholder) for online ones. */
    val artwork: ArtworkRef?,
    /** Opens the track in its service ("Open in {Service}"); only `https` links. */
    val link: String?,
    /** The source the track comes from (a plugin id), for the service's name; null for local tracks. */
    val source: String?,
    /** The engine's entry; null for the remote session's track. */
    val local: NowPlaying?,
) {
    /** `Artist · Album`, either part alone, or null. */
    val subtitle: String? get() = Labels.artistAlbum(artist, album)
}

/** Small derived values the player views share. */
object PlayerModel {
    /** The repeat button cycles Off → All → One → Off. */
    fun nextRepeat(mode: RepeatMode): RepeatMode = when (mode) {
        RepeatMode.Off -> RepeatMode.All
        RepeatMode.All -> RepeatMode.One
        RepeatMode.One -> RepeatMode.Off
    }

    fun isPlaying(status: Status): Boolean = status == Status.Playing

    /**
     * What the player views show: while the Remote slice has a source, the service's track (or,
     * before the service names one, the service itself); else the engine's; null for nothing.
     */
    fun track(state: CoreState): PlayerTrack? {
        val remote = state.remote
        val source = remote.source
        if (source != null) {
            val item = remote.item
            return PlayerTrack(
                key = "remote:$source:${item?.id.orEmpty()}",
                title = item?.title ?: RemoteModel.serviceName(remote, state.sources),
                artist = item?.subtitle,
                album = item?.album,
                artwork = item?.artwork,
                link = Links.openable(item?.link),
                source = source,
                local = null,
            )
        }
        val now = state.nowPlaying ?: return null
        return PlayerTrack(
            key = "np:${now.uid}",
            title = now.title,
            artist = now.artist,
            album = now.album,
            artwork = now.artwork,
            link = Links.openable(now.link),
            source = (now.track as? TrackRef.Source)?.track?.plugin,
            local = now,
        )
    }

    /** Whether the player shows a remote session (its controls reach the service's player). */
    fun isRemote(state: CoreState): Boolean = state.remote.isActive

    /** Playing, paused or stopped: the remote session's while one runs, else the engine's. */
    fun status(state: CoreState): Status = if (state.remote.isActive) state.remote.status else state.playback.status

    /** The time base views draw progress and lyrics from (fed to the one ticker). */
    fun anchor(state: CoreState): PlaybackAnchor = if (state.remote.isActive) state.remote.anchor else state.playback.anchor

    fun shuffle(state: CoreState): Boolean = if (state.remote.isActive) state.remote.shuffle else state.queue.shuffle

    fun repeat(state: CoreState): RepeatMode = if (state.remote.isActive) state.remote.repeat else state.queue.repeat

    /**
     * The track's duration: the anchor's once it is known, else the tags' (the engine's track) or
     * the service's (the remote session's).
     */
    fun durationUs(state: CoreState): Long? = if (state.remote.isActive) {
        state.remote.anchor.durationUs ?: state.remote.item?.durationMs?.let { it * 1_000 }
    } else {
        state.playback.anchor.durationUs ?: state.nowPlaying?.durationMs?.let { it * 1_000 }
    }

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
    /** The media item's id, unique in the session's list: the queue entry's, or the remote session's song. */
    val id: String,
    /** The queue entry's uid, which choosing the item jumps to; null for a remote session's song. */
    val uid: Long?,
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
    /**
     * The session for the core's slices: the engine's song and Up Next, or, while a remote
     * session runs, the service's song (docs/ui.md: the system's media controls follow it, as on
     * the other platforms).
     */
    fun from(state: CoreState): SessionModel {
        if (state.remote.isActive) return remote(state)
        val now = state.nowPlaying
        val current = now?.let {
            SessionItem(
                id = entryId(it.uid),
                uid = it.uid,
                title = it.title,
                artist = it.artist,
                album = it.album,
                artworkUri = artworkUri(it.artwork),
                durationMs = it.durationMs ?: state.playback.anchor.durationUs?.div(1_000),
            )
        }
        val upNext = state.queue.upNext.map { item ->
            SessionItem(entryId(item.uid), item.uid, Labels.provisionalTitle(item.track), null, null, null, null)
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

    /**
     * The remote session's song as the notification and lock screen show it: what the player
     * views show ([PlayerModel.track]: the service's song, or the service before it names one),
     * with the session's anchor, play state, shuffle and repeat. The service keeps its own Up Next,
     * so the song stands alone; Next and Previous still reach the service, as the core routes them.
     */
    private fun remote(state: CoreState): SessionModel {
        val remote = state.remote
        val track = PlayerModel.track(state)
        val current = track?.let {
            SessionItem(
                id = it.key,
                uid = null,
                title = it.title,
                artist = it.artist,
                album = it.album,
                artworkUri = artworkUri(it.artwork),
                durationMs = PlayerModel.durationUs(state)?.div(1_000),
            )
        }
        val sessionState = when {
            current == null -> SessionPlaybackState.Idle
            remote.status == Status.Ended -> SessionPlaybackState.Ended
            remote.status == Status.Stopped -> SessionPlaybackState.Idle
            else -> SessionPlaybackState.Ready
        }
        return SessionModel(
            playWhenReady = remote.status == Status.Playing,
            state = sessionState,
            items = listOfNotNull(current),
            currentIndex = 0,
            shuffle = remote.shuffle,
            repeat = remote.repeat,
            anchor = remote.anchor,
            buffering = false,
        )
    }

    private fun entryId(uid: Long): String = "entry:$uid"

    private fun artworkUri(artwork: ArtworkRef?): String? = when (artwork) {
        is ArtworkRef.File -> "file://" + artwork.path
        is ArtworkRef.Url -> artwork.url
        null -> null
    }

    /** The core command for a seek the system asked for. */
    fun seekCommand(model: SessionModel, kind: SeekKind, index: Int, positionMs: Long): Command? = when (kind) {
        SeekKind.Next -> Command.Next
        SeekKind.Previous -> Command.Previous
        SeekKind.InCurrentItem -> Command.Seek(positionMs.coerceAtLeast(0))
        SeekKind.DefaultPosition -> if (index == model.currentIndex) Command.Seek(0) else jumpTo(model, index)
        SeekKind.ToItem -> when {
            index == model.currentIndex -> Command.Seek(positionMs.coerceAtLeast(0))
            else -> jumpTo(model, index)
        }
    }

    /** Jumps to the queue entry at [index]; nothing for a remote session's song or an index past the list. */
    private fun jumpTo(model: SessionModel, index: Int): Command? = model.items.getOrNull(index)?.uid?.let { Command.JumpTo(it) }
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
