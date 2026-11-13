package org.jellyfesh.logic.time

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jellyfesh.logic.LinearClock
import org.jellyfesh.logic.MS
import org.jellyfesh.logic.SpanTimeline
import org.jellyfesh.logic.anchor

class FrameSchedulerTest {
    private fun next(clock: LinearClock, nowMs: Long, vararg demands: TimeDemand, durationUs: Long? = 240_000_000): Wake {
        val now = nowMs * MS
        return FrameScheduler.next(clock, clock.positionUs(now), now, durationUs, demands.toList())
    }

    @Test
    fun pausedOrStoppedNeverWakes() {
        val clock = LinearClock()
        assertEquals(Wake.Idle, next(clock, 0, TimeDemand.Seconds))
        clock.update(anchor(1_250, 0, rate = 0f), 0)
        assertEquals(Wake.Idle, next(clock, 5_000, TimeDemand.Seconds, TimeDemand.Steps(300)))
    }

    @Test
    fun nothingVisibleNeverWakes() {
        val clock = LinearClock().apply { update(anchor(0, 0), 0) }
        assertEquals(Wake.Idle, next(clock, 100))
    }

    @Test
    fun secondsLabelWakesOnTheNextWholeSecond() {
        val clock = LinearClock().apply { update(anchor(1_250, 0), 0) }
        // 750 ms to 2.000 s, plus the 1 ms late margin.
        assertEquals(Wake.After(751 * MS), next(clock, 0, TimeDemand.Seconds))
    }

    @Test
    fun barWakesOnItsNextStep() {
        val clock = LinearClock().apply { update(anchor(0, 0), 0) }
        // 240 s over 480 px: a step every 500 ms; the label's second is later.
        assertEquals(Wake.After(501 * MS), next(clock, 0, TimeDemand.Seconds, TimeDemand.Steps(480)))
        // Without a duration the bar has no steps; only the label wakes.
        assertEquals(Wake.After(1_001 * MS), next(clock, 0, TimeDemand.Seconds, TimeDemand.Steps(480), durationUs = null))
    }

    @Test
    fun neverWakesSoonerThanTheMinimum() {
        val clock = LinearClock().apply { update(anchor(999, 0), 0) }
        assertEquals(Wake.After(FrameScheduler.MIN_WAIT_NS), next(clock, 0, TimeDemand.Seconds))
    }

    @Test
    fun lyricsWakeAtTheNextChangeWithoutASweep() {
        val timeline = SpanTimeline(listOf(SpanTimeline.Span(0, 0, 10_000, 10_400), SpanTimeline.Span(0, 1, 10_500, 11_000)))
        val clock = LinearClock().apply { update(anchor(9_000, 0), 0) }
        assertEquals(Wake.After(1_001 * MS), next(clock, 0, TimeDemand.Lyrics(timeline, sweep = false)))
    }

    @Test
    fun aSweepDrawsEveryFrameOnlyWhileAWordIsSung() {
        val timeline = SpanTimeline(listOf(SpanTimeline.Span(0, 0, 10_000, 10_400), SpanTimeline.Span(0, 1, 10_500, 11_000)))
        val clock = LinearClock().apply { update(anchor(10_000, 0), 0) }
        val sweep = TimeDemand.Lyrics(timeline, sweep = true)
        assertEquals(Wake.NextFrame, next(clock, 100, sweep))
        // Between words (10.4 s to 10.5 s) nothing moves: sleep until the next word.
        assertEquals(Wake.After(101 * MS), next(clock, 400, sweep))
        // Paused mid-word: nothing per frame.
        clock.update(anchor(10_200, 1_000, rate = 0f), 1_000 * MS)
        assertEquals(Wake.Idle, next(clock, 1_000, sweep))
    }

    @Test
    fun theEarliestDemandWins() {
        val timeline = SpanTimeline(listOf(SpanTimeline.Span(0, 0, 1_100, 1_900)))
        val clock = LinearClock().apply { update(anchor(1_000, 0), 0) }
        val wake = next(clock, 0, TimeDemand.Seconds, TimeDemand.Lyrics(timeline, sweep = false))
        assertEquals(Wake.After(101 * MS), wake)
    }

    @Test
    fun sweepingNeedsAWordStillFilling() {
        assertTrue(FrameScheduler.sweeping(LyricCursor(0, 0, 0.5f)))
        assertTrue(!FrameScheduler.sweeping(LyricCursor(0, 0, 1f)))
        assertTrue(!FrameScheduler.sweeping(LyricCursor(0, null, 0f)))
    }

    @Test
    fun demandsCountChanges() {
        val demands = TimeDemands()
        demands.set("bar", TimeDemand.Steps(100))
        demands.set("bar", TimeDemand.Steps(100))
        assertEquals(1, demands.version)
        demands.set("bar", TimeDemand.Steps(200))
        demands.set("label", TimeDemand.Seconds)
        demands.set("label", null)
        demands.set("missing", null)
        assertEquals(4, demands.version)
        assertEquals(listOf<TimeDemand>(TimeDemand.Steps(200)), demands.all.toList())
    }

    @Test
    fun anchorsExtrapolateLikeTheCore() {
        // core/jf-engine/src/anchor.rs tests: 1 s at 5 s plays on; sub-microseconds round half away.
        val a = org.jellyfesh.logic.core.PlaybackAnchor(1, 1_000_000, 5_000_000_000, 1f, null)
        assertEquals(1_250_000, a.positionAtUs(5_250_000_000))
        assertEquals(1_000_000, a.positionAtUs(5_000_000_499))
        assertEquals(1_000_001, a.positionAtUs(5_000_000_500))
        assertEquals(999_999, a.positionAtUs(4_999_999_500))
        assertEquals(1_000_000, a.copy(rate = 0f).positionAtUs(9_000_000_000))
    }

    @Test
    fun timingHelpersMatchTheCore() {
        assertEquals(2_000_000, nextSecondUs(1_250_000))
        assertEquals(3_000_000, nextSecondUs(2_000_000))
        assertEquals(0, nextSecondUs(-1))
        assertEquals(1_600_000, nextStepUs(1_250_000, 400_000))
        assertEquals(null, nextStepUs(1, 0))
    }
}
