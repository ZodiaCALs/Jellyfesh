package org.jellyfesh.android.ui.library

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.JfButton

/**
 * How music gets into the library on Android (option C of the Step 0 index, implemented here
 * while the choice is open): access to music lets MediaStore name the folders that hold songs at
 * once, and any folder can be added with the system's folder picker.
 */
@Stable
class LibraryAccessUi internal constructor(
    val requestMusic: () -> Unit,
    val addFolder: () -> Unit,
    private val permission: State<Boolean>,
) {
    val hasMusicPermission: Boolean get() = permission.value
}

@Composable
fun rememberLibraryAccess(): LibraryAccessUi {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val hasPermission = remember { mutableStateOf(graph.library.hasMediaPermission()) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        hasPermission.value = granted
        if (granted) {
            scope.launch {
                val added = graph.library.addMediaStoreRoots()
                overlays.toast(
                    if (added > 0) {
                        context.resources.getQuantityString(R.plurals.toast_folders_added, added, added)
                    } else {
                        context.getString(R.string.toast_no_new_folders)
                    },
                )
            }
        }
    }
    val folder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val path = graph.library.addTree(uri)
        overlays.toast(
            when {
                path == null -> context.getString(R.string.folder_unreadable)
                !graph.library.hasMediaPermission() -> context.getString(R.string.folder_needs_permission)
                else -> context.getString(R.string.folder_added)
            },
        )
    }
    return remember(permission, folder) {
        LibraryAccessUi(
            requestMusic = { permission.launch(graph.library.mediaPermission) },
            addFolder = { folder.launch(null) },
            permission = hasPermission,
        )
    }
}

/** The two ways in: access to music (when not granted yet) and a folder of the person's choice. */
@Composable
fun LibraryAccessButtons(modifier: Modifier = Modifier) {
    val access = rememberLibraryAccess()
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (!access.hasMusicPermission) {
            JfButton(stringResource(R.string.action_allow_music), access.requestMusic, style = ButtonStyle.Accent, icon = R.drawable.ic_library)
        }
        JfButton(stringResource(R.string.action_add_folder), access.addFolder, icon = R.drawable.ic_folder)
    }
}

/** For screens that lay the two buttons out in a row. */
@Composable
fun LibraryAccessRow(modifier: Modifier = Modifier) {
    val access = rememberLibraryAccess()
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (!access.hasMusicPermission) {
            JfButton(stringResource(R.string.action_allow_music), access.requestMusic, style = ButtonStyle.Accent, modifier = Modifier.weight(1f))
        }
        JfButton(stringResource(R.string.action_add_folder), access.addFolder, modifier = Modifier.weight(1f))
    }
}
