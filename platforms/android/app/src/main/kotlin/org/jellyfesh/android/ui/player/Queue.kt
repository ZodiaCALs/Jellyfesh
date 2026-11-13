package org.jellyfesh.android.ui.player

import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SheetAction
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.queue.QueueDrag
import org.jellyfesh.logic.queue.QueueModel
import org.jellyfesh.logic.queue.UpNextState
import org.jellyfesh.tokens.Tokens

private val ROW_HEIGHT = 64.dp

/**
 * Up Next: the next entries in playback order (the slice's window of 50). Drag a row by its handle
 * to move it; the drop shows at once and becomes `MoveInQueue`, and the next Queue slice confirms
 * it (QueueModel). TalkBack gets "Move up" and "Move down" actions on each row instead.
 */
@Composable
fun QueueView(queue: QueueSnapshot, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    var upNext by remember { mutableStateOf<UpNextState?>(null) }
    LaunchedEffect(queue) { upNext = QueueModel.upNext(upNext, queue) }
    val state = upNext ?: QueueModel.upNext(null, queue)
    val rowPx = with(LocalDensity.current) { ROW_HEIGHT.toPx() }
    val clearTitle = stringResource(R.string.queue_clear_title)
    val clearBody = stringResource(R.string.queue_clear_body)
    val clearLabel = stringResource(R.string.action_clear)
    val removeLabel = stringResource(R.string.action_remove)
    val moveUp = stringResource(R.string.a11y_move_up)
    val moveDown = stringResource(R.string.a11y_move_down)
    val remaining = (queue.len - ((queue.position ?: -1) + 1) - state.items.size).coerceAtLeast(0)

    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item(key = "header") {
            SectionHeader(stringResource(R.string.queue_up_next)) {
                if (queue.len > 0) {
                    JfButton(stringResource(R.string.queue_clear), {
                        overlays.confirm(clearTitle, clearBody, clearLabel, destructive = true) { graph.send(Command.ClearQueue) }
                    }, style = ButtonStyle.Quiet)
                }
            }
        }
        if (state.items.isEmpty()) {
            item(key = "empty") { EmptyState(R.drawable.ic_queue, stringResource(R.string.queue_empty_title), stringResource(R.string.queue_empty_body)) }
        }
        val drag = state.drag
        val target = drag?.let { QueueModel.target(it.from, it.offsetPx, rowPx, state.items.size) }
        itemsIndexed(state.items, key = { _, item -> item.uid }) { index, item ->
            val shift = when {
                drag == null || target == null -> 0f
                index == drag.from -> drag.offsetPx
                drag.from < target && index in (drag.from + 1)..target -> -rowPx
                drag.from > target && index in target until drag.from -> rowPx
                else -> 0f
            }
            QueueRow(
                item = item,
                dragging = drag?.uid == item.uid,
                modifier = Modifier
                    .zIndex(if (drag?.uid == item.uid) 1f else 0f)
                    .graphicsLayer { translationY = shift }
                    .semantics {
                        customActions = buildList {
                            if (index > 0) {
                                add(CustomAccessibilityAction(moveUp) {
                                    graph.send(Command.MoveInQueue(item.uid, QueueModel.playbackIndex(queue.position, index - 1)))
                                    true
                                })
                            }
                            if (index < state.items.size - 1) {
                                add(CustomAccessibilityAction(moveDown) {
                                    graph.send(Command.MoveInQueue(item.uid, QueueModel.playbackIndex(queue.position, index + 1)))
                                    true
                                })
                            }
                        }
                    },
                onClick = { graph.send(Command.JumpTo(item.uid)) },
                onMore = { title ->
                    overlays.showActions(title, null, listOf(SheetAction(removeLabel, R.drawable.ic_close, destructive = true) { graph.send(Command.RemoveFromQueue(item.uid)) }))
                },
                onDragStart = { upNext = state.copy(drag = QueueDrag(item.uid, index, 0f)) },
                onDrag = { dy -> upNext = upNext?.let { s -> s.drag?.let { d -> s.copy(drag = d.copy(offsetPx = d.offsetPx + dy)) } ?: s } },
                onDragEnd = {
                    val current = upNext ?: return@QueueRow
                    val (settled, command) = QueueModel.drop(current, queue.position, rowPx)
                    upNext = settled
                    command?.let(graph::send)
                },
            )
        }
        if (remaining > 0) {
            item(key = "more") {
                JfText(
                    pluralStringResource(R.plurals.queue_more, remaining, remaining),
                    style = JfType.caption,
                    color = Jf.colors.ink50,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                )
            }
        }
    }
}

@Composable
private fun QueueRow(
    item: QueueItem,
    dragging: Boolean,
    modifier: Modifier,
    onClick: () -> Unit,
    onMore: (String) -> Unit,
    onDragStart: () -> Unit,
    onDrag: (Float) -> Unit,
    onDragEnd: () -> Unit,
) {
    val colors = Jf.colors
    val (title, subtitle, art) = rowText(item.track)
    val dragLabel = stringResource(R.string.a11y_drag)
    Row(
        modifier
            .fillMaxWidth()
            .height(ROW_HEIGHT)
            .then(if (dragging) Modifier.graphicsLayer { shadowElevation = 12f; shape = RoundedCornerShape(12.dp); clip = true } else Modifier)
            .clickable(onClick = onClick)
            .padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ArtworkImage(art, title, 44.dp, RoundedCornerShape(Tokens.Artwork.radiusList.dp))
        Column(Modifier.weight(1f)) {
            JfText(title, style = JfType.listTitle, maxLines = 1)
            if (subtitle != null) JfText(subtitle, style = JfType.listSecondary, color = colors.ink70, maxLines = 1)
        }
        JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), { onMore(title) }, size = 40.dp, iconSize = 20.dp)
        Box(
            Modifier
                .size(48.dp)
                .semantics { contentDescription = dragLabel }
                .pointerInput(item.uid) {
                    detectVerticalDragGestures(
                        onDragStart = { onDragStart() },
                        onDragEnd = onDragEnd,
                        onDragCancel = onDragEnd,
                        onVerticalDrag = { change, dy ->
                            change.consume()
                            onDrag(dy)
                        },
                    )
                },
            contentAlignment = Alignment.Center,
        ) {
            JfIcon(R.drawable.ic_drag, colors.ink50, size = 20.dp)
        }
    }
}

/** What a queue row shows: the library's tags for library tracks, else what the entry carries. */
@Composable
private fun rowText(track: TrackRef): Triple<String, String?, ArtKey?> {
    val unknown = stringResource(R.string.unknown_title)
    return when (track) {
        is TrackRef.Library -> {
            val result by rememberQuery(track.id) { it.track(track.id) }
            val t = (result as? QueryResult.Ok)?.value
            Triple(t?.title ?: unknown, t?.let { Labels.artistAlbum(it.artist, it.album) }, ArtKey.ofHash(t?.artworkHash))
        }
        is TrackRef.File -> Triple(Labels.provisionalTitle(track) ?: unknown, null, null)
        is TrackRef.Source -> Triple(track.track.title, Labels.artistAlbum(track.track.artist, track.track.album), null)
    }
}
