package org.jellyfesh.android.playback

import android.app.PendingIntent
import android.content.Intent
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import org.jellyfesh.android.JellyfeshApp
import org.jellyfesh.android.MainActivity

/**
 * The Media3 session service: the media notification, the lock screen controls and the headset
 * and Bluetooth buttons, all backed by [CorePlayer]. Media3 makes it a foreground service while
 * the core plays, so playback keeps going with the screen off (and on Android 17, which stops
 * background audio without one).
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var focus: AudioFocusController? = null

    override fun onCreate() {
        super.onCreate()
        val graph = (application as JellyfeshApp).graph
        val player = CorePlayer(graph, Looper.getMainLooper())
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, player).setSessionActivity(open).build()
        focus = AudioFocusController(this, graph).also { it.start() }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        focus?.stop()
        focus = null
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}
