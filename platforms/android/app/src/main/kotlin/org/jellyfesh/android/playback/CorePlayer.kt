package org.jellyfesh.android.playback

import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.SettableFuture
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import org.jellyfesh.android.AppGraph
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.player.SeekKind
import org.jellyfesh.logic.player.SessionModel
import org.jellyfesh.logic.player.SessionModels
import org.jellyfesh.logic.player.SessionPlaybackState
import org.jellyfesh.logic.time.positionAtUs

/**
 * The Media3 face of the core (architecture §11: a `SimpleBasePlayer` facade). It plays nothing
 * itself: the system's requests (notification buttons, lock screen, headset and Bluetooth keys)
 * become core commands, and its state is the core's slices, mapped by [SessionModels]: the
 * engine's song and Up Next, or a remote session's song while one runs (the core routes the
 * commands to the service then). Next and Previous work with a remote session's single song too:
 * Media3 hands an otherwise ignored skip to [handleSeek] with its command. Runs on the main
 * looper, as Media3 requires.
 */
@OptIn(UnstableApi::class)
class CorePlayer(private val graph: AppGraph, looper: Looper) : SimpleBasePlayer(looper) {
    private var model: SessionModel = SessionModels.from(graph.store.state.value)
    private val handler = Handler(looper)

    /** Requests waiting for the core's next slice, so the system UI does not flicker back. */
    private val waiting = mutableListOf<SettableFuture<Any?>>()

    /** Source pictures as the core's cached files (null: none to be had), and those being asked for. */
    private val covers = LinkedHashMap<String, String?>()
    private val coversAsked = HashSet<String>()

    private val job: Job = graph.scope.launch {
        graph.store.state.collect { state ->
            model = SessionModels.from(state)
            invalidateState()
            waiting.forEach { it.set(null) }
            waiting.clear()
        }
    }

    /**
     * The `file://` URI of a source's picture once the core has cached it; until then none, while
     * the artwork loader waits for the core's word and the state is drawn again when it comes.
     * Media3 is never given the source's URL, so the app fetches nothing itself.
     */
    private fun coverUri(url: String): String? {
        if (covers.containsKey(url)) return covers[url]?.let { "file://$it" }
        if (coversAsked.add(url)) {
            graph.scope.launch {
                val path = graph.images.sourceFile(url, COVER_PX)
                coversAsked.remove(url)
                covers[url] = path
                while (covers.size > 16) covers.remove(covers.keys.first())
                invalidateState()
            }
        }
        return null
    }

    override fun getState(): State {
        val items = model.items.map { item ->
            val artwork = item.artworkUri ?: item.artworkUrl?.let(::coverUri)
            val metadata = MediaMetadata.Builder()
                .setTitle(item.title)
                .setArtist(item.artist)
                .setAlbumTitle(item.album)
                .setArtworkUri(artwork?.let(Uri::parse))
                .setIsPlayable(true)
                .setIsBrowsable(false)
                .build()
            MediaItemData.Builder(item.id)
                .setMediaItem(MediaItem.Builder().setMediaId(item.id).setMediaMetadata(metadata).build())
                .setMediaMetadata(metadata)
                .setDurationUs(item.durationMs?.times(1_000) ?: C.TIME_UNSET)
                .setIsSeekable(true)
                .build()
        }
        val anchor = model.anchor
        val playbackState = when {
            items.isEmpty() -> Player.STATE_IDLE
            else -> when (model.state) {
                SessionPlaybackState.Idle -> Player.STATE_IDLE
                SessionPlaybackState.Buffering -> Player.STATE_BUFFERING
                SessionPlaybackState.Ready -> Player.STATE_READY
                SessionPlaybackState.Ended -> Player.STATE_ENDED
            }
        }
        val builder = State.Builder()
            .setAvailableCommands(COMMANDS)
            .setPlayWhenReady(model.playWhenReady, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setPlaybackState(playbackState)
            // State.Builder.build() rejects loading while idle or ended.
            .setIsLoading(model.buffering && (playbackState == Player.STATE_BUFFERING || playbackState == Player.STATE_READY))
            .setShuffleModeEnabled(model.shuffle)
            .setRepeatMode(
                when (model.repeat) {
                    RepeatMode.Off -> Player.REPEAT_MODE_OFF
                    RepeatMode.All -> Player.REPEAT_MODE_ALL
                    RepeatMode.One -> Player.REPEAT_MODE_ONE
                },
            )
            .setPlaylist(items)
        if (items.isNotEmpty()) {
            builder.setCurrentMediaItemIndex(model.currentIndex)
            // The system asks for the position when it draws; this extrapolates the core's anchor
            // then and costs nothing in between.
            builder.setContentPositionMs { anchor.positionAtUs(graph.nowNs()).coerceAtLeast(0) / 1_000 }
        }
        return builder.build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        graph.send(if (playWhenReady) Command.Play else Command.Pause)
        return untilNextSlice()
    }

    override fun handleStop(): ListenableFuture<*> {
        // Media3's stop keeps the playlist; pausing keeps the core's queue and position too.
        graph.send(Command.Pause)
        return untilNextSlice()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        val kind = when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> SeekKind.Next
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> SeekKind.Previous
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM, Player.COMMAND_SEEK_BACK, Player.COMMAND_SEEK_FORWARD -> SeekKind.InCurrentItem
            Player.COMMAND_SEEK_TO_DEFAULT_POSITION -> SeekKind.DefaultPosition
            else -> SeekKind.ToItem
        }
        val position = if (positionMs == C.TIME_UNSET) 0 else positionMs
        SessionModels.seekCommand(model, kind, mediaItemIndex, position)?.let(graph::send)
        return untilNextSlice()
    }

    override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> {
        graph.send(Command.SetShuffle(shuffleModeEnabled))
        return untilNextSlice()
    }

    override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> {
        val mode = when (repeatMode) {
            Player.REPEAT_MODE_ONE -> RepeatMode.One
            Player.REPEAT_MODE_ALL -> RepeatMode.All
            else -> RepeatMode.Off
        }
        graph.send(Command.SetRepeat(mode))
        return untilNextSlice()
    }

    override fun handleRelease(): ListenableFuture<*> {
        job.cancel()
        waiting.forEach { it.set(null) }
        waiting.clear()
        return Futures.immediateVoidFuture()
    }

    /** Completes when the core publishes its next slice, or after a second without one. */
    private fun untilNextSlice(): ListenableFuture<*> {
        val future = SettableFuture.create<Any?>()
        waiting += future
        handler.postDelayed({
            if (waiting.remove(future)) future.set(null)
        }, 1_000)
        return future
    }

    private companion object {
        /** The size the notification and lock screen show a source's picture at. */
        const val COVER_PX = 512

        val COMMANDS: Player.Commands = Player.Commands.Builder()
            .addAll(
                Player.COMMAND_PLAY_PAUSE,
                Player.COMMAND_STOP,
                Player.COMMAND_SEEK_TO_DEFAULT_POSITION,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_NEXT,
                Player.COMMAND_SEEK_TO_MEDIA_ITEM,
                Player.COMMAND_SET_SHUFFLE_MODE,
                Player.COMMAND_SET_REPEAT_MODE,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_TIMELINE,
                Player.COMMAND_GET_METADATA,
                Player.COMMAND_RELEASE,
            )
            .build()
    }
}
