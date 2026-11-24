package org.jellyfesh.logic.player

import org.jellyfesh.logic.state.CoreState

/** What the picture-in-picture window says about itself (Android 13+ shows it in its menu). */
data class PipCaption(val title: String, val subtitle: String?)

/**
 * The mini player on Android (ADR-0026 §4; docs/ui.md, "The player bar and the mini player"):
 * picture-in-picture when the person leaves Jellyfesh while music plays, only with Settings ›
 * Appearance › Mini Player in Picture-in-Picture on (off by default: Android documents
 * picture-in-picture for video, calls and navigation). Never the overlay permission. The window
 * shows the cover, the title and the artist, a progress line and the lyrics bar's line; its
 * controls are the media session's, which Android draws itself.
 */
object MiniPlayerPip {
    /** The window's shape: wide, for the cover beside two lines of text. */
    const val ASPECT_WIDTH = 16
    const val ASPECT_HEIGHT = 9

    /** Android 12: auto-enter (`setAutoEnterEnabled`) and seamless resizing can be set. */
    const val AUTO_ENTER_SDK = 31

    /** Android 13: the window's title and subtitle. */
    const val CAPTION_SDK = 33

    /**
     * Whether leaving Jellyfesh now opens the mini player: the setting on ([settingOn]), the device
     * able to ([supported]: `FEATURE_PICTURE_IN_PICTURE`, which low-RAM devices lack), and a song
     * playing, here or on a remote device.
     */
    fun opensOnLeave(state: CoreState, settingOn: Boolean, supported: Boolean): Boolean =
        settingOn && supported && PlayerModel.track(state) != null && PlayerModel.isPlaying(PlayerModel.status(state))

    /**
     * Whether the full bar shows the mini player button, which opens the window at once: only with
     * the setting on and a device able to, as the web hides it where it has no mini player.
     */
    fun showsButton(state: CoreState, settingOn: Boolean, supported: Boolean): Boolean =
        settingOn && supported && PlayerModel.track(state) != null

    /** The title and artist the window carries, or null with nothing to play. */
    fun caption(state: CoreState): PipCaption? {
        val track = PlayerModel.track(state) ?: return null
        return PipCaption(track.title, track.artist ?: track.subtitle)
    }
}
