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
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
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

/** What a row's More menu acts on. */
data class TrackTarget(
    val track: TrackRef,
    val title: String,
    val subtitle: String?,
    val albumId: Long? = null,
    val albumTitle: String? = null,
    val favorite: Boolean? = null,
)

/**
 * The actions every song, album and playlist offers: play next, play last, add to a playlist,
 * favourite and go to the album. They only send commands; the core decides what happens.
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

    /** The More menu of one song. */
    fun showFor(target: TrackTarget, extra: List<SheetAction> = emptyList()) {
        val actions = buildList {
            add(SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(listOf(target.track)) })
            add(SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(listOf(target.track)) })
            if (target.track is TrackRef.Library || target.track is TrackRef.File) {
                add(SheetAction(s(R.string.action_add_to_playlist), R.drawable.ic_playlist) { addToPlaylist(listOf(target.track)) })
            }
            when (target.favorite) {
                true -> add(SheetAction(s(R.string.action_unfavourite), R.drawable.ic_heart_filled) { graph.send(Command.SetFavorite(target.track, false)) })
                false -> add(SheetAction(s(R.string.action_favourite), R.drawable.ic_heart) { graph.send(Command.SetFavorite(target.track, true)) })
                null -> Unit
            }
            if (target.albumId != null) {
                add(SheetAction(s(R.string.action_go_to_album), R.drawable.ic_album) { nav.openAlbum(target.albumId, target.albumTitle ?: target.title) })
            }
            addAll(extra)
        }
        overlays.showActions(target.title, target.subtitle, actions)
    }

    /** The More menu of an album, an artist or a playlist (a collection). */
    fun showForCollection(title: String, subtitle: String?, collection: CollectionRef, extra: List<SheetAction> = emptyList()) {
        val actions = buildList {
            add(SheetAction(s(R.string.action_play), R.drawable.ic_play) { graph.send(Command.PlayCollection(collection, 0, false)) })
            add(SheetAction(s(R.string.action_shuffle), R.drawable.ic_shuffle) { graph.send(Command.PlayCollection(collection, 0, true)) })
            add(SheetAction(s(R.string.action_play_next), R.drawable.ic_play_next) { playNext(collection) })
            add(SheetAction(s(R.string.action_play_last), R.drawable.ic_queue) { playLast(collection) })
            addAll(extra)
        }
        overlays.showActions(title, subtitle, actions)
    }

    /** The playlist picker: the person's playlists and a new one. */
    fun addToPlaylist(tracks: List<TrackRef>) {
        overlays.showSheet(s(R.string.playlist_add_title)) { dismiss ->
            AddToPlaylistContent(
                onPick = { id, name ->
                    dismiss()
                    graph.send(Command.AddToPlaylist(id, tracks))
                    overlays.toast(s(R.string.toast_added_to_playlist, name))
                },
                onNew = {
                    dismiss()
                    overlays.prompt(s(R.string.playlist_new_title), s(R.string.playlist_name), "", s(R.string.action_create)) { name ->
                        graph.send(Command.CreatePlaylist(name, tracks))
                    }
                },
            )
        }
    }
}

@Composable
private fun AddToPlaylistContent(onPick: (Long, String) -> Unit, onNew: () -> Unit) {
    val graph = LocalGraph.current
    val revision = graph.store.state.value.libraryRevision
    val playlists by rememberQuery(revision) { it.playlists() }
    Column {
        ActionRow(stringResource(R.string.playlist_new_with_songs), R.drawable.ic_add, onClick = onNew)
        when (val p = playlists) {
            QueryResult.Loading -> LoadingRow()
            is QueryResult.Ok -> p.value.forEach { playlist ->
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
