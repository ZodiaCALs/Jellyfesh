package org.jellyfesh.logic.queue

import kotlin.math.roundToInt
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot

/**
 * The Up Next list while the person drags a row. The core owns the order; this only shows the
 * drop locally until the next Queue slice arrives, so the row does not jump back meanwhile.
 */
data class QueueDrag(
    /** The uid of the dragged row. */
    val uid: Long,
    /** Its index in the list when the drag began. */
    val from: Int,
    /** How far it has moved, in pixels (positive is down). */
    val offsetPx: Float,
)

/** The Up Next list: the slice's entries, with a local reorder shown until the core confirms it. */
data class UpNextState(
    val items: List<QueueItem>,
    /** The queue version the local order is based on; a newer slice replaces it. */
    val baseVersion: Long,
    val drag: QueueDrag? = null,
)

object QueueModel {
    /** The Up Next list for a slice, keeping a pending local order when the slice is not newer. */
    fun upNext(previous: UpNextState?, queue: QueueSnapshot): UpNextState =
        if (previous != null && queue.version <= previous.baseVersion && previous.drag == null) {
            previous
        } else {
            UpNextState(queue.upNext, queue.version, previous?.drag?.takeIf { d -> queue.upNext.any { it.uid == d.uid } })
        }

    /** The index a dragged row lands on: whole rows moved, clamped to the list. */
    fun target(from: Int, offsetPx: Float, rowPx: Float, count: Int): Int {
        if (count <= 0 || rowPx <= 0f) return from
        return (from + (offsetPx / rowPx).roundToInt()).coerceIn(0, count - 1)
    }

    /** [items] with the entry at [from] moved to [to]. */
    fun <T> moved(items: List<T>, from: Int, to: Int): List<T> {
        if (from == to || from !in items.indices || to !in items.indices) return items
        val out = items.toMutableList()
        val item = out.removeAt(from)
        out.add(to, item)
        return out
    }

    /**
     * The playback-order index of Up Next row [index]: the rows follow the current entry (or
     * start the queue when nothing is current).
     */
    fun playbackIndex(queuePosition: Int?, index: Int): Int = (queuePosition?.plus(1) ?: 0) + index

    /**
     * Ends a drag: the new local list and the command that makes the move real, or null when the
     * row did not move.
     */
    fun drop(state: UpNextState, queuePosition: Int?, rowPx: Float): Pair<UpNextState, Command?> {
        val drag = state.drag ?: return state to null
        val to = target(drag.from, drag.offsetPx, rowPx, state.items.size)
        val settled = state.copy(drag = null)
        if (to == drag.from) return settled to null
        val items = moved(state.items, drag.from, to)
        return settled.copy(items = items) to Command.MoveInQueue(drag.uid, playbackIndex(queuePosition, to))
    }
}
