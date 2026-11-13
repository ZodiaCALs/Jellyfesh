package org.jellyfesh.android.playback

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import androidx.core.content.ContextCompat
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import org.jellyfesh.android.AppGraph
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.player.AudioFocusPolicy
import org.jellyfesh.logic.player.FocusChange

/**
 * Audio focus and "becoming noisy" for the core's playback (the core's own engine does not
 * negotiate with Android). Playback starting from anywhere (UI, notification, headset) requests
 * focus; another app taking it pauses, and a transient loss resumes afterwards
 * ([AudioFocusPolicy]). Unplugging headphones pauses.
 */
class AudioFocusController(private val context: Context, private val graph: AppGraph) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val policy = AudioFocusPolicy()
    private var holding = false
    private var playing = false
    private var job: Job? = null
    private var abandonJob: Job? = null

    private val request: AudioFocusRequest = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .build(),
        )
        .setWillPauseWhenDucked(false)
        .setOnAudioFocusChangeListener { change ->
            val focus = when (change) {
                AudioManager.AUDIOFOCUS_GAIN -> FocusChange.Gain
                AudioManager.AUDIOFOCUS_LOSS -> FocusChange.Loss
                AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> FocusChange.LossTransient
                AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> FocusChange.LossTransientCanDuck
                else -> return@setOnAudioFocusChangeListener
            }
            if (focus == FocusChange.Loss) holding = false
            policy.onChange(focus, playing)?.let(graph::send)
        }
        .build()

    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) graph.send(Command.Pause)
        }
    }
    private var noisyRegistered = false

    fun start() {
        job = graph.scope.launch {
            graph.store.state.map { it.playback.status }.distinctUntilChanged().collect(::onStatus)
        }
    }

    fun stop() {
        job?.cancel()
        abandonJob?.cancel()
        abandon()
        setNoisy(false)
    }

    private fun onStatus(status: Status) {
        playing = status == Status.Playing
        setNoisy(playing)
        if (playing) {
            abandonJob?.cancel()
            if (!holding) {
                val result = audio.requestAudioFocus(request)
                holding = result == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
                if (result == AudioManager.AUDIOFOCUS_REQUEST_FAILED) graph.send(Command.Pause)
            }
        } else if (!policy.resumeOnGain) {
            // Paused by the person or stopped: let other apps have the output soon, but not on
            // every quick pause and resume.
            abandonJob?.cancel()
            abandonJob = graph.scope.launch {
                delay(if (status == Status.Paused) 30_000 else 0)
                abandon()
            }
        }
    }

    private fun abandon() {
        if (holding) {
            audio.abandonAudioFocusRequest(request)
            holding = false
        }
    }

    private fun setNoisy(on: Boolean) {
        if (on == noisyRegistered) return
        if (on) {
            ContextCompat.registerReceiver(
                context,
                noisy,
                IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        } else {
            context.unregisterReceiver(noisy)
        }
        noisyRegistered = on
    }
}
