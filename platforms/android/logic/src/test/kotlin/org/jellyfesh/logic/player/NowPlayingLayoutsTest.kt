package org.jellyfesh.logic.player

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.MiniPlayerStyle
import org.jellyfesh.logic.core.NowPlayingLayout
import org.jellyfesh.logic.player.NowPlayingArrangement.Across
import org.jellyfesh.logic.player.NowPlayingArrangement.CoverAndLine
import org.jellyfesh.logic.player.NowPlayingArrangement.Hero
import org.jellyfesh.logic.player.NowPlayingArrangement.Pane
import org.jellyfesh.logic.player.NowPlayingArrangement.Side

class NowPlayingLayoutsTest {
    private fun of(layout: NowPlayingLayout, wide: Boolean, panel: NowPlayingPanel?) = NowPlayingLayouts.arrangement(layout, wide, panel)

    @Test
    fun theStageIsV006() {
        assertEquals(Hero, of(NowPlayingLayout.Stage, false, null))
        assertEquals(Pane(NowPlayingPanel.Queue), of(NowPlayingLayout.Stage, false, NowPlayingPanel.Queue))
        assertEquals(Side(NowPlayingPanel.Lyrics), of(NowPlayingLayout.Stage, true, null))
        assertEquals(Side(NowPlayingPanel.Info), of(NowPlayingLayout.Stage, true, NowPlayingPanel.Info))
    }

    @Test
    fun artworkShowsTheSungLineUnderTheCover() {
        assertEquals(CoverAndLine, of(NowPlayingLayout.Artwork, false, null))
        assertEquals(Pane(NowPlayingPanel.Lyrics), of(NowPlayingLayout.Artwork, false, NowPlayingPanel.Lyrics))
        assertEquals(CoverAndLine, of(NowPlayingLayout.Artwork, true, NowPlayingPanel.Lyrics))
        assertEquals(Side(NowPlayingPanel.Queue), of(NowPlayingLayout.Artwork, true, NowPlayingPanel.Queue))
    }

    @Test
    fun lyricsPutsThePaneAcross() {
        assertEquals(Across(NowPlayingPanel.Lyrics), of(NowPlayingLayout.Lyrics, false, null))
        assertEquals(Pane(NowPlayingPanel.Info), of(NowPlayingLayout.Lyrics, false, NowPlayingPanel.Info))
        assertEquals(Across(NowPlayingPanel.Queue), of(NowPlayingLayout.Lyrics, true, NowPlayingPanel.Queue))
        assertEquals(NowPlayingPanel.Lyrics, NowPlayingLayouts.chosen(Across(NowPlayingPanel.Lyrics)))
        assertNull(NowPlayingLayouts.chosen(CoverAndLine))
    }

    @Test
    fun theMiniPlayerStyles() {
        assertEquals(MiniPlayerKind.Bar, NowPlayingLayouts.miniPlayer(MiniPlayerStyle.Bar, wide = true))
        assertEquals(MiniPlayerKind.Mini, NowPlayingLayouts.miniPlayer(MiniPlayerStyle.Bar, wide = false))
        assertEquals(MiniPlayerKind.Mini, NowPlayingLayouts.miniPlayer(MiniPlayerStyle.Compact, wide = true))
        assertEquals(MiniPlayerKind.Floating, NowPlayingLayouts.miniPlayer(MiniPlayerStyle.Floating, wide = false))
    }
}
