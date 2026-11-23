package org.jellyfesh.logic.nav

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.Place
import org.jellyfesh.logic.theme.LookStyle

class PlacesTest {
    private fun custom(vararg places: Place) = LookStyle(LookSnapshot.Default.copy(customized = true, sidebar = places.toList()))

    @Test
    fun phonesKeepTheTabBarUpTo760DpAndWiderWindowsGetTheSidebar() {
        assertFalse(Layout.usesSidebar(360f))
        assertFalse(Layout.usesSidebar(760f))
        assertTrue(Layout.usesSidebar(761f))
        assertTrue(Layout.usesSidebar(1280f))
    }

    @Test
    fun theSidebarListsTheLibrarysSevenPlacesThenSearchAndSources() {
        assertEquals(
            listOf(
                listOf(
                    SidebarPlace.Home,
                    SidebarPlace.Albums,
                    SidebarPlace.Artists,
                    SidebarPlace.Songs,
                    SidebarPlace.Playlists,
                    SidebarPlace.Favourites,
                    SidebarPlace.Downloads,
                ),
                listOf(SidebarPlace.Search, SidebarPlace.Sources),
            ),
            Sidebar.groups(LookStyle.Luminous),
        )
    }

    @Test
    fun aCustomizedLookOrdersTheSidebarInOneGroupWithDownloadsAfterItsLibraryPlaces() {
        assertEquals(
            listOf(listOf(SidebarPlace.Search, SidebarPlace.Songs, SidebarPlace.Home, SidebarPlace.Downloads, SidebarPlace.Sources)),
            Sidebar.groups(custom(Place.Search, Place.Songs, Place.Home, Place.Sources)),
        )
    }

    @Test
    fun theSidebarMarksWhereThePersonIs() {
        assertEquals(SidebarPlace.Home, Sidebar.selected(NavState()))
        var nav = Sidebar.select(NavState(), SidebarPlace.Favourites)
        assertEquals(SidebarPlace.Favourites, Sidebar.selected(nav))
        // A page opened from a library place keeps the place marked.
        nav = Navigator.push(nav, Screen.Album(1, "A"))
        assertEquals(SidebarPlace.Favourites, Sidebar.selected(nav))
        assertEquals(SidebarPlace.Settings, Sidebar.selected(Sidebar.select(nav, SidebarPlace.Settings)))
        assertEquals(SidebarPlace.Search, Sidebar.selected(Sidebar.select(nav, SidebarPlace.Search)))
    }

    @Test
    fun choosingALibraryPlaceOpensItsPageAndTheOthersTheirTabs() {
        var nav = Sidebar.select(NavState(), SidebarPlace.Songs)
        assertEquals(listOf<Screen>(Screen.Library(LibrarySection.Songs)), nav.stack)
        nav = Navigator.push(nav, Screen.Artist(2, "B"))
        // The same place again goes back to its page.
        nav = Sidebar.select(nav, SidebarPlace.Songs)
        assertEquals(listOf<Screen>(Screen.Library(LibrarySection.Songs)), nav.stack)
        nav = Sidebar.select(nav, SidebarPlace.Downloads)
        assertEquals(Screen.Library(LibrarySection.Downloads), nav.screen)
        nav = Sidebar.select(nav, SidebarPlace.Sources)
        assertEquals(Tab.Sources, nav.tab)
        nav = Sidebar.select(nav, SidebarPlace.Home)
        assertEquals(Screen.Home, nav.screen)
    }

    @Test
    fun thePhoneTabsFollowTheLookWithSettingsLast() {
        assertEquals(listOf(Tab.Home, Tab.Library, Tab.Search, Tab.Sources, Tab.Settings), PhoneTabs.tabs(LookStyle.Luminous))
        assertEquals(
            listOf(Tab.Search, Tab.Library, Tab.Home, Tab.Settings),
            PhoneTabs.tabs(custom(Place.Search, Place.Songs, Place.Albums, Place.Home)),
        )
    }
}
