package org.jellyfesh.logic.nav

import org.jellyfesh.logic.core.Place
import org.jellyfesh.logic.theme.LookStyle

/** The two layouts of ADR-0023, chosen by the window's width (docs/ui.md, "Navigation"). */
object Layout {
    /** The widest window that keeps the phone layout (Jellyfesh Web's 760 px). */
    const val PHONE_MAX_DP = 760f

    /** Whether a window [widthDp] wide shows the sidebar panel (from 761 dp) rather than the tab bar. */
    fun usesSidebar(widthDp: Float): Boolean = widthDp > PHONE_MAX_DP
}

/** A place of the sidebar panel (761 dp and wider), in the order every app lists them. */
enum class SidebarPlace { Home, Albums, Artists, Songs, Playlists, Favourites, Downloads, Search, Sources, Settings }

/**
 * The sidebar panel (docs/ui.md, "Navigation"): the wordmark, the library's seven places, a
 * separator, Search and Sources; Open Files… and Settings at its foot. A customized look
 * (ADR-0020) lists the places it shows, in its order, as one group.
 */
object Sidebar {
    val LIBRARY = listOf(
        SidebarPlace.Home,
        SidebarPlace.Albums,
        SidebarPlace.Artists,
        SidebarPlace.Songs,
        SidebarPlace.Playlists,
        SidebarPlace.Favourites,
        SidebarPlace.Downloads,
    )
    val DISCOVER = listOf(SidebarPlace.Search, SidebarPlace.Sources)

    /** The groups above the panel's foot, each drawn after a separator but the first. */
    fun groups(style: LookStyle): List<List<SidebarPlace>> {
        if (!style.customized) return listOf(LIBRARY, DISCOVER)
        val places = style.places.map(::place)
        // Downloads is newer than the look's places: it follows the look's last library place.
        val last = places.indexOfLast { it in LIBRARY }
        return listOf(places.take(last + 1) + SidebarPlace.Downloads + places.drop(last + 1))
    }

    fun place(place: Place): SidebarPlace = when (place) {
        Place.Home -> SidebarPlace.Home
        Place.Albums -> SidebarPlace.Albums
        Place.Artists -> SidebarPlace.Artists
        Place.Songs -> SidebarPlace.Songs
        Place.Playlists -> SidebarPlace.Playlists
        Place.Favourites -> SidebarPlace.Favourites
        Place.Search -> SidebarPlace.Search
        Place.Sources -> SidebarPlace.Sources
    }

    /** The library section a place opens, or null for the places that are tabs of their own. */
    fun section(place: SidebarPlace): LibrarySection? = when (place) {
        SidebarPlace.Albums -> LibrarySection.Albums
        SidebarPlace.Artists -> LibrarySection.Artists
        SidebarPlace.Songs -> LibrarySection.Songs
        SidebarPlace.Playlists -> LibrarySection.Playlists
        SidebarPlace.Favourites -> LibrarySection.Favorites
        SidebarPlace.Downloads -> LibrarySection.Downloads
        SidebarPlace.Home, SidebarPlace.Search, SidebarPlace.Sources, SidebarPlace.Settings -> null
    }

    /** The place the panel marks for where the person is. */
    fun selected(state: NavState): SidebarPlace = when (state.tab) {
        Tab.Home -> SidebarPlace.Home
        Tab.Library -> when ((state.stack.first() as? Screen.Library)?.section ?: LibrarySection.Albums) {
            LibrarySection.Albums -> SidebarPlace.Albums
            LibrarySection.Artists -> SidebarPlace.Artists
            LibrarySection.Songs -> SidebarPlace.Songs
            LibrarySection.Playlists -> SidebarPlace.Playlists
            LibrarySection.Favorites -> SidebarPlace.Favourites
            LibrarySection.Downloads -> SidebarPlace.Downloads
        }
        Tab.Search -> SidebarPlace.Search
        Tab.Sources -> SidebarPlace.Sources
        Tab.Settings -> SidebarPlace.Settings
    }

    /** Choosing a place: its page, back at its top when it is the place already chosen. */
    fun select(state: NavState, place: SidebarPlace): NavState {
        section(place)?.let { return Navigator.openLibrary(state, it) }
        val tab = when (place) {
            SidebarPlace.Search -> Tab.Search
            SidebarPlace.Sources -> Tab.Sources
            SidebarPlace.Settings -> Tab.Settings
            else -> Tab.Home
        }
        return Navigator.select(state, tab)
    }
}

/**
 * The phone layout's tab bar (up to 760 dp): Home, Library, Search, Sources and Settings. A
 * customized look (ADR-0020) orders the first four by its places (any library place stands for
 * Library); Settings stays last, as on Jellyfesh Web.
 */
object PhoneTabs {
    fun tabs(style: LookStyle): List<Tab> {
        if (!style.customized) return Tab.entries
        return style.places.map(::tab).distinct() + Tab.Settings
    }

    fun tab(place: Place): Tab = when (place) {
        Place.Home -> Tab.Home
        Place.Albums, Place.Artists, Place.Songs, Place.Playlists, Place.Favourites -> Tab.Library
        Place.Search -> Tab.Search
        Place.Sources -> Tab.Sources
    }
}
