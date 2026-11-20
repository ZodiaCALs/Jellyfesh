package org.jellyfesh.logic.settings

import org.jellyfesh.logic.core.AiLyricsSnapshot
import org.jellyfesh.logic.core.AudioConsent
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.PluginKind
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.ProSnapshot

/**
 * Settings › Extensions (ADR-0020 §5): Jellyfesh Pro, then every extension, plugins that make
 * lyrics or change the look and offer no music (`PluginSummary.kind` Extension). AI Lyrics is the
 * one the `AiLyrics` slice names, Customize the active appearance plugin.
 */
object Extensions {
    /** The capability of AI Lyrics (`make_lyrics`) and of the look (`appearance`), as the core lists them. */
    const val MAKE_LYRICS = "make_lyrics"
    const val APPEARANCE = "appearance"

    /** The installed extensions, in the core's order. */
    fun list(plugins: List<PluginSummary>): List<PluginSummary> = plugins.filter { it.kind == PluginKind.Extension }

    fun makesLyrics(plugin: PluginSummary): Boolean = MAKE_LYRICS in plugin.capabilities

    fun changesTheLook(plugin: PluginSummary): Boolean = APPEARANCE in plugin.capabilities

    /** The page Appearance › Customize… opens: the active appearance plugin, else the first one installed. */
    fun customize(plugins: List<PluginSummary>, look: LookSnapshot): PluginSummary? =
        look.plugin?.let { id -> plugins.firstOrNull { it.id == id } } ?: list(plugins).firstOrNull(::changesTheLook)

    /** The AI Lyrics plugin: the slice's, else the first extension that makes lyrics. */
    fun aiLyrics(plugins: List<PluginSummary>, ai: AiLyricsSnapshot): PluginSummary? =
        ai.plugin?.let { id -> plugins.firstOrNull { it.id == id } } ?: list(plugins).firstOrNull(::makesLyrics)

    /** The Pro badge beside an extension that needs Pro (and Customize…) while Pro is inactive. */
    fun showsProBadge(plugin: PluginSummary, pro: ProSnapshot): Boolean = plugin.requiresPro && !pro.active

    /** Sending Audio: the origins this plugin may send songs to. */
    fun consents(ai: AiLyricsSnapshot, plugin: String): List<AudioConsent> = ai.consents.filter { it.plugin == plugin }
}
