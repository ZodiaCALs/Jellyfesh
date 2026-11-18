package org.jellyfesh.logic.sources

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

/** Your Sources on the Sources tab (docs/ui.md, "Sources"). */
object YourSources {
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
     * What choosing a source opens: its catalogue when it is on and can browse (choosing one
     * browses it), otherwise its page in Settings › Sources (Enabled, sign-in, its settings).
     */
    fun opens(plugin: PluginSummary): Screen =
        if (canBrowse(plugin)) Screen.SourceBrowse(plugin.id, null, plugin.name) else Screen.Source(plugin.id)

    const val BROWSE = "browse"
}
