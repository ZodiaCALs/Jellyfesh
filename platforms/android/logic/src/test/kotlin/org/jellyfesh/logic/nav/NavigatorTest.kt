package org.jellyfesh.logic.nav

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class NavigatorTest {
    @Test
    fun eachTabKeepsItsPlace() {
        var nav = NavState()
        nav = Navigator.select(nav, Tab.Library)
        nav = Navigator.push(nav, Screen.Album(3, "Sample Album"))
        nav = Navigator.select(nav, Tab.Search)
        assertEquals(Screen.Search, nav.screen)
        nav = Navigator.select(nav, Tab.Library)
        assertEquals(Screen.Album(3, "Sample Album"), nav.screen)
    }

    @Test
    fun tappingTheCurrentTabReturnsToItsRoot() {
        var nav = Navigator.push(NavState(), Screen.Playlist(1, "Mix"))
        nav = Navigator.push(nav, Screen.Album(2, "B"))
        nav = Navigator.select(nav, Tab.Home)
        assertEquals(Screen.Home, nav.screen)
        assertEquals(1, nav.stack.size)
    }

    @Test
    fun backClosesNowPlayingThenPopsThenGoesHome() {
        var nav = Navigator.select(NavState(), Tab.Sources)
        nav = Navigator.push(nav, Screen.Source("org.jellyfesh.subsonic"))
        nav = Navigator.openNowPlaying(nav)
        assertTrue(nav.canGoBack)
        nav = Navigator.back(nav)!!
        assertFalse(nav.nowPlayingOpen)
        assertEquals(Screen.Source("org.jellyfesh.subsonic"), nav.screen)
        nav = Navigator.back(nav)!!
        assertEquals(Screen.Sources, nav.screen)
        nav = Navigator.back(nav)!!
        assertEquals(Tab.Home, nav.tab)
        assertFalse(nav.canGoBack)
        assertNull(Navigator.back(nav), "at Home's root back leaves the app")
    }

    @Test
    fun pushingTheSameScreenTwiceDoesNotStack() {
        val nav = Navigator.push(Navigator.push(NavState(), Screen.Settings()), Screen.Settings())
        assertEquals(2, nav.stack.size)
    }

    @Test
    fun aSectionChangeReplacesTheTop() {
        var nav = Navigator.select(NavState(), Tab.Library)
        nav = Navigator.replaceTop(nav, Screen.Library(LibrarySection.Songs))
        assertEquals(listOf<Screen>(Screen.Library(LibrarySection.Songs)), nav.stack)
    }

    @Test
    fun openingAScreenClosesNowPlaying() {
        val nav = Navigator.push(Navigator.openNowPlaying(NavState()), Screen.Artist(5, "Placeholder Artist"))
        assertFalse(nav.nowPlayingOpen)
    }
}
