package org.jellyfesh.logic.seek

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class SeekModelTest {
    @Test
    fun theThumbFollowsTheFingerThenHoldsTheSeek() {
        var state = SeekModel.start(0.2f)
        state = SeekModel.move(state, 0.5f)
        assertEquals(0.5f, SeekModel.displayFraction(state, positionMs = 10_000, durationMs = 200_000, generation = 1, nowMs = 0))
        val (after, target) = SeekModel.end(state, durationMs = 200_000, generation = 1, nowMs = 1_000)
        assertEquals(100_000L, target)
        // Until the core's anchor shows the seek (a new generation), the thumb stays at 50 %.
        assertEquals(0.5f, SeekModel.displayFraction(after, positionMs = 11_000, durationMs = 200_000, generation = 1, nowMs = 1_200))
        assertEquals(0.505f, SeekModel.displayFraction(after, positionMs = 101_000, durationMs = 200_000, generation = 2, nowMs = 1_300))
        // ...or for at most the settle time.
        assertEquals(0.06f, SeekModel.displayFraction(after, positionMs = 12_000, durationMs = 200_000, generation = 1, nowMs = 1_000 + SeekModel.SETTLE_MS))
    }

    @Test
    fun settleClearsThePendingSeek() {
        val (state, _) = SeekModel.end(SeekModel.start(0.1f), 100_000, 3, 0)
        assertEquals(10_000L, state.pendingMs)
        assertEquals(10_000L, SeekModel.settle(state, generation = 3, nowMs = 10).pendingMs)
        assertNull(SeekModel.settle(state, generation = 4, nowMs = 10).pendingMs)
    }

    @Test
    fun withoutADurationNothingIsSent() {
        val (state, target) = SeekModel.end(SeekModel.start(0.5f), durationMs = null, generation = 1, nowMs = 0)
        assertNull(target)
        assertEquals(SeekState(), state)
        assertEquals(0f, SeekModel.displayFraction(state, 5_000, null, 1, 0))
    }

    @Test
    fun fractionsAreClamped() {
        assertEquals(1f, SeekModel.start(1.7f).dragFraction)
        assertEquals(0f, SeekModel.move(SeekModel.start(0.5f), -3f).dragFraction)
        assertEquals(1f, SeekModel.displayFraction(SeekState(), 500_000, 100_000, 1, 0))
    }
}
