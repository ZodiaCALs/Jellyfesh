package org.jellyfesh.logic.sources

import java.net.URI
import org.jellyfesh.logic.core.ArtworkRef
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackRef

/**
 * Item links (docs/ui.md, "Links and remote playback"; core-api.md, "Links"): Open in {Service}
 * wherever an item has a link, the plugin's name as {Service}.
 */
object Links {
    /**
     * [link] when the app may hand it to the system: an absolute `https` link with a host, as the
     * core sends them, with its scheme in lower case (Android matches schemes case-sensitively).
     * Anything else (another scheme, an `intent:` link, a relative path) is never opened.
     */
    fun openable(link: String?): String? {
        val text = link?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val uri = runCatching { URI(text) }.getOrNull() ?: return null
        val scheme = uri.scheme ?: return null
        if (!scheme.equals("https", ignoreCase = true) || uri.host.isNullOrBlank()) return null
        return "https" + text.substring(scheme.length)
    }

    /** The {Service} of Open in {Service}: the plugin's name, else its id. */
    fun serviceName(sources: SourcesSnapshot, plugin: String): String =
        sources.plugins.firstOrNull { it.id == plugin }?.name?.takeIf { it.isNotBlank() } ?: plugin

    /**
     * The {Service} of "Content from {Service}", the credit a source's views show when its plugin
     * asks for attribution (docs/ui.md, "Links and remote playback"); null for every other source.
     */
    fun attributedService(sources: SourcesSnapshot, plugin: String): String? =
        sources.plugins.firstOrNull { it.id == plugin }?.takeIf { !it.attribution.isNullOrBlank() }?.let { serviceName(sources, plugin) }
}

/** What choosing a row from a source does (search and browse rows). */
sealed interface ItemAction {
    /** Plays the row's track with the other playable tracks of its list, from it (`PlayTracks`). */
    data class Play(val tracks: List<TrackRef>, val start: Int) : ItemAction

    /**
     * Opens an album, artist, playlist or folder in the source's browser, with what its row shows
     * for the page's header ([link]: its Open in {Service}).
     */
    data class Browse(
        val plugin: String,
        val node: String,
        val title: String,
        val link: String?,
        val kind: ItemKind = ItemKind.Folder,
        val subtitle: String? = null,
        val artwork: ArtworkRef? = null,
    ) : ItemAction

    /** Opens the item in its service: it cannot play here but has a link. */
    data class OpenLink(val url: String) : ItemAction

    /** Nothing: an item that cannot play here and has no link (its reason shows instead). */
    data object None : ItemAction
}

object SourceItems {
    /**
     * The main action of [item] in [list] (tap, click or Enter): a playable track plays with the
     * list's other playable tracks; an item that cannot play here but has a link opens it; other
     * albums, artists, playlists and folders open in the browser; anything else does nothing.
     */
    fun mainAction(item: SourceItem, list: List<SourceItem>): ItemAction {
        val track = item.track
        val link = Links.openable(item.link)
        return when {
            item.playable && track != null -> {
                val tracks = list.filter { it.playable }.mapNotNull { it.track }
                ItemAction.Play(tracks, tracks.indexOf(track).coerceAtLeast(0))
            }
            !item.playable && link != null -> ItemAction.OpenLink(link)
            item.kind != ItemKind.Track -> ItemAction.Browse(item.source, item.id, item.title, link, item.kind, item.subtitle, item.artwork)
            else -> ItemAction.None
        }
    }

    /**
     * What a card's play button plays (docs/ui.md, "Rows and cards"): a playable album or
     * playlist, whole. Songs have none (choosing one plays it), nor do artists and folders.
     */
    fun playsWhole(item: SourceItem): CollectionRef? =
        if (item.playable && (item.kind == ItemKind.Album || item.kind == ItemKind.Playlist)) CollectionRef.Source(item.source, item.id, item.kind) else null
}
