package org.jellyfesh.android.ui.library

import android.text.format.DateUtils
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.actions.TrackActions
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.android.ui.search.SourceItemRow
import org.jellyfesh.android.ui.search.runItemAction
import org.jellyfesh.android.ui.search.sourceItemMenu
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.SourceLibraryList
import org.jellyfesh.logic.core.SourceLibraryPart
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.library.SourceLibraryModel
import org.jellyfesh.logic.library.SourceLibraryPages
import org.jellyfesh.logic.library.SourceLibrarySection
import org.jellyfesh.logic.sources.SourceItems

/**
 * The sources' likes and playlists for Favourites and Playlists (ADR-0024; [SourceLibraryModel]):
 * asks the core to refresh them when the page opens and when it is returned to after ten minutes,
 * and keeps what Show More brings (SourceLibraryPage), or why it failed.
 */
@Composable
fun rememberSourceLibraryPages(): MutableState<SourceLibraryPages> {
    val graph = LocalGraph.current
    val context = LocalContext.current
    val pages = remember { mutableStateOf(SourceLibraryPages()) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        var last: Long? = null
        lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            val now = System.currentTimeMillis()
            if (SourceLibraryModel.refreshes(last, now)) {
                graph.send(Command.RefreshSourceLibrary(null))
                last = now
            }
        }
    }
    LaunchedEffect(Unit) {
        graph.store.signals.collect { event ->
            pages.value = when (event) {
                is CoreEvent.SourceLibraryPage -> SourceLibraryModel.onPage(pages.value, event.page)
                is CoreEvent.CommandFailed -> SourceLibraryModel.onCommandFailed(pages.value, event.id, describeFailure(context, event.error))
                else -> pages.value
            }
        }
    }
    return pages
}

/**
 * One section per source: its name, "Content from {Service}" when its terms ask for it, then each
 * list, named on Favourites ([titled]: Songs, Albums, Artists), with "Updated {time} ago" once
 * loaded, its rows (which play, browse or open in the service as anywhere else), Show More while
 * the source has more, and why the list or its last page failed. A source whose likes came back
 * empty says "Nothing liked in {Service} yet." instead ([SourceLibrarySection.nothingLiked]).
 * Read-only: the heart marks songs in Jellyfesh only.
 */
internal fun LazyListScope.sourceLibrarySections(
    sections: List<SourceLibrarySection>,
    pages: SourceLibraryPages,
    titled: Boolean,
    playing: TrackRef?,
    send: (Command) -> Long,
    push: (Screen) -> Unit,
    actions: TrackActions,
    onShowMore: (String, SourceLibraryList) -> Unit,
) {
    sections.forEach { section ->
        val source = section.source
        item(key = "srclib-${source.plugin}") {
            Column {
                SectionHeader(source.name)
                source.attribution?.takeIf { it.isNotBlank() }?.let {
                    JfText(stringResource(R.string.source_attribution, it), style = JfType.listSecondary, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp))
                }
            }
        }
        section.nothingLikedAtMs?.let { at ->
            item(key = "srclib-${source.plugin}-none") {
                Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                    JfText(stringResource(R.string.source_library_nothing_liked, source.name), style = JfType.small, color = Jf.colors.ink70)
                    JfText(updatedText(at), style = JfType.caption, color = Jf.colors.ink50)
                }
            }
        }
        section.lists.forEach { list ->
            val key = "srclib-${source.plugin}-${list.part}"
            val entries = SourceLibraryModel.entries(source.plugin, list, pages)
            val items = entries.map { it.item }
            item(key = "$key-h") { SourceListHeading(list, titled) }
            if (list.loading && entries.isEmpty()) item(key = "$key-loading") { LoadingRow() }
            list.error?.let { error -> item(key = "$key-e") { Notice(error, warning = true) } }
            items(entries, key = { "$key-${it.item.id}" }) { entry ->
                val action = SourceItems.mainAction(entry.item, items)
                SourceItemRow(entry.item, playing, action, onMore = sourceItemMenu(entry.item, actions)) {
                    runItemAction(action, send, push, actions::openLink)
                }
            }
            SourceLibraryModel.error(pages, source.plugin, list.part)?.let { error -> item(key = "$key-more-e") { Notice(error, warning = true) } }
            if (SourceLibraryModel.next(source.plugin, list, pages) != null) {
                item(key = "$key-more") {
                    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                        JfButton(
                            stringResource(R.string.search_show_more),
                            { onShowMore(source.plugin, list) },
                            style = ButtonStyle.Quiet,
                            enabled = !SourceLibraryModel.loadingMore(pages, source.plugin, list.part),
                        )
                    }
                }
            }
        }
    }
}

/** A list's name on Favourites, and "Updated {time} ago" once it has been loaded. */
@Composable
private fun SourceListHeading(list: SourceLibraryList, titled: Boolean) {
    val updated = list.refreshedAtMs?.let { updatedText(it) }
    if (!titled && updated == null) return
    Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
        if (titled) JfText(stringResource(partLabel(list.part)), style = JfType.label, color = Jf.colors.ink70)
        updated?.let { JfText(it, style = JfType.caption, color = Jf.colors.ink50) }
    }
}

/** "Updated 5 minutes ago": when a source's list came back. */
@Composable
private fun updatedText(atMs: Long): String =
    stringResource(R.string.source_library_updated, DateUtils.getRelativeTimeSpanString(atMs, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString())

private fun partLabel(part: SourceLibraryPart): Int = when (part) {
    SourceLibraryPart.LikedSongs -> R.string.kind_tracks
    SourceLibraryPart.LikedAlbums -> R.string.kind_albums
    SourceLibraryPart.LikedArtists -> R.string.kind_artists
    SourceLibraryPart.Playlists -> R.string.kind_playlists
}
