package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.NowPlaying
import org.jellyfesh.logic.core.Status
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.state.CoreState

class MiniPlayerPipTest {
    private val song = NowPlaying(7, TrackRef.Library(7), "/m/7.flac", "Morning Harbour", "Glass Harbour", "Slow Tide", 200_000, null, null)
    private val playing = CoreState(ready = true, nowPlaying = song).let { it.copy(playback = it.playback.copy(status = Status.Playing)) }

    @Test
    fun leavingOpensTheMiniPlayerOnlyWithTheSettingWhileMusicPlays() {
        assertTrue(MiniPlayerPip.opensOnLeave(playing, settingOn = true, supported = true))
        // Off by default: Android documents picture-in-picture for video, calls and navigation.
        assertFalse(MiniPlayerPip.opensOnLeave(playing, settingOn = false, supported = true))
        // Low-RAM devices have no picture-in-picture.
        assertFalse(MiniPlayerPip.opensOnLeave(playing, settingOn = true, supported = false))
        val paused = playing.copy(playback = playing.playback.copy(status = Status.Paused))
        assertFalse(MiniPlayerPip.opensOnLeave(paused, settingOn = true, supported = true))
        assertFalse(MiniPlayerPip.opensOnLeave(CoreState(ready = true), settingOn = true, supported = true))
    }

    @Test
    fun theWindowCarriesTheTitleAndArtist() {
        assertEquals(PipCaption("Morning Harbour", "Glass Harbour"), MiniPlayerPip.caption(playing))
        assertNull(MiniPlayerPip.caption(CoreState(ready = true)))
        assertEquals(16f / 9f, MiniPlayerPip.ASPECT_WIDTH.toFloat() / MiniPlayerPip.ASPECT_HEIGHT)
    }
}
