package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals

/** The full player bar's room, by its own width, as Jellyfesh Web's container queries decide. */
class PlayerBarLayoutTest {
    @Test
    fun aNarrowerBarDropsTheVolumeLineThenLyricsAndQueueThenTheMuteButton() {
        assertEquals(PlayerBarTools(volumeLine = true, paneButtons = true, volume = true), PlayerBarLayout.tools(1_000f))
        assertEquals(PlayerBarTools(volumeLine = false, paneButtons = true, volume = true), PlayerBarLayout.tools(999f))
        assertEquals(PlayerBarTools(volumeLine = false, paneButtons = true, volume = true), PlayerBarLayout.tools(760f))
        assertEquals(PlayerBarTools(volumeLine = false, paneButtons = false, volume = true), PlayerBarLayout.tools(759f))
        assertEquals(PlayerBarTools(volumeLine = false, paneButtons = false, volume = true), PlayerBarLayout.tools(640f))
        // The full bar beside the sidebar in a 900 dp window is about 588 dp wide.
        assertEquals(PlayerBarTools(volumeLine = false, paneButtons = false, volume = false), PlayerBarLayout.tools(588f))
    }

    @Test
    fun theTransportTakesUpTo340WhileTheTitleKeeps112() {
        assertEquals(340f, PlayerBarLayout.middleWidth(700f))
        assertEquals(340f, PlayerBarLayout.middleWidth(452f))
        assertEquals(300f, PlayerBarLayout.middleWidth(412f))
        // Never under its own least, even when the title must give way.
        assertEquals(232f, PlayerBarLayout.middleWidth(300f))
    }
}
