package org.jellyfesh.logic.nav

import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.ItemKind

/**
 * The places of the phone layout's tab bar, up to 760 dp wide (docs/ui.md, "Navigation";
 * ADR-0023): Home, Library, Search, Sources and Settings. From 761 dp the sidebar panel shows the
 * same places ([Sidebar]), with the library's sections as places of their own.
 */
enum class Tab { Home, Library, Search, Sources, Settings }

/** The library's sections: the Library tab's switcher on phones, the sidebar's places on wider windows. */
enum class LibrarySection { Albums, Artists, Songs, Playlists, Favorites, Downloads }

/**
 * Settings' sections, in the order every app lists them (docs/ui.md, "Settings"): one page in the
 * main window, the section chosen at its top. The lyrics settings live in Appearance, with no
 * section of their own; Extensions holds Jellyfesh Pro and the extensions.
 */
enum class SettingsSection { Appearance, Playback, Library, Downloads, Sources, Extensions, About }

/** A screen in a tab's back stack. */
sealed interface Screen {
    data object Home : Screen
    data class Library(val section: LibrarySection = LibrarySection.Albums) : Screen
    data object Search : Screen
    data object Sources : Screen
    data class Album(val id: Long, val title: String) : Screen
    data class Artist(val id: Long, val name: String) : Screen
    data class Playlist(val id: Long, val name: String) : Screen

    /** A source's Discover page (v0.0.7): its shelves, with Browse in the header. */
    data class Discover(val pluginId: String) : Screen

    /**
     * A source's catalogue: its root ([node] null) or an album, artist, playlist or folder, with
     * what its row showed: its Open in {Service} [link], its [kind] (an album or playlist gets the
     * full page), its [subtitle] (an album's artist) and its [artwork].
     */
    data class SourceBrowse(
        val pluginId: String,
        val node: String?,
        val title: String,
        val link: String? = null,
        val kind: ItemKind? = null,
        val subtitle: String? = null,
        val artwork: ArtworkRef? = null,
    ) : Screen
    /** Settings, one page with its sections (the Settings tab's root, or the sidebar's foot). */
    data class Settings(val section: SettingsSection = SettingsSection.Appearance) : Screen

    /** An extension's page in Settings › Extensions (AI Lyrics, Customize). */
    data class Extension(val pluginId: String) : Screen
}

/**
 * Where the person is: one back stack per tab (switching tabs keeps each tab's place), and Now
 * Playing as a layer over everything. Immutable; every move returns a new state.
 */
data class NavState(
    val tab: Tab = Tab.Home,
    val stacks: Map<Tab, List<Screen>> = Tab.entries.associateWith { listOf(root(it)) },
    val nowPlayingOpen: Boolean = false,
) {
    val stack: List<Screen> get() = stacks[tab].orEmpty().ifEmpty { listOf(root(tab)) }
    val screen: Screen get() = stack.last()
    val canGoBack: Boolean get() = nowPlayingOpen || stack.size > 1 || tab != Tab.Home

    companion object {
        fun root(tab: Tab): Screen = when (tab) {
            Tab.Home -> Screen.Home
            Tab.Library -> Screen.Library()
            Tab.Search -> Screen.Search
            Tab.Sources -> Screen.Sources
            Tab.Settings -> Screen.Settings()
        }
    }
}

object Navigator {
    /** Opens [screen] on top of the current tab. */
    fun push(state: NavState, screen: Screen): NavState {
        if (state.screen == screen) return state.copy(nowPlayingOpen = false)
        return state.copy(stacks = state.stacks + (state.tab to state.stack + screen), nowPlayingOpen = false)
    }

    /** Replaces the top screen (a library section change, for example). */
    fun replaceTop(state: NavState, screen: Screen): NavState =
        state.copy(stacks = state.stacks + (state.tab to state.stack.dropLast(1) + screen))

    /** A tab tap: switches tabs, or returns to the tab's root when it is already selected. */
    fun select(state: NavState, tab: Tab): NavState = when {
        state.tab != tab -> state.copy(tab = tab, nowPlayingOpen = false)
        state.stack.size > 1 -> state.copy(stacks = state.stacks + (tab to listOf(state.stack.first())), nowPlayingOpen = false)
        else -> state.copy(nowPlayingOpen = false)
    }

    /**
     * Back: closes Now Playing, then pops the tab's stack, then returns to Home. Null when there
     * is nowhere to go back to (the system then leaves the app).
     */
    fun back(state: NavState): NavState? = when {
        state.nowPlayingOpen -> state.copy(nowPlayingOpen = false)
        state.stack.size > 1 -> state.copy(stacks = state.stacks + (state.tab to state.stack.dropLast(1)))
        state.tab != Tab.Home -> state.copy(tab = Tab.Home)
        else -> null
    }

    /**
     * A library section from the switcher or the sidebar: the Library tab at that section's page,
     * whatever was open in the tab before.
     */
    fun openLibrary(state: NavState, section: LibrarySection): NavState =
        state.copy(tab = Tab.Library, stacks = state.stacks + (Tab.Library to listOf(Screen.Library(section))), nowPlayingOpen = false)

    /** Settings at [section] (Sources Settings…, Open AI Lyrics Settings): the Settings tab's page. */
    fun openSettings(state: NavState, section: SettingsSection): NavState =
        state.copy(tab = Tab.Settings, stacks = state.stacks + (Tab.Settings to listOf(Screen.Settings(section))), nowPlayingOpen = false)

    /** An extension's page (Open AI Lyrics Settings): Settings › Extensions, with the page on top. */
    fun openExtension(state: NavState, pluginId: String): NavState =
        push(openSettings(state, SettingsSection.Extensions), Screen.Extension(pluginId))

    /** Another section at the top of the Settings page: the page stays, its section changes. */
    fun showSettingsSection(state: NavState, section: SettingsSection): NavState =
        if (state.screen is Screen.Settings) replaceTop(state, Screen.Settings(section)) else openSettings(state, section)

    fun openNowPlaying(state: NavState): NavState = state.copy(nowPlayingOpen = true)

    fun closeNowPlaying(state: NavState): NavState = state.copy(nowPlayingOpen = false)
}
