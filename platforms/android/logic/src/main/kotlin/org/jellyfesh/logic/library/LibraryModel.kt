package org.jellyfesh.logic.library

import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.Artist
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreClient
import org.jellyfesh.logic.core.DownloadTarget
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.LibraryStats
import org.jellyfesh.logic.core.SourcesSnapshot
import org.jellyfesh.logic.core.TrackQuery
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackSort

/** A list's order: what it is sorted by, and whether the order runs backwards (the menu's Descending). */
data class SortOrder<F>(val field: F, val descending: Boolean)

/**
 * The sort menus of Albums and Songs (docs/ui.md, "Pages"): the fields in the menu's order, then
 * Descending. The core sorts; the menu only says how. Choosing a field starts it the way people
 * read it (newest first for Year and Recently Added, A to Z otherwise), and Descending turns the
 * order round from there.
 */
object SortMenus {
    /** Albums: Title, Artist, Year, Recently Added. */
    val ALBUM_FIELDS: List<AlbumSort> = listOf(AlbumSort.Title, AlbumSort.Artist, AlbumSort.Year, AlbumSort.Added)

    /** Songs: Title, Artist, Album, Year, Recently Added. */
    val SONG_FIELDS: List<TrackSort> = listOf(TrackSort.Title, TrackSort.Artist, TrackSort.Album, TrackSort.Year, TrackSort.Added)

    val DEFAULT_ALBUMS: SortOrder<AlbumSort> = album(AlbumSort.Title)
    val DEFAULT_SONGS: SortOrder<TrackSort> = song(TrackSort.Artist)

    /** The order an Albums field starts in. */
    fun album(field: AlbumSort): SortOrder<AlbumSort> = SortOrder(field, field == AlbumSort.Year || field == AlbumSort.Added)

    /** The order a Songs field starts in. */
    fun song(field: TrackSort): SortOrder<TrackSort> = SortOrder(field, field == TrackSort.Year || field == TrackSort.Added)

    /** Descending switched on or off. */
    fun <F> descending(order: SortOrder<F>, on: Boolean): SortOrder<F> = order.copy(descending = on)

    /** The query for a page of Songs in [order]. */
    fun songsQuery(order: SortOrder<TrackSort>, offset: Int, limit: Int): TrackQuery =
        TrackQuery(offset = offset, limit = limit, sort = order.field, descending = order.descending)

    /**
     * Every library song in the Songs page's [order], as one collection: Play, Shuffle and a row
     * cover the whole list, whatever has been read of it (the core plays it, with no paging).
     */
    fun allSongs(order: SortOrder<TrackSort>): CollectionRef = CollectionRef.Songs(order.field, order.descending)
}

/** The songs a collection holds, as Add to Playlist… adds them (the core's queries, page by page). */
object CollectionTracks {
    /** Rows read per query while gathering a long collection. */
    const val PAGE = 500

    /** At most this many rows are gathered, whatever the collection holds. */
    const val MAX = 50_000

    /** Whether Add to Playlist… can gather [collection]'s songs here (a source's containers cannot). */
    fun canAdd(collection: CollectionRef): Boolean = collection !is CollectionRef.Source

    /**
     * Every song of [collection] that a playlist can hold (library songs and files), in the
     * collection's order. Runs queries: call it off the main thread.
     */
    fun of(core: CoreClient, collection: CollectionRef): List<TrackRef> = when (collection) {
        is CollectionRef.Album -> core.albumTracks(collection.id).map { TrackRef.Library(it.id) }
        is CollectionRef.Artist -> pages { offset ->
            core.tracks(TrackQuery(offset = offset, limit = PAGE, sort = TrackSort.Album, artistId = collection.id)).map { TrackRef.Library(it.id) }
        }
        is CollectionRef.Playlist -> pages { offset -> core.playlistEntries(collection.id, offset, PAGE).map { it.row.track } }
        CollectionRef.Favorites -> pages { offset -> core.favorites(offset, PAGE).map { it.track } }
        is CollectionRef.Songs -> pages { offset ->
            core.tracks(TrackQuery(offset = offset, limit = PAGE, sort = collection.sort, descending = collection.descending)).map { TrackRef.Library(it.id) }
        }
        is CollectionRef.Tracks -> collection.tracks
        is CollectionRef.Source -> emptyList()
    }.filter(::playlistCanHold)

    /** Playlists hold library songs and files; a source's songs are left out, as the song menu does. */
    fun playlistCanHold(track: TrackRef): Boolean = track is TrackRef.Library || track is TrackRef.File

    private inline fun pages(read: (offset: Int) -> List<TrackRef>): List<TrackRef> {
        val out = ArrayList<TrackRef>()
        while (true) {
            val page = read(out.size)
            out += page
            if (page.size < PAGE || out.size >= MAX) return out
        }
    }
}

/** Finds the library's artist for a name the core gave with a song or an album. */
object ArtistMatch {
    /** The artist named exactly [name], else the first whose name matches ignoring case, else null. */
    fun find(name: String, artists: List<Artist>): Artist? =
        artists.firstOrNull { it.name == name } ?: artists.firstOrNull { it.name.equals(name, ignoreCase = true) }
}

/** Home (docs/ui.md, "Pages"). */
object HomeModel {
    /**
     * Whether Home shows the welcome instead of its shelves: nothing in the library (and no scan
     * filling it right now) and no source signed in.
     */
    fun showsWelcome(stats: LibraryStats?, scanning: Boolean, sources: SourcesSnapshot): Boolean =
        stats != null && stats.tracks == 0L && !scanning && sources.plugins.none { it.account != null }
}

/** The items of a song's menu (long press or More), in the order every app lists them. */
enum class TrackMenuItem { PlayNext, PlayLast, AddToPlaylist, AddToFavourites, RemoveFromFavourites, Download, GoToAlbum, GoToArtist, CreateSyncedLyrics, GetInfo, OpenLink }

object TrackMenu {
    /**
     * The song menu (docs/ui.md, "Song menu" and "Links and remote playback"): Play Next, Play
     * Last and Add to Playlist… (songs a playlist can hold), none of them for a remote source's
     * songs, which never join the queue; Add to or Remove from Favourites (when the favourite
     * state is known); Download for a source's song not downloaded yet ([download], ADR-0025);
     * Go to Album and Go to Artist (when the library has them); Create Synced Lyrics while AI
     * Lyrics makes them for it ([makesLyrics], v0.0.7); then Open in {Service} when the song has a
     * link.
     */
    fun items(
        track: TrackRef,
        favourite: Boolean?,
        hasAlbum: Boolean,
        hasArtist: Boolean,
        remote: Boolean = false,
        hasLink: Boolean = false,
        makesLyrics: Boolean = false,
        getInfo: Boolean = false,
        download: Boolean = false,
    ): List<TrackMenuItem> = buildList {
        if (!remote) {
            add(TrackMenuItem.PlayNext)
            add(TrackMenuItem.PlayLast)
            if (CollectionTracks.playlistCanHold(track)) add(TrackMenuItem.AddToPlaylist)
        }
        when (favourite) {
            true -> add(TrackMenuItem.RemoveFromFavourites)
            false -> add(TrackMenuItem.AddToFavourites)
            null -> Unit
        }
        if (download) add(TrackMenuItem.Download)
        if (hasAlbum) add(TrackMenuItem.GoToAlbum)
        if (hasArtist) add(TrackMenuItem.GoToArtist)
        if (makesLyrics) add(TrackMenuItem.CreateSyncedLyrics)
        if (getInfo) add(TrackMenuItem.GetInfo)
        if (hasLink) add(TrackMenuItem.OpenLink)
    }
}

/** The items of an album's, artist's or playlist's More menu, before a playlist's own (Rename… and on). */
enum class CollectionMenuItem { Play, Shuffle, PlayNext, PlayLast, AddToPlaylist, Download, OpenLink }

object CollectionMenu {
    /**
     * The More menu of an album, artist or playlist (docs/ui.md, "Album, artist and playlist
     * pages"): Play and Shuffle where the list has no buttons for them ([withPlay]); Play Next and
     * Play Last, except for a remote source's (its songs never join the queue); Add to Playlist…
     * when the songs can be gathered here; Download for a source's album or playlist when the
     * source offers downloads ([download], ADR-0025); then Open in {Service} when it has a link.
     */
    fun items(
        collection: CollectionRef,
        withPlay: Boolean,
        remote: Boolean = false,
        hasLink: Boolean = false,
        download: Boolean = false,
    ): List<CollectionMenuItem> = buildList {
        if (withPlay) {
            add(CollectionMenuItem.Play)
            add(CollectionMenuItem.Shuffle)
        }
        if (!remote) {
            add(CollectionMenuItem.PlayNext)
            add(CollectionMenuItem.PlayLast)
            if (CollectionTracks.canAdd(collection)) add(CollectionMenuItem.AddToPlaylist)
        }
        if (download) add(CollectionMenuItem.Download)
        if (hasLink) add(CollectionMenuItem.OpenLink)
    }
}

/**
 * Download in the menus (docs/ui.md, "Album, artist and playlist pages"; ADR-0025): a source's
 * songs, albums and playlists, when the source offers downloads (`PluginSummary.download`, which
 * the core enforces too), and for a song only until it is downloaded.
 */
object DownloadMenus {
    fun offersDownloads(sources: SourcesSnapshot, plugin: String?): Boolean =
        plugin != null && sources.plugins.any { it.id == plugin && it.download && it.enabled }

    /** A song's Download: a source's song that is not downloaded yet. */
    fun canDownload(sources: SourcesSnapshot, track: TrackRef, downloaded: Boolean): Boolean =
        !downloaded && track is TrackRef.Source && offersDownloads(sources, track.track.plugin)

    /** An album's or playlist's Download: one a source listed. */
    fun canDownload(sources: SourcesSnapshot, collection: CollectionRef): Boolean =
        collection is CollectionRef.Source && collection.kind in DOWNLOADABLE && offersDownloads(sources, collection.plugin)

    fun download(track: TrackRef): Command = Command.Download(DownloadTarget.Track(track))

    fun download(collection: CollectionRef.Source): Command = Command.Download(DownloadTarget.Collection(collection.plugin, collection.id, collection.kind))

    private val DOWNLOADABLE = setOf(ItemKind.Album, ItemKind.Playlist)
}
