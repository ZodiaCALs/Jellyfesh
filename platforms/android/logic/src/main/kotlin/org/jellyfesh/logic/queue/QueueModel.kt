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
     * Ends a drag over Up Next's [count] rows (the slice's entries, then the rows read past them):
     * the new local list and the command that makes the move real, or null when the row did not
     * move. A move within the slice's entries shows at once; one that reaches past them shows when
     * the core's next Queue slice arrives.
     */
    fun drop(state: UpNextState, queuePosition: Int?, rowPx: Float, count: Int = state.items.size): Pair<UpNextState, Command?> {
        val drag = state.drag ?: return state to null
        val to = target(drag.from, drag.offsetPx, rowPx, maxOf(count, state.items.size))
        val settled = state.copy(drag = null)
        if (to == drag.from) return settled to null
        val command = Command.MoveInQueue(drag.uid, playbackIndex(queuePosition, to))
        val local = drag.from < state.items.size && to < state.items.size
        return (if (local) settled.copy(items = moved(state.items, drag.from, to)) else settled) to command
    }
}

/**
 * Up Next past the Queue slice's entries: the slice holds the next 50 and `queue_window` pages the
 * rest (docs/core-api.md), so every entry after the current one is listed, as in the other apps.
 */
object UpNextTail {
    /** Entries the core reads per page past the slice. */
    const val PAGE = 100

    /** How many entries follow the current one (every entry while none is current): Up Next's rows. */
    fun total(queue: QueueSnapshot): Int = (queue.len - (queue.position?.plus(1) ?: 0)).coerceAtLeast(0)

    /** How many of those rows lie past the slice's entries. */
    fun count(queue: QueueSnapshot): Int = (total(queue) - queue.upNext.size).coerceAtLeast(0)

    /** The `queue_window` offset (playback order from the queue's start) of row [index] past the slice. */
    fun windowOffset(queue: QueueSnapshot, index: Int): Int = QueueModel.playbackIndex(queue.position, queue.upNext.size + index)
}
