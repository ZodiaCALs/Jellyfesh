package org.jellyfesh.android.ui.actions

import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.library.DetailsModel
import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.android.ui.player.rememberInfoSheets
import org.jellyfesh.android.ui.player.KnownInfo
import org.jellyfesh.android.ui.player.InfoSheets
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.valueOrNull
import org.jellyfesh.android.ui.components.ActionRow
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.components.SheetAction
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.nav.NavController
import org.jellyfesh.android.ui.player.LyricsMaker
import org.jellyfesh.android.ui.player.rememberLyricsMaker
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.SongRef
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.library.ArtistMatch
import org.jellyfesh.logic.library.CollectionMenu
import org.jellyfesh.android.ui.library.LocalDownloadGate
import org.jellyfesh.logic.library.DownloadMenus
import org.jellyfesh.logic.library.GoTo
import org.jellyfesh.logic.library.GoToPlaces
import org.jellyfesh.logic.library.SourceLibraryModel
import org.jellyfesh.logic.library.CollectionTracks
import org.jellyfesh.logic.library.CollectionMenuItem
import org.jellyfesh.logic.library.TrackMenu
import org.jellyfesh.logic.library.TrackMenuItem
import org.jellyfesh.logic.lyrics.LyricsMakingModel
import org.jellyfesh.logic.player.RemoteModel
import org.jellyfesh.logic.sources.Links

/** What a row's More menu acts on. Unknown details (null) are looked up when the menu opens. */
data class TrackTarget(
    val track: TrackRef,
    val title: String,
    val subtitle: String?,
    val albumId: Long? = null,
    val albumTitle: String? = null,
    val favorite: Boolean? = null,
    /** The song's artist, for Go to Artist. */
    val artist: String? = null,
    /** Opens the song in its service (Open in {Service}). */
    val link: String? = null,
    /** The source the link belongs to (a plugin id), for the service's name; else the song's own source. */
    val source: String? = null,
    /** A source's ids of the song's album and artist: Go to Album and Go to Artist open its pages (v0.0.7). */
    val sourceAlbumId: String? = null,
    val sourceArtistId: String? = null,
    val durationMs: Long? = null,
    /** A source's song whose file is on this device (the Downloaded mark): no Download in its menu. */
    val downloaded: Boolean = false,
)

/**
 * The menus of songs, albums, artists and playlists (docs/ui.md, "Song menu" and "Album, artist
 * and playlist pages"). They only send commands; the core decides what happens.
 */
@Stable
class TrackActions internal constructor(
    private val graph: AppGraph,
    private val nav: NavController,
    private val overlays: Overlays,
    private val context: android.content.Context,
    private val lyricsMaker: LyricsMaker,
    private val infoSheets: InfoSheets,
    /** Sends a Download once storage allows it (Android 8 and 9 ask first; ADR-0025 §2). */
    private val download: (Command) -> Unit,
) {
    private fun s(id: Int, vararg args: Any): String = context.getString(id, *args)

    fun playNext(tracks: List<TrackRef>) {
        graph.send(Command.Enqueue(tracks, playNext = true))
        overlays.toast(s(R.string.toast_plays_next))
    }

    fun playLast(tracks: List<TrackRef>) {
        graph.send(Command.Enqueue(tracks, playNext = false))
        overlays.toast(s(R.string.toast_added_to_queue))
    }

    fun playNext(collection: CollectionRef) {
        graph.send(Command.EnqueueCollection(collection, playNext = true))
        overlays.toast(s(R.string.toast_plays_next))
    }

    fun playLast(collection: CollectionRef) {
        graph.send(Command.EnqueueCollection(collection, playNext = false))
        overlays.toast(s(R.string.toast_added_to_queue))
    }

    /** Open in {Service}: the service's app or the browser; a short message when nothing can open it. */
    fun openLink(link: String) {
        if (!openLinkIntent(context, link)) overlays.toast(s(R.string.link_no_app))
    }

    /** "Open in QQ Music": the label of the link of an item from [source] (a plugin id). */
    fun openInLabel(source: String): String = s(R.string.action_open_in, Links.serviceName(graph.store.state.value.sources, source))

    /** A sheet of [actions] under [title] (and [subtitle]). */
    fun showSheetActions(title: String, subtitle: String?, actions: List<SheetAction>) = overlays.showActions(title, subtitle, actions)

    /** The Open in {Service} action of an item from [source], when [link] may be opened. */
    fun openLinkAction(link: String?, source: String?): SheetAction? {
        val url = Links.openable(link) ?: return null
        return SheetAction(openInLabel(source.orEmpty()), R.drawable.ic_external) { openLink(url) }
    }

    /**
     * The menu of one song: Play Next, Play Last, Add to Playlist…, Add to or Remove from
     * Favourites, Go to Album, Go to Artist ([TrackMenu]), then [extra]. What the row did not
     * know (whether it is a favourite, its album and artist) is read from the library first.
     */
    fun showFor(target: TrackTarget, extra: List<SheetAction> = emptyList()) {
        val library = target.track as? TrackRef.Library
        val needsDetails = library != null && (target.albumId == null || target.artist == null)
        // Sources keep their own favourites: a source's song offers no heart unless the row knows it.
        if ((target.favorite != null || target.track is TrackRef.Source) && !needsDetails) {
            present(target, extra)
            return
        }
        graph.scope.launch {
            var known = target
            if (known.favorite == null) known = known.copy(favorite = graph.query { it.isFavorite(target.track) }.valueOrNull())
            if (library != null && needsDetails) {
                graph.query { it.track(library.id) }.valueOrNull()?.let { t ->
                    known = known.copy(albumId = known.albumId ?: t.albumId, albumTitle = known.albumTitle ?: t.album, artist = known.artist ?: t.artist)
                }
            }
            present(known, extra)
        }
    }

    private fun present(target: TrackTarget, extra: List<SheetAction>) {
        val sources = graph.store.state.value.sources
        // The library's album and artist, or a source's song's pages in its source (ADR-0022, ADR-0027).
        val plugin = (target.track as? TrackRef.Source)?.track?.plugin
        val album = GoToPlaces.album(target.track, target.title, target.albumId, target.albumTitle, target.sourceAlbumId, target.artist)
        val artist = GoToPlaces.artist(target.track, target.title, target.artist, target.sourceArtistId)
        val linkAction = openLinkAction(target.link, target.source ?: plugin)
        val items = TrackMenu.items(
            target.track,
            target.favorite,
            hasAlbum = album != null,
            hasArtist = artist != null,
            remote = RemoteModel.isRemote(sources, target.track),
            hasLink = linkAction != null,
            makesLyrics = LyricsMakingModel.offersFor(graph.store.state.value.aiLyrics, target.track),
            getInfo = true,
            download = DownloadMenus.canDownload(sources, target.track, target.downloaded),
        )
        val actions = items.mapNotNull { item ->
            when (item) {
                TrackMenuItem.PlayNext -> SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(listOf(target.track)) }
                TrackMenuItem.Download -> SheetAction(s(R.string.action_download), R.drawable.ic_download) { download(DownloadMenus.download(target.track)) }
                TrackMenuItem.PlayLast -> SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(listOf(target.track)) }
                TrackMenuItem.AddToPlaylist -> SheetAction(s(R.string.action_add_to_playlist), R.drawable.ic_playlist) { addToPlaylist(listOf(target.track)) }
                TrackMenuItem.AddToFavourites -> SheetAction(s(R.string.action_favourite), R.drawable.ic_heart) { graph.send(Command.SetFavorite(target.track, true)) }
                TrackMenuItem.RemoveFromFavourites -> SheetAction(s(R.string.action_unfavourite), R.drawable.ic_heart_filled) { graph.send(Command.SetFavorite(target.track, false)) }
                TrackMenuItem.GoToAlbum -> album?.let { place -> SheetAction(s(R.string.action_go_to_album), R.drawable.ic_album) { go(place) } }
                TrackMenuItem.GoToArtist -> artist?.let { place -> SheetAction(s(R.string.action_go_to_artist), R.drawable.ic_artist) { go(place) } }
                TrackMenuItem.GetInfo -> SheetAction(s(R.string.action_get_info), R.drawable.ic_info) {
                    infoSheets.show(
                        DetailsModel.itemFor(target.track),
                        KnownInfo(target.title, target.artist, target.albumTitle, target.durationMs, plugin),
                    )
                }
                TrackMenuItem.CreateSyncedLyrics -> (target.track as? TrackRef.Library)?.let { song ->
                    SheetAction(s(R.string.ai_create), R.drawable.ic_sparkle) { lyricsMaker.make(SongRef.Library(song.id)) }
                }
                TrackMenuItem.OpenLink -> linkAction
            }
        }
        overlays.showActions(target.title, target.subtitle, actions + extra)
    }

    /** Get Info for an album or an artist (their pages' More menus). */
    fun getInfoAction(item: ItemRef, known: KnownInfo): SheetAction =
        SheetAction(s(R.string.action_get_info), R.drawable.ic_info) { infoSheets.show(item, known) }

    /** Opens the library's artist of this name (songs and albums carry the name only). */
    fun goToArtist(name: String) {
        graph.scope.launch {
            val found = graph.query { it.searchLibrary(name, 20) }.valueOrNull()?.artists.orEmpty()
            ArtistMatch.find(name, found)?.let { nav.openArtist(it.id, it.name) }
        }
    }

    /** Go to Album or Go to Artist: the library's page, or a source's. */
    private fun go(place: GoTo) {
        when (place) {
            is GoTo.LibraryAlbum -> nav.openAlbum(place.id, place.title)
            is GoTo.LibraryArtist -> goToArtist(place.name)
            is GoTo.SourcePage -> nav.push(Screen.SourceBrowse(place.plugin, place.id, place.title, kind = place.kind, subtitle = place.subtitle))
        }
    }

    /**
     * The More menu of an album, an artist or a playlist ([CollectionMenu]): Play Next, Play Last
     * and Add to Playlist… (none of them for a remote source's), Go to Artist for an album whose
     * [artist] (and, from a source, [sourceArtistId]) is given, Download for a source's album or
     * playlist when the source offers downloads, Open in {Service} when it has a [link], then
     * [extra] (a playlist's Rename…, Export Playlist… and Delete Playlist…). Lists without Play
     * and Shuffle buttons ([withPlay]: cards and rows) offer those first.
     */
    fun showForCollection(
        title: String,
        subtitle: String?,
        collection: CollectionRef,
        extra: List<SheetAction> = emptyList(),
        withPlay: Boolean = true,
        link: String? = null,
        artist: String? = null,
        sourceArtistId: String? = null,
    ) {
        val sources = graph.store.state.value.sources
        val linkAction = openLinkAction(link, (collection as? CollectionRef.Source)?.plugin)
        val artistPage = GoToPlaces.artist(collection, title, artist, sourceArtistId)
        val items = CollectionMenu.items(
            collection,
            withPlay,
            remote = RemoteModel.isRemote(sources, collection),
            hasLink = linkAction != null,
            download = DownloadMenus.canDownload(sources, collection),
            saveAsPlaylist = SourceLibraryModel.canSaveAsPlaylist(collection),
            hasArtist = artistPage != null,
        )
        val actions = items.mapNotNull { item ->
            when (item) {
                CollectionMenuItem.Play -> SheetAction(s(R.string.action_play), R.drawable.ic_play) { graph.send(Command.PlayCollection(collection, 0, false)) }
                CollectionMenuItem.Shuffle -> SheetAction(s(R.string.action_shuffle), R.drawable.ic_shuffle) { graph.send(Command.PlayCollection(collection, 0, true)) }
                CollectionMenuItem.PlayNext -> SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(collection) }
                CollectionMenuItem.PlayLast -> SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(collection) }
                CollectionMenuItem.AddToPlaylist -> SheetAction(s(R.string.action_add_to_playlist), R.drawable.ic_playlist) { addToPlaylist(collection) }
                CollectionMenuItem.GoToArtist -> artistPage?.let { place -> SheetAction(s(R.string.action_go_to_artist), R.drawable.ic_artist) { go(place) } }
                // Save as Playlist…: a Jellyfesh playlist of its songs, named first (ADR-0024).
                CollectionMenuItem.SaveAsPlaylist -> (collection as? CollectionRef.Source)?.let { source ->
                    SheetAction(s(R.string.action_save_as_playlist), R.drawable.ic_playlist) {
                        overlays.prompt(s(R.string.save_as_playlist_title), s(R.string.playlist_name), title, s(R.string.action_save)) { name ->
                            SourceLibraryModel.saveAsPlaylist(source, name)?.let(graph::send)
                        }
                    }
                }
                CollectionMenuItem.Download -> (collection as? CollectionRef.Source)?.let { source ->
                    SheetAction(s(R.string.action_download), R.drawable.ic_download) { download(DownloadMenus.download(source)) }
                }
                CollectionMenuItem.OpenLink -> linkAction
            }
        } + extra
        overlays.showActions(title, subtitle, actions)
    }

    /** The playlist picker for songs: the person's playlists and New Playlist…. */
    fun addToPlaylist(tracks: List<TrackRef>) = choosePlaylist(exclude = null) { tracks }

    /** The playlist picker for a collection: its songs are gathered from the library once one is chosen. */
    fun addToPlaylist(collection: CollectionRef) = choosePlaylist(exclude = (collection as? CollectionRef.Playlist)?.id) {
        graph.query { CollectionTracks.of(it, collection) }.valueOrNull().orEmpty()
    }

    private fun choosePlaylist(exclude: Long?, tracks: suspend () -> List<TrackRef>) {
        overlays.showSheet(s(R.string.playlist_add_title)) { dismiss ->
            AddToPlaylistContent(
                exclude = exclude,
                onPick = { id, name ->
                    dismiss()
                    graph.scope.launch {
                        val refs = tracks()
                        if (refs.isNotEmpty()) {
                            graph.send(Command.AddToPlaylist(id, refs))
                            overlays.toast(s(R.string.toast_added_to_playlist, name))
                        }
                    }
                },
                onNew = {
                    dismiss()
                    // New Playlist… asks for the name before it creates anything.
                    overlays.prompt(s(R.string.playlist_new_title), s(R.string.playlist_name), "", s(R.string.action_create)) { name ->
                        graph.scope.launch { graph.send(Command.CreatePlaylist(name, tracks())) }
                    }
                },
            )
        }
    }
}

@Composable
private fun AddToPlaylistContent(exclude: Long?, onPick: (Long, String) -> Unit, onNew: () -> Unit) {
    val graph = LocalGraph.current
    val revision = graph.store.state.value.libraryRevision
    val playlists by rememberQuery(revision) { it.playlists() }
    Column {
        ActionRow(stringResource(R.string.action_new_playlist), R.drawable.ic_add, onClick = onNew)
        when (val p = playlists) {
            QueryResult.Loading -> LoadingRow()
            is QueryResult.Ok -> p.value.filter { it.id != exclude }.forEach { playlist ->
                ActionRow(playlist.name, R.drawable.ic_playlist) { onPick(playlist.id, playlist.name) }
            }
            is QueryResult.NotAvailable -> JfText(stringResource(R.string.not_available_yet), style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(20.dp))
            is QueryResult.Failed -> JfText(stringResource(R.string.query_failed, p.message), style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(20.dp))
        }
    }
}

@Composable
fun rememberTrackActions(): TrackActions {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val lyricsMaker = rememberLyricsMaker()
    val infoSheets = rememberInfoSheets()
    val gate = LocalDownloadGate.current
    return remember(graph, nav, overlays, context, lyricsMaker, infoSheets, gate) {
        TrackActions(graph, nav, overlays, context, lyricsMaker, infoSheets) { command -> gate?.download(command) ?: graph.send(command) }
    }
}
