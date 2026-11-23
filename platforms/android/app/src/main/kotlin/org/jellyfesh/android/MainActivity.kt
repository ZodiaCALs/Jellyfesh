package org.jellyfesh.android

import android.content.ComponentName
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import org.jellyfesh.android.playback.PlaybackService
import org.jellyfesh.android.system.PictureInPicture
import org.jellyfesh.android.ui.AppRoot

/**
 * The one activity. It hosts the Compose UI and connects a Media3 controller while visible, which
 * binds [PlaybackService] so the session (notification, lock screen, buttons) exists whenever
 * playback starts from the UI.
 */
class MainActivity : ComponentActivity() {
    private var controller: ListenableFuture<MediaController>? = null

    /** The mini player as picture-in-picture (ADR-0026 §4), following the core's state. */
    private var pictureInPicture: PictureInPicture? = null
    private var pictureInPictureUpdates: Job? = null

    private val app: JellyfeshApp get() = application as JellyfeshApp

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        handle(intent)
        // Resume on Launch, when the process started for this window (ResumeOnLaunch).
        app.graph.windowCreated(android.os.SystemClock.uptimeMillis())
        val pip = PictureInPicture(this)
        pictureInPicture = pip
        pictureInPictureUpdates = app.graph.scope.launch { app.graph.store.state.collect { pip.update(it) } }
        app.graph.pictureInPicture.value = isInPictureInPictureMode
        setContent { AppRoot(app.graph, app.authRedirects) }
    }

    override fun onDestroy() {
        pictureInPictureUpdates?.cancel()
        pictureInPictureUpdates = null
        super.onDestroy()
    }

    /** Before Android 12 the mini player opens here when the person leaves while music plays. */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        pictureInPicture?.onUserLeaveHint()
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        app.graph.pictureInPicture.value = isInPictureInPictureMode
    }

    override fun onStart() {
        super.onStart()
        controller = MediaController.Builder(this, SessionToken(this, ComponentName(this, PlaybackService::class.java))).buildAsync()
    }

    override fun onResume() {
        super.onResume()
        // A purchase may have completed while Jellyfesh was away (Play Billing's guidance).
        app.graph.purchases?.refresh()
    }

    override fun onStop() {
        controller?.let { MediaController.releaseFuture(it) }
        controller = null
        super.onStop()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    /** OAuth redirects come back as `jellyfesh://auth?...`. */
    private fun handle(intent: Intent?) {
        val uri = intent?.data ?: return
        if (uri.scheme == "jellyfesh" && uri.host == "auth") app.authRedirects.tryEmit(uri.toString())
    }
}
