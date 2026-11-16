package org.jellyfesh.logic.nav

/** The four tabs of the bottom bar. */
enum class Tab { Home, Library, Search, Sources }

/** The library's sections (Misty's segmented control on the Library tab). */
enum class LibrarySection { Albums, Artists, Songs, Playlists, Favorites }

/**
 * Settings' sections, in the order every app lists them (docs/ui.md, "Settings"). Android keeps
 * About in Settings; the lyrics settings live in Appearance, with no section of their own.
 */
enum class SettingsSection { Appearance, Playback, Library, Sources, About }

/** A screen in a tab's back stack. */
sealed interface Screen {
    data object Home : Screen
    data class Library(val section: LibrarySection = LibrarySection.Albums) : Screen
    data object Search : Screen
    data object Sources : Screen
    data class Album(val id: Long, val title: String) : Screen
    data class Artist(val id: Long, val name: String) : Screen
    data class Playlist(val id: Long, val name: String) : Screen
    data class Source(val pluginId: String) : Screen
    /** A source's catalogue: its root ([node] null) or an album, artist, playlist or folder with its Open in {Service} [link]. */
    data class SourceBrowse(val pluginId: String, val node: String?, val title: String, val link: String? = null) : Screen
    data class Settings(val section: SettingsSection? = null) : Screen
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

    fun openNowPlaying(state: NavState): NavState = state.copy(nowPlayingOpen = true)

    fun closeNowPlaying(state: NavState): NavState = state.copy(nowPlayingOpen = false)
}
