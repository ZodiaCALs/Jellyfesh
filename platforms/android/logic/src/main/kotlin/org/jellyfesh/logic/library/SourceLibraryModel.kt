package org.jellyfesh.logic.library

import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SourceLibrary
import org.jellyfesh.logic.core.SourceLibraryEntry
import org.jellyfesh.logic.core.SourceLibraryList
import org.jellyfesh.logic.core.SourceLibraryPage
import org.jellyfesh.logic.core.SourceLibraryPart
import org.jellyfesh.logic.core.SourceLibrarySnapshot

/** One list of one source. */
data class SourceListKey(val plugin: String, val part: SourceLibraryPart)

/**
 * What Show More brought beyond each list's first page (which the slice holds): the entries, the
 * cursor for the next page, the first page they follow (its `refreshedAtMs`), the page on its way
 * and why the last one failed.
 */
data class SourceLibraryPages(
    val extra: Map<SourceListKey, List<SourceLibraryEntry>> = emptyMap(),
    val next: Map<SourceListKey, String?> = emptyMap(),
    val base: Map<SourceListKey, Long?> = emptyMap(),
    val pending: Map<SourceListKey, Long> = emptyMap(),
    val errors: Map<SourceListKey, String> = emptyMap(),
)

/**
 * A source's section on Favourites or Playlists: the lists of it the page shows. On Favourites, a
 * source whose likes all came back empty shows no lists and says "Nothing liked in {Service} yet."
 * ([nothingLikedAtMs]).
 */
data class SourceLibrarySection(
    val source: SourceLibrary,
    val lists: List<SourceLibraryList>,
    /** Every liked list of the source came back empty: when the last of them did ("Updated {time} ago"); else null. */
    val nothingLikedAtMs: Long? = null,
) {
    val nothingLiked: Boolean get() = nothingLikedAtMs != null
}

/**
 * The sources' likes and playlists (ADR-0024; docs/ui.md, "Playlists" and "Favourites"), from the
 * core's SourceLibrary slice: after the Jellyfesh playlists, one section per signed-in source with
 * its playlists; after the favourites, one with its liked songs, albums and artists. A list shows
 * once it has entries or something to say (loading, an error), with "Updated {time} ago" from its
 * age and Show More while the source has more. On Favourites a source whose likes came back empty
 * still shows its name, with "Nothing liked in {Service} yet." (v0.0.9). The lists refresh when
 * the page opens, and when it is returned to after ten minutes.
 */
object SourceLibraryModel {
    const val REFRESH_AFTER_MS: Long = 10 * 60 * 1_000

    /** The likes Favourites shows, in their order. */
    val LIKES: List<SourceLibraryPart> = listOf(SourceLibraryPart.LikedSongs, SourceLibraryPart.LikedAlbums, SourceLibraryPart.LikedArtists)

    /**
     * Favourites: each source with its liked songs, albums and artists that have entries, are
     * loading or failed (a failed list says why). A source whose liked lists all came back empty
     * is still listed, saying so; one whose lists have not come back yet, or that keeps no likes
     * at all, is left out: nothing is known of its likes.
     */
    fun likes(snapshot: SourceLibrarySnapshot): List<SourceLibrarySection> = snapshot.sources.mapNotNull { source ->
        val liked = source.lists.filter { it.part in LIKES }
        val lists = shown(liked, LIKES)
        val answered = liked.mapNotNull { it.refreshedAtMs }
        when {
            lists.isNotEmpty() -> SourceLibrarySection(source, lists)
            // None has entries, loads or failed: empty, once every one of them has come back.
            liked.isNotEmpty() && answered.size == liked.size -> SourceLibrarySection(source, emptyList(), nothingLikedAtMs = answered.max())
            else -> null
        }
    }

    /** Playlists: each source with its playlists, once they have entries, load or failed. */
    fun playlists(snapshot: SourceLibrarySnapshot): List<SourceLibrarySection> = snapshot.sources.mapNotNull { source ->
        val lists = shown(source.lists, listOf(SourceLibraryPart.Playlists))
        if (lists.isEmpty()) null else SourceLibrarySection(source, lists)
    }

    /** The lists of [parts] that have entries or something to say, in the order of [parts]. */
    private fun shown(lists: List<SourceLibraryList>, parts: List<SourceLibraryPart>): List<SourceLibraryList> =
        lists.filter { it.part in parts && (it.entries.isNotEmpty() || it.loading || it.error != null) }.sortedBy { parts.indexOf(it.part) }

    /** Whether the lists should refresh as the page shows again: when it opens, or after ten minutes since they last did. */
    fun refreshes(lastRefreshMs: Long?, nowMs: Long): Boolean = lastRefreshMs == null || nowMs - lastRefreshMs >= REFRESH_AFTER_MS

    /** The list's entries: its first page, then what Show More added after that page (each entry once). */
    fun entries(plugin: String, list: SourceLibraryList, pages: SourceLibraryPages): List<SourceLibraryEntry> {
        val extra = current(pages, SourceListKey(plugin, list.part), list)?.let { pages.extra[it] }.orEmpty()
        if (extra.isEmpty()) return list.entries
        val seen = list.entries.mapTo(HashSet()) { it.item.id }
        return list.entries + extra.filter { seen.add(it.item.id) }
    }

    /** The cursor for the list's next page: the last Show More's, else (none arrived yet, or it failed) the first page's. */
    fun next(plugin: String, list: SourceLibraryList, pages: SourceLibraryPages): String? {
        val key = current(pages, SourceListKey(plugin, list.part), list) ?: return list.next
        return if (key in pages.next) pages.next[key] else list.next
    }

    /** Show More for a list: the command for its next page, or null when it has none or one is on its way. */
    fun showMore(plugin: String, list: SourceLibraryList, pages: SourceLibraryPages): Command.SourceLibraryMore? {
        if (SourceListKey(plugin, list.part) in pages.pending) return null
        val cursor = next(plugin, list, pages) ?: return null
        return Command.SourceLibraryMore(plugin, list.part, cursor)
    }

    /** Show More was sent with id [request] (0 when the core is not running: nothing is on its way). */
    fun moreSent(pages: SourceLibraryPages, plugin: String, list: SourceLibraryList, request: Long): SourceLibraryPages {
        if (request == 0L) return pages
        val key = SourceListKey(plugin, list.part)
        // Pages that followed an older first page start over.
        val fresh = if (current(pages, key, list) == null) pages.copy(extra = pages.extra - key, next = pages.next - key) else pages
        return fresh.copy(pending = fresh.pending + (key to request), base = fresh.base + (key to list.refreshedAtMs), errors = fresh.errors - key)
    }

    fun loadingMore(pages: SourceLibraryPages, plugin: String, part: SourceLibraryPart): Boolean = SourceListKey(plugin, part) in pages.pending

    /** Show More's answer: its entries join the list, which keeps the new cursor; a failure is kept to show. */
    fun onPage(pages: SourceLibraryPages, page: SourceLibraryPage): SourceLibraryPages {
        val key = pending(pages, page.request) ?: return pages
        val rest = pages.pending - key
        if (page.error != null) return pages.copy(pending = rest, errors = pages.errors + (key to page.error))
        return pages.copy(
            pending = rest,
            extra = pages.extra + (key to pages.extra[key].orEmpty() + page.entries),
            next = pages.next + (key to page.next),
            errors = pages.errors - key,
        )
    }

    /** A refused Show More ("not available in this build yet"): its reason is kept to show. */
    fun onCommandFailed(pages: SourceLibraryPages, id: Long, error: String): SourceLibraryPages {
        val key = pending(pages, id) ?: return pages
        return pages.copy(pending = pages.pending - key, errors = pages.errors + (key to error))
    }

    fun error(pages: SourceLibraryPages, plugin: String, part: SourceLibraryPart): String? = pages.errors[SourceListKey(plugin, part)]

    /** A source playlist's entry in its source's Playlists list, when the slice has it. */
    fun entry(snapshot: SourceLibrarySnapshot, plugin: String, id: String): SourceLibraryEntry? = snapshot.sources
        .firstOrNull { it.plugin == plugin }
        ?.lists?.firstOrNull { it.part == SourceLibraryPart.Playlists }
        ?.entries?.firstOrNull { it.item.id == id }

    /**
     * Whether a source playlist's page says "{Service} lists this playlist's songs only in
     * {Service}.": the service lists no songs for it (a followed Spotify playlist). Play and Shuffle
     * still start it there.
     */
    fun songsOnlyInService(snapshot: SourceLibrarySnapshot, plugin: String, id: String): Boolean = entry(snapshot, plugin, id)?.songsListed == false

    /** Save as Playlist…: offered for a source's playlist. */
    fun canSaveAsPlaylist(collection: CollectionRef): Boolean = collection is CollectionRef.Source && collection.kind == ItemKind.Playlist

    /** Save as Playlist… with the name the person gave: a Jellyfesh playlist of its songs (answered by PlaylistCreated); null for a blank name. */
    fun saveAsPlaylist(collection: CollectionRef.Source, name: String): Command? =
        name.trim().takeIf { it.isNotEmpty() }?.let { Command.SavePlaylistCopy(collection.plugin, collection.id, it) }

    /** The key when Show More's pages follow the list's current first page; null when they follow an older one. */
    private fun current(pages: SourceLibraryPages, key: SourceListKey, list: SourceLibraryList): SourceListKey? =
        key.takeIf { it in pages.base && pages.base[it] == list.refreshedAtMs }

    private fun pending(pages: SourceLibraryPages, request: Long): SourceListKey? = pages.pending.entries.firstOrNull { it.value == request }?.key
}
