package org.jellyfesh.logic.theme

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Density
import org.jellyfesh.logic.core.HomeSection
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.LyricsHighlight
import org.jellyfesh.logic.core.LyricsWeight
import org.jellyfesh.logic.core.MiniPlayerStyle
import org.jellyfesh.logic.core.NowPlayingLayout
import org.jellyfesh.logic.core.Place
import org.jellyfesh.logic.core.Typeface
import org.jellyfesh.logic.nav.LibrarySection

class LookStyleTest {
    private val custom = LookSnapshot.Default.copy(
        cornerScale = 1.5f,
        glass = 0.4f,
        aurora = 0.2f,
        typeface = Typeface.Serif,
        textScale = 1.2f,
        density = Density.Compact,
        lyricsWeight = LyricsWeight.Heavy,
        lyricsGlow = 0.5f,
        lyricsHighlight = LyricsHighlight.Custom(0x11FF8A3D),
        sidebar = listOf(Place.Home, Place.Songs, Place.Albums, Place.Songs, Place.Search),
        home = listOf(HomeSection.MostPlayed, HomeSection.Summary, HomeSection.MostPlayed),
        nowPlaying = NowPlayingLayout.Lyrics,
        miniPlayer = MiniPlayerStyle.Floating,
        customized = true,
    )

    @Test
    fun anUncustomizedLookIsLuminousWhateverItHolds() {
        val style = LookStyle(custom.copy(customized = false))
        assertEquals(1f, style.cornerScale)
        assertEquals(1f, style.glass)
        assertEquals(0.75f, style.aurora(0.75f))
        assertEquals(Typeface.System, style.typeface)
        assertEquals(1f, style.textScale)
        assertEquals(1f, style.densityScale)
        assertEquals(700, style.lyricsWeight(700))
        assertEquals(SungInk.Token, style.sungInk)
        assertEquals(0f, style.glowRadius(40f))
        assertEquals(Place.entries, style.places)
        assertEquals(LookSnapshot.Default.home, style.home)
        assertEquals(NowPlayingLayout.Stage, style.nowPlaying)
        assertEquals(MiniPlayerStyle.Bar, style.miniPlayer)
    }

    @Test
    fun aCustomizedLookScalesAndOrders() {
        val style = LookStyle(custom)
        assertEquals(15f, style.radius(10f))
        assertEquals(0.2f, style.transparency(0.5f), 1e-6f)
        assertEquals(0f, style.transparency(0f))
        assertEquals(0.2f, style.aurora(0.75f))
        assertEquals(Typeface.Serif, style.typeface)
        assertEquals(1.2f, style.textScale)
        assertEquals(8f, style.spacing(10f), 1e-6f)
        assertEquals(800, style.lyricsWeight(700))
        assertEquals(SungInk.Custom(0xFF8A3D), style.sungInk)
        assertEquals(7f, style.glowRadius(40f), 1e-4f)
        assertEquals(listOf(Place.Home, Place.Songs, Place.Albums, Place.Search), style.places)
        assertEquals(listOf(HomeSection.MostPlayed, HomeSection.Summary), style.home)
        assertEquals(NowPlayingLayout.Lyrics, style.nowPlaying)
        assertEquals(MiniPlayerStyle.Floating, style.miniPlayer)
    }

    @Test
    fun valuesOutOfRangeAreClampedAndNaNIsTheDefault() {
        val style = LookStyle(custom.copy(cornerScale = 9f, glass = Float.NaN, textScale = 0.5f, aurora = Float.NaN))
        assertEquals(2f, style.cornerScale)
        assertEquals(1f, style.glass)
        assertEquals(0.85f, style.textScale)
        assertEquals(0.75f, style.aurora(0.75f))
    }

    @Test
    fun theLibrarySwitcherFollowsTheLooksPlaces() {
        val style = LookStyle(custom)
        // Downloads is newer than the look's places: always last.
        assertEquals(listOf(LibrarySection.Songs, LibrarySection.Albums, LibrarySection.Downloads), LookPlaces.librarySections(style))
        assertEquals(LibrarySection.Songs, LookPlaces.shown(LibrarySection.Playlists, LookPlaces.librarySections(style)))
        assertEquals(LibrarySection.Albums, LookPlaces.shown(LibrarySection.Albums, LookPlaces.librarySections(style)))
        assertEquals(LibrarySection.Downloads, LookPlaces.shown(LibrarySection.Downloads, LookPlaces.librarySections(style)))
        // Without any of the switcher's places, every section stays.
        assertEquals(LibrarySection.entries, LookPlaces.librarySections(LookStyle(custom.copy(sidebar = listOf(Place.Home, Place.Sources)))))
        assertEquals(LibrarySection.entries, LookPlaces.librarySections(LookStyle.Luminous))
        assertFalse(LookPlaces.summaryUnderTitle(style))
        assertTrue(LookPlaces.summaryUnderTitle(LookStyle.Luminous))
    }
}
