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
        nav = Navigator.push(nav, Screen.Discover("org.jellyfesh.subsonic"))
        nav = Navigator.openNowPlaying(nav)
        assertTrue(nav.canGoBack)
        nav = Navigator.back(nav)!!
        assertFalse(nav.nowPlayingOpen)
        assertEquals(Screen.Discover("org.jellyfesh.subsonic"), nav.screen)
        nav = Navigator.back(nav)!!
        assertEquals(Screen.Sources, nav.screen)
        nav = Navigator.back(nav)!!
        assertEquals(Tab.Home, nav.tab)
        assertFalse(nav.canGoBack)
        assertNull(Navigator.back(nav), "at Home's root back leaves the app")
    }

    @Test
    fun pushingTheSameScreenTwiceDoesNotStack() {
        val nav = Navigator.push(Navigator.push(NavState(), Screen.Album(1, "A")), Screen.Album(1, "A"))
        assertEquals(2, nav.stack.size)
    }

    @Test
    fun thePhoneLayoutHasFiveTabsWithSettingsLast() {
        assertEquals(listOf(Tab.Home, Tab.Library, Tab.Search, Tab.Sources, Tab.Settings), Tab.entries)
        assertEquals(Screen.Settings(SettingsSection.Appearance), NavState.root(Tab.Settings))
        val nav = Navigator.select(NavState(), Tab.Settings)
        assertEquals(Screen.Settings(), nav.screen)
    }

    @Test
    fun theLibraryHasSixSectionsAndSettingsSevenInTheSpecsOrder() {
        assertEquals(
            listOf(LibrarySection.Albums, LibrarySection.Artists, LibrarySection.Songs, LibrarySection.Playlists, LibrarySection.Favorites, LibrarySection.Downloads),
            LibrarySection.entries,
        )
        assertEquals(
            listOf(
                SettingsSection.Appearance,
                SettingsSection.Playback,
                SettingsSection.Library,
                SettingsSection.Downloads,
                SettingsSection.Sources,
                SettingsSection.Extensions,
                SettingsSection.About,
            ),
            SettingsSection.entries,
        )
    }

    @Test
    fun openingALibrarySectionLeavesWhatTheTabHadOpen() {
        var nav = Navigator.select(NavState(), Tab.Library)
        nav = Navigator.push(nav, Screen.Album(3, "Sample Album"))
        nav = Navigator.openLibrary(Navigator.openNowPlaying(nav), LibrarySection.Downloads)
        assertEquals(Tab.Library, nav.tab)
        assertEquals(listOf<Screen>(Screen.Library(LibrarySection.Downloads)), nav.stack)
        assertFalse(nav.nowPlayingOpen)
    }

    @Test
    fun anExtensionsPageOpensInSettingsFromAnywhere() {
        val nav = Navigator.openExtension(Navigator.openNowPlaying(NavState()), "org.jellyfesh.ai-lyrics")
        assertEquals(Tab.Settings, nav.tab)
        assertEquals(listOf(Screen.Settings(SettingsSection.Extensions), Screen.Extension("org.jellyfesh.ai-lyrics")), nav.stack)
        assertFalse(nav.nowPlayingOpen)
    }

    @Test
    fun settingsOpensAtASectionAndSwitchesSectionsInPlace() {
        var nav = Navigator.select(NavState(), Tab.Sources)
        // Sources Settings… opens the Settings page at Sources, not on top of the Sources page.
        nav = Navigator.openSettings(nav, SettingsSection.Sources)
        assertEquals(Tab.Settings, nav.tab)
        assertEquals(listOf<Screen>(Screen.Settings(SettingsSection.Sources)), nav.stack)
        assertEquals(listOf<Screen>(Screen.Sources), nav.stacks[Tab.Sources])
        nav = Navigator.showSettingsSection(nav, SettingsSection.About)
        assertEquals(listOf<Screen>(Screen.Settings(SettingsSection.About)), nav.stack)
        // From a page opened inside Settings, a section opens the Settings page again.
        nav = Navigator.push(nav, Screen.Extension("org.jellyfesh.customize"))
        nav = Navigator.showSettingsSection(nav, SettingsSection.Playback)
        assertEquals(listOf<Screen>(Screen.Settings(SettingsSection.Playback)), nav.stack)
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
