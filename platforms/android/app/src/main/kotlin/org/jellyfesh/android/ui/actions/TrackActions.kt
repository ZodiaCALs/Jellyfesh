package org.jellyfesh.android.ui.actions

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
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.library.ArtistMatch
import org.jellyfesh.logic.library.CollectionTracks
import org.jellyfesh.logic.library.TrackMenu
import org.jellyfesh.logic.library.TrackMenuItem

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

    /**
     * The menu of one song: Play Next, Play Last, Add to Playlist…, Add to or Remove from
     * Favourites, Go to Album, Go to Artist ([TrackMenu]), then [extra]. What the row did not
     * know (whether it is a favourite, its album and artist) is read from the library first.
     */
    fun showFor(target: TrackTarget, extra: List<SheetAction> = emptyList()) {
        val library = target.track as? TrackRef.Library
        val needsDetails = library != null && (target.albumId == null || target.artist == null)
        if (target.favorite != null && !needsDetails) {
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
        val albumId = target.albumId
        val artist = target.artist?.takeIf { it.isNotBlank() && target.track is TrackRef.Library }
        val items = TrackMenu.items(target.track, target.favorite, hasAlbum = albumId != null, hasArtist = artist != null)
        val actions = items.map { item ->
            when (item) {
                TrackMenuItem.PlayNext -> SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(listOf(target.track)) }
                TrackMenuItem.PlayLast -> SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(listOf(target.track)) }
                TrackMenuItem.AddToPlaylist -> SheetAction(s(R.string.action_add_to_playlist), R.drawable.ic_playlist) { addToPlaylist(listOf(target.track)) }
                TrackMenuItem.AddToFavourites -> SheetAction(s(R.string.action_favourite), R.drawable.ic_heart) { graph.send(Command.SetFavorite(target.track, true)) }
                TrackMenuItem.RemoveFromFavourites -> SheetAction(s(R.string.action_unfavourite), R.drawable.ic_heart_filled) { graph.send(Command.SetFavorite(target.track, false)) }
                TrackMenuItem.GoToAlbum -> SheetAction(s(R.string.action_go_to_album), R.drawable.ic_album) { albumId?.let { nav.openAlbum(it, target.albumTitle ?: target.title) } }
                TrackMenuItem.GoToArtist -> SheetAction(s(R.string.action_go_to_artist), R.drawable.ic_artist) { artist?.let(::goToArtist) }
            }
        }
        overlays.showActions(target.title, target.subtitle, actions + extra)
    }

    /** Opens the library's artist of this name (songs and albums carry the name only). */
    fun goToArtist(name: String) {
        graph.scope.launch {
            val found = graph.query { it.searchLibrary(name, 20) }.valueOrNull()?.artists.orEmpty()
            ArtistMatch.find(name, found)?.let { nav.openArtist(it.id, it.name) }
        }
    }

    /**
     * The More menu of an album, an artist or a playlist: Play Next, Play Last and Add to
     * Playlist…, then [extra] (a playlist's Rename…, Export Playlist… and Delete Playlist…).
     * Lists without Play and Shuffle buttons ([withPlay]) offer those first.
     */
    fun showForCollection(
        title: String,
        subtitle: String?,
        collection: CollectionRef,
        extra: List<SheetAction> = emptyList(),
        withPlay: Boolean = true,
    ) {
        val actions = buildList {
            if (withPlay) {
                add(SheetAction(s(R.string.action_play), R.drawable.ic_play) { graph.send(Command.PlayCollection(collection, 0, false)) })
                add(SheetAction(s(R.string.action_shuffle), R.drawable.ic_shuffle) { graph.send(Command.PlayCollection(collection, 0, true)) })
            }
            add(SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(collection) })
            add(SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(collection) })
            if (CollectionTracks.canAdd(collection)) {
                add(SheetAction(s(R.string.action_add_to_playlist), R.drawable.ic_playlist) { addToPlaylist(collection) })
            }
            addAll(extra)
        }
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
    return remember(graph, nav, overlays, context) { TrackActions(graph, nav, overlays, context) }
}
