package org.jellyfesh.logic.sources

import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.nav.Screen

/** Where a source stands, for its line in Your Sources and Settings › Sources › Installed. */
enum class SourceState {
    /** Turned off (by the person, or while community plugins are not allowed). */
    Off,

    /** Signed in: "Signed in as …". */
    SignedIn,

    /** It signs in and nobody has: "Not signed in". */
    NotSignedIn,

    /** On, and it needs no sign-in: nothing is said beyond its badge, as in the other apps. */
    Ready,
}

/** What choosing a source on the Sources page does ([YourSources.opens]). */
sealed interface SourceOpen {
    /** Its Discover page. */
    data class Page(val screen: Screen) : SourceOpen

    /** The sign-in sheet, with its first way to sign in. */
    data object SignIn : SourceOpen

    /** Settings › Sources, where it is turned on. */
    data object Settings : SourceOpen
}

/** Your Sources on the Sources tab (docs/ui.md, "Sources"). */
object YourSources {
    /**
     * The music sources among the installed plugins, for Your Sources and Settings › Sources ›
     * Installed. Extensions (AI Lyrics, Customize: `kind` Extension) are listed in Settings ›
     * Extensions instead (ADR-0020).
     */
    fun sources(plugins: List<PluginSummary>): List<PluginSummary> = plugins.filter { it.kind == PluginKind.Source }

    /** Whether a music source is on: only then does search ask the sources. */
    fun anyOn(plugins: List<PluginSummary>): Boolean = plugins.any { it.kind == PluginKind.Source && it.enabled }

    /** The state a source's line shows after its badge. */
    fun state(plugin: PluginSummary): SourceState = when {
        !plugin.enabled -> SourceState.Off
        plugin.account != null -> SourceState.SignedIn
        plugin.authMethods.isNotEmpty() -> SourceState.NotSignedIn
        else -> SourceState.Ready
    }

    /** Whether the source has a catalogue to browse (the `browse` capability) and is on. */
    fun canBrowse(plugin: PluginSummary): Boolean = plugin.enabled && BROWSE in plugin.capabilities

    /**
     * What choosing a source on the Sources page does, as on Jellyfesh Web: its Discover page
     * (docs/ui.md, "Discover"; Browse is a button there) while it is on and signed in or needs no
     * sign-in; the sign-in sheet while it is on and nobody has signed in; Settings › Sources,
     * where it is turned on, while it is off.
     */
    fun opens(plugin: PluginSummary): SourceOpen = when {
        !plugin.enabled -> SourceOpen.Settings
        plugin.account == null && plugin.authMethods.isNotEmpty() -> SourceOpen.SignIn
        else -> SourceOpen.Page(Screen.Discover(plugin.id))
    }

    /** Browse in the Discover page's header: the source's own folders, from its top level. */
    fun browse(plugin: PluginSummary): Screen? = if (canBrowse(plugin)) Screen.SourceBrowse(plugin.id, null, plugin.name) else null

    const val BROWSE = "browse"
}
