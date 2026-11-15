package org.jellyfesh.android.ui.library

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.launch
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.components.SheetAction
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.playlist.PlaylistFiles

/**
 * Import Playlist… and Export Playlist… (M3U8) through the system's document picker: a picked
 * file goes to the core's `ImportPlaylist`, and `export_playlist` writes the file the person
 * created (docs/ui.md, "Playlists"; the copying is in [org.jellyfesh.android.library.LibraryAccess]).
 */
@Stable
class PlaylistTransfer internal constructor(
    /** Opens the picker for an M3U or M3U8 file to import. */
    val import: () -> Unit,
    /** Opens the picker to create the M3U8 file for playlist `id`, named after it. */
    val export: (id: Long, name: String) -> Unit,
)

@Composable
fun rememberPlaylistTransfer(): PlaylistTransfer {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // The playlist being exported while the picker is open (kept if the activity is recreated).
    var exportingId by rememberSaveable { mutableStateOf<Long?>(null) }
    var exportingName by rememberSaveable { mutableStateOf<String?>(null) }
    val importer = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val sent = graph.library.importPlaylist(uri)
            // The core then reports the new playlist (PlaylistCreated) or why it failed.
            overlays.toast(context.getString(if (sent) R.string.toast_playlist_importing else R.string.toast_playlist_import_failed))
        }
    }
    val exporter = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(PlaylistFiles.EXPORT_TYPE)) { uri ->
        val id = exportingId
        val name = exportingName.orEmpty()
        exportingId = null
        exportingName = null
        if (uri == null || id == null) return@rememberLauncherForActivityResult
        scope.launch {
            val failure = graph.library.exportPlaylist(id, uri)
            overlays.toast(
                if (failure == null) {
                    context.getString(R.string.toast_playlist_exported, name)
                } else {
                    context.getString(R.string.toast_playlist_export_failed, failure)
                },
            )
        }
    }
    return remember(importer, exporter) {
        PlaylistTransfer(
            import = { importer.launch(PlaylistFiles.IMPORT_TYPES.toTypedArray()) },
            export = { id, name ->
                exportingId = id
                exportingName = name
                exporter.launch(PlaylistFiles.exportName(name))
            },
        )
    }
}

/**
 * A playlist's own items in its More menu, after the ones every collection has: Rename…, Export
 * Playlist… and Delete Playlist…, which asks first (docs/ui.md, "Steps that ask first").
 */
@Stable
class PlaylistActions internal constructor(
    private val graph: AppGraph,
    private val overlays: Overlays,
    private val transfer: PlaylistTransfer,
    private val context: android.content.Context,
) {
    private fun s(id: Int, vararg args: Any): String = context.getString(id, *args)

    /** New Playlist…: asks for the name first, then creates it. */
    fun create() {
        overlays.prompt(s(R.string.playlist_new_title), s(R.string.playlist_name), "", s(R.string.action_create)) { name ->
            graph.send(Command.CreatePlaylist(name, emptyList()))
        }
    }

    /** [afterDelete] runs once the person confirmed (a playlist's page goes back). */
    fun items(id: Long, name: String, afterDelete: () -> Unit = {}): List<SheetAction> = listOf(
        SheetAction(s(R.string.action_rename_ask), R.drawable.ic_edit) {
            overlays.prompt(s(R.string.playlist_rename_title), s(R.string.playlist_name), name, s(R.string.action_rename)) {
                graph.send(Command.RenamePlaylist(id, it))
            }
        },
        SheetAction(s(R.string.action_export_playlist), R.drawable.ic_export) { transfer.export(id, name) },
        SheetAction(s(R.string.action_delete_playlist_ask), R.drawable.ic_trash, destructive = true) {
            overlays.confirm(s(R.string.playlist_delete_title, name), s(R.string.playlist_delete_body), s(R.string.action_delete_playlist), destructive = true) {
                graph.send(Command.DeletePlaylist(id))
                afterDelete()
            }
        },
    )
}

@Composable
fun rememberPlaylistActions(transfer: PlaylistTransfer = rememberPlaylistTransfer()): PlaylistActions {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    return remember(graph, overlays, transfer, context) { PlaylistActions(graph, overlays, transfer, context) }
}
