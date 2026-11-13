package org.jellyfesh.logic.time

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.jellyfesh.logic.LinearClock
import org.jellyfesh.logic.MS
import org.jellyfesh.logic.SpanTimeline
import org.jellyfesh.logic.anchor

@OptIn(ExperimentalCoroutinesApi::class)
class PlaybackTickerTest {
    /** A ticker on the test's virtual clock, counting the frames it asks for (16 ms each). */
    private class Rig(scope: TestScope) {
        var frames = 0
        val ticker = PlaybackTicker(
            clock = LinearClock(),
            now = { scope.testScheduler.currentTime * MS },
            awaitFrame = {
                frames++
                delay(16)
            },
        )
        val positions = mutableListOf<Long>()
        val job = scope.backgroundScope.launch { ticker.positionUs.collect { positions += it } }
        val loop = scope.backgroundScope.launch { ticker.run() }
    }

    @Test
    fun aPausedPlayerDoesNoWork() = runTest {
        val rig = Rig(this)
        rig.ticker.setDemand("label", TimeDemand.Seconds)
        rig.ticker.onAnchor(anchor(5_000, 0, rate = 0f))
        runCurrent()
        val before = rig.positions.size
        advanceTimeBy(60_000)
        assertEquals(before, rig.positions.size, "no updates while paused")
        assertEquals(0, rig.frames)
    }

    @Test
    fun aLabelUpdatesOncePerSecond() = runTest {
        val rig = Rig(this)
        rig.ticker.setDemand("label", TimeDemand.Seconds)
        rig.ticker.onAnchor(anchor(0, 0))
        runCurrent()
        advanceTimeBy(10_500)
        val whole = rig.positions.filter { it > 0 }.map { it / 1_000_000 }.distinct()
        assertEquals((1L..10L).toList(), whole)
        // About one update per second, not per frame.
        assertTrue(rig.positions.size in 10..13, "updates: ${rig.positions.size}")
        assertEquals(0, rig.frames)
    }

    @Test
    fun aSweepRunsPerFrameOnlyWhileAWordLasts() = runTest {
        val rig = Rig(this)
        val timeline = SpanTimeline(listOf(SpanTimeline.Span(0, 0, 1_000, 1_500), SpanTimeline.Span(0, 1, 3_000, 3_200)))
        rig.ticker.setDemand("lyrics", TimeDemand.Lyrics(timeline, sweep = true))
        rig.ticker.onAnchor(anchor(0, 0))
        runCurrent()
        advanceTimeBy(5_000)
        // 500 ms + 200 ms of sweeping at 16 ms a frame, about 44 frames; nothing in between.
        assertTrue(rig.frames in 40..48, "frames: ${rig.frames}")
    }

    @Test
    fun aNewDemandWakesASleepingLoop() = runTest {
        val rig = Rig(this)
        rig.ticker.onAnchor(anchor(0, 0))
        runCurrent()
        advanceTimeBy(3_000)
        val idle = rig.positions.size
        rig.ticker.setDemand("label", TimeDemand.Seconds)
        runCurrent()
        assertTrue(rig.positions.size > idle, "re-planned on the new demand")
        advanceTimeBy(2_100)
        assertTrue(rig.positions.last() >= 5_000_000)
    }

    @Test
    fun aSeekShowsAtOnce() = runTest {
        val rig = Rig(this)
        rig.ticker.setDemand("label", TimeDemand.Seconds)
        rig.ticker.onAnchor(anchor(10_000, 0))
        runCurrent()
        advanceTimeBy(500)
        rig.ticker.onAnchor(anchor(90_000, 500, generation = 2))
        assertEquals(90_000_000, rig.ticker.positionUs.value)
    }
}

class MonotonicBridgeTest {
    @Test
    fun measuresTheOffsetOnce() {
        var platform = 1_000L
        var calls = 0
        val bridge = MonotonicBridge(
            coreNow = {
                calls++
                platform + 50_000L
            },
            platformNow = { platform.also { platform += 10 } },
        )
        assertEquals(5, calls)
        // Each sample reads the platform clock at p and p + 10 and the core at p + 10 + 50 000:
        // the offset is the core time minus the midpoint, 50 005.
        assertEquals(50_005L, bridge.offsetNs)
        val now = bridge.nowNs()
        assertEquals(platform - 10 + bridge.offsetNs, now)
        assertEquals(123L + bridge.offsetNs, bridge.toCore(123L))
    }
}
