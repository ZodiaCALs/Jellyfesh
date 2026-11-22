package org.jellyfesh.logic.player

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.state.CoreState

/**
 * Resume on Launch on Android (docs/ffi.md, `resume_on_start`). The process can start without
 * its window: for a media button, or when the system restarts the media service. The app starts
 * the core with `resumeOnStart` false, so no audio begins before its media session exists, and
 * sends Play itself when the process started for its window: the first window created soon after
 * the process began.
 */
object ResumeOnLaunch {
    /** A window created within this long of the process starting is the reason it started. */
    const val WINDOW_MS = 10_000L

    fun startedWithWindow(processStartUptimeMs: Long, windowUptimeMs: Long): Boolean =
        processStartUptimeMs > 0 && windowUptimeMs - processStartUptimeMs in 0..WINDOW_MS

    /**
     * Play, once: the person's Resume Playback is on, the process started for its window, and the
     * restored queue has a song that is not playing yet. Null when there is nothing to send.
     */
    fun command(state: CoreState, startedWithWindow: Boolean): Command? {
        if (!startedWithWindow || !state.ready || !state.settings.resumeOnLaunch) return null
        if (state.queue.current == null) return null
        return when (state.playback.status) {
            org.jellyfesh.logic.core.Status.Stopped, org.jellyfesh.logic.core.Status.Paused -> Command.Play
            else -> null
        }
    }
}
