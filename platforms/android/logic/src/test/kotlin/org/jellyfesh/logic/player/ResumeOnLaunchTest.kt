package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.state.CoreState

class ResumeOnLaunchTest {
    private val restored = CoreState(
        ready = true,
        settings = Settings(resumeOnLaunch = true),
        queue = QueueSnapshot.Empty.copy(version = 1, len = 1, position = 0, current = QueueItem(7, TrackRef.Library(1))),
    )

    @Test
    fun aWindowSoonAfterTheProcessStartedIsAStartWithTheWindow() {
        assertTrue(ResumeOnLaunch.startedWithWindow(processStartUptimeMs = 1_000, windowUptimeMs = 2_500))
        // The process started for a media button; the window came much later.
        assertFalse(ResumeOnLaunch.startedWithWindow(processStartUptimeMs = 1_000, windowUptimeMs = 60_000))
        assertFalse(ResumeOnLaunch.startedWithWindow(processStartUptimeMs = 0, windowUptimeMs = 500))
    }

    @Test
    fun playIsSentOnlyForAStartWithTheWindowAndASongToResume() {
        assertEquals(Command.Play, ResumeOnLaunch.command(restored, startedWithWindow = true))
        assertNull(ResumeOnLaunch.command(restored, startedWithWindow = false))
        assertNull(ResumeOnLaunch.command(restored.copy(settings = Settings(resumeOnLaunch = false)), startedWithWindow = true))
        assertNull(ResumeOnLaunch.command(restored.copy(queue = QueueSnapshot.Empty), startedWithWindow = true))
        assertNull(ResumeOnLaunch.command(restored.copy(ready = false), startedWithWindow = true))
        val playing = restored.copy(playback = restored.playback.copy(status = Status.Playing))
        assertNull(ResumeOnLaunch.command(playing, startedWithWindow = true))
    }
}
