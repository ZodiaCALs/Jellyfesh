package org.jellyfesh.logic.queue

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertSame
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.QueueItem
import org.jellyfesh.logic.core.QueueSnapshot
import org.jellyfesh.logic.core.TrackRef

class QueueModelTest {
    private val items = (1L..5L).map { QueueItem(it * 10, TrackRef.Library(it)) }
    private fun queue(version: Long, upNext: List<QueueItem> = items, position: Int? = 2) =
        QueueSnapshot.Empty.copy(version = version, len = 8, position = position, upNext = upNext)

    @Test
    fun dropTargetsWholeRows() {
        assertEquals(2, QueueModel.target(0, 110f, 52f, 5))
        assertEquals(0, QueueModel.target(3, -400f, 52f, 5))
        assertEquals(4, QueueModel.target(1, 9_999f, 52f, 5))
        assertEquals(1, QueueModel.target(1, 20f, 52f, 5))
    }

    @Test
    fun aDropMovesLocallyAndSendsThePlaybackIndex() {
        val state = UpNextState(items, baseVersion = 4, drag = QueueDrag(uid = 10, from = 0, offsetPx = 104f))
        val (dropped, command) = QueueModel.drop(state, queuePosition = 2, rowPx = 52f)
        assertEquals(listOf(20L, 30L, 10L, 40L, 50L), dropped.items.map { it.uid })
        // Row 2 of Up Next follows the current entry at playback index 2: index 5.
        assertEquals(Command.MoveInQueue(10, 5), command)
        assertNull(dropped.drag)
    }

    @Test
    fun aDropInPlaceSendsNothing() {
        val state = UpNextState(items, 4, QueueDrag(30, 2, 10f))
        val (_, command) = QueueModel.drop(state, 2, 52f)
        assertNull(command)
    }

    @Test
    fun withNothingCurrentUpNextStartsTheQueue() {
        assertEquals(3, QueueModel.playbackIndex(null, 3))
        assertEquals(6, QueueModel.playbackIndex(2, 3))
    }

    @Test
    fun theLocalOrderHoldsUntilANewerSlice() {
        val local = UpNextState(QueueModel.moved(items, 0, 2), baseVersion = 4)
        assertSame(local, QueueModel.upNext(local, queue(4)))
        val fresh = QueueModel.upNext(local, queue(5, items.reversed()))
        assertEquals(items.reversed(), fresh.items)
        assertEquals(5, fresh.baseVersion)
    }

    @Test
    fun movedLeavesBadIndicesAlone() {
        assertEquals(items, QueueModel.moved(items, 9, 1))
        assertEquals(items, QueueModel.moved(items, 1, 1))
    }

    @Test
    fun upNextListsEveryEntryAfterTheCurrentOnePastTheSlice() {
        // 120 entries, the 4th playing: 116 follow it; the slice carries the next 50.
        val slice = (1L..50L).map { QueueItem(it, TrackRef.Library(it)) }
        val q = QueueSnapshot.Empty.copy(version = 9, len = 120, position = 3, upNext = slice)
        assertEquals(116, UpNextTail.total(q))
        assertEquals(66, UpNextTail.count(q))
        // The first row past the slice is entry 4 + 50 in playback order.
        assertEquals(54, UpNextTail.windowOffset(q, 0))
        assertEquals(54 + 65, UpNextTail.windowOffset(q, 65))
        // Nothing current: Up Next is the whole queue.
        val idle = q.copy(position = null)
        assertEquals(120, UpNextTail.total(idle))
        assertEquals(50, UpNextTail.windowOffset(idle, 0))
        // A short queue has nothing past the slice.
        assertEquals(0, UpNextTail.count(queue(1)))
        assertEquals(0, UpNextTail.count(QueueSnapshot.Empty))
    }

    @Test
    fun aDropPastTheSliceSendsTheMoveAndWaitsForTheCore() {
        // Five slice rows and 20 rows past them: row 1 dragged down to row 9.
        val state = UpNextState(items, baseVersion = 4, drag = QueueDrag(uid = 20, from = 1, offsetPx = 8 * 52f))
        val (dropped, command) = QueueModel.drop(state, queuePosition = 2, rowPx = 52f, count = 25)
        assertEquals(Command.MoveInQueue(20, 12), command)
        assertEquals(items, dropped.items, "the slice's rows stay until the core's next slice")
        assertNull(dropped.drag)
        // A row past the slice dragged up into it.
        val up = UpNextState(items, 4, QueueDrag(uid = 900, from = 12, offsetPx = -10 * 52f))
        assertEquals(Command.MoveInQueue(900, 5), QueueModel.drop(up, 2, 52f, count = 25).second)
        // Without the rows past the slice, a drop stays within it, as before.
        assertEquals(Command.MoveInQueue(20, 7), QueueModel.drop(state, 2, 52f).second)
    }
}
