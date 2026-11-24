package org.jellyfesh.android.system

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.pm.PackageManager
import android.os.Build
import android.util.Rational
import org.jellyfesh.logic.player.MiniPlayerPip
import org.jellyfesh.logic.player.PipCaption
import org.jellyfesh.logic.state.CoreState

/**
 * The mini player as Android's picture-in-picture ([MiniPlayerPip]; ADR-0026 §4): the activity's
 * parameters follow the core's state. From Android 12 the window opens by itself when the person
 * leaves (`setAutoEnterEnabled`) while the setting is on and music plays, with seamless resizing
 * off (it shows no video); from Android 13 it carries the title and artist. Before Android 12 the
 * activity enters it from `onUserLeaveHint`. Its controls are the media session's, which Android
 * draws itself; the overlay permission is never used.
 */
class PictureInPicture(private val activity: Activity) {
    /** Low-RAM devices have no picture-in-picture. */
    val supported: Boolean = activity.packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)

    private var opensOnLeave = false
    private var caption: PipCaption? = null
    private var applied: Pair<Boolean, PipCaption?>? = null

    /** The core's state changed: the window's parameters follow it, when they change. */
    fun update(state: CoreState) {
        opensOnLeave = MiniPlayerPip.opensOnLeave(state, state.settings.miniPlayer.androidPip, supported)
        caption = MiniPlayerPip.caption(state)
        if (!supported) return
        val wanted = opensOnLeave to caption
        if (wanted == applied) return
        applied = wanted
        runCatching { activity.setPictureInPictureParams(params()) }
    }

    /** The full bar's mini player button: the window opens at once. */
    fun open() {
        if (!supported) return
        runCatching { activity.enterPictureInPictureMode(params()) }
    }

    /** The person is leaving (Home, Recents): before Android 12 the activity enters the window itself. */
    fun onUserLeaveHint() {
        if (Build.VERSION.SDK_INT >= MiniPlayerPip.AUTO_ENTER_SDK || !opensOnLeave) return
        runCatching { activity.enterPictureInPictureMode(params()) }
    }

    private fun params(): PictureInPictureParams {
        val builder = PictureInPictureParams.Builder().setAspectRatio(Rational(MiniPlayerPip.ASPECT_WIDTH, MiniPlayerPip.ASPECT_HEIGHT))
        if (Build.VERSION.SDK_INT >= MiniPlayerPip.AUTO_ENTER_SDK) {
            builder.setAutoEnterEnabled(opensOnLeave).setSeamlessResizeEnabled(false)
        }
        if (Build.VERSION.SDK_INT >= MiniPlayerPip.CAPTION_SDK) {
            caption?.let { builder.setTitle(it.title).setSubtitle(it.subtitle) }
        }
        return builder.build()
    }
}
