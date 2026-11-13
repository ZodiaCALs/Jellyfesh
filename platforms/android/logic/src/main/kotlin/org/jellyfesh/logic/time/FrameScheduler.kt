package org.jellyfesh.logic.time

/**
 * Something on screen that changes with playback time. Views register one while they are visible
 * (see [TimeDemands]); the scheduler wakes only for the earliest change among them.
 */
sealed interface TimeDemand {
    /** A clock label (`m:ss`): changes on every whole second. */
    data object Seconds : TimeDemand

    /** A bar or ring drawn in [steps] steps across the track (its length in pixels). */
    data class Steps(val steps: Int) : TimeDemand

    /**
     * Lyrics: the active line or word changes at the timeline's next change. With [sweep] the
     * view also fills the current word as it is sung, which moves every frame while a word lasts.
     */
    data class Lyrics(val timeline: LyricTimelineLookup, val sweep: Boolean) : TimeDemand
}

/** When to draw next. */
sealed interface Wake {
    /** Nothing moves (paused, stopped, or nothing time-driven visible): wait for a change. */
    data object Idle : Wake

    /** Draw again after [ns] nanoseconds. */
    data class After(val ns: Long) : Wake

    /** Draw on the next frame: a word sweep is moving. */
    data object NextFrame : Wake
}

/**
 * Turns the visible time-driven views into the next wake-up (architecture.md, "Drawing time
 * without waste"): the next second for labels, the next step for bars, the lyric timeline's next
 * change, and per-frame drawing only while a word sweep is in progress. This is what `jf play`
 * does; the app runs it on the main thread with the ported `DisplayClock`.
 */
object FrameScheduler {
    /** Never wake sooner than this, even when a change is due now. */
    const val MIN_WAIT_NS: Long = 2_000_000

    /** Wake this much after a change is due, so the draw lands past it. */
    const val WAKE_LATE_NS: Long = 1_000_000

    /**
     * @param positionUs the position just drawn ([DrawClock.positionUs] at [nowNs]).
     * @param durationUs the track's duration, for bars; null when unknown.
     */
    fun next(
        clock: DrawClock,
        positionUs: Long,
        nowNs: Long,
        durationUs: Long?,
        demands: Iterable<TimeDemand>,
    ): Wake {
        if (!clock.isRunning) return Wake.Idle
        var target: Long? = null
        fun consider(t: Long?) {
            if (t != null && (target == null || t < target!!)) target = t
        }
        for (demand in demands) {
            when (demand) {
                TimeDemand.Seconds -> consider(nextSecondUs(positionUs))
                is TimeDemand.Steps -> {
                    val duration = durationUs ?: continue
                    if (duration <= 0 || demand.steps <= 0) continue
                    consider(nextStepUs(positionUs, maxOf(1L, duration / demand.steps)))
                }
                is TimeDemand.Lyrics -> {
                    if (demand.timeline.isEmpty) continue
                    val tMs = Math.floorDiv(positionUs, 1_000L)
                    if (demand.sweep && sweeping(demand.timeline.cursor(tMs))) return Wake.NextFrame
                    demand.timeline.nextChange(tMs)?.let { consider(it * 1_000L) }
                }
            }
        }
        val due = target ?: return Wake.Idle
        val wait = clock.waitUntilNs(due, nowNs) ?: return Wake.Idle
        return Wake.After(maxOf(wait + WAKE_LATE_NS, MIN_WAIT_NS))
    }

    /** A word is being sung and its fill is still moving. */
    fun sweeping(cursor: LyricCursor): Boolean = cursor.word != null && cursor.wordProgress < 1f
}

/**
 * The time-driven views on screen now, keyed by whoever registered them. Main thread only; bump
 * [version] observers re-plan when it changes.
 */
class TimeDemands {
    private val entries = LinkedHashMap<Any, TimeDemand>()

    /** Changes whenever a demand is added, replaced or removed. */
    var version: Long = 0
        private set

    /** Registers (or replaces) [key]'s demand; null removes it. */
    fun set(key: Any, demand: TimeDemand?) {
        val changed = if (demand == null) entries.remove(key) != null else entries.put(key, demand) != demand
        if (changed) version++
    }

    val all: Collection<TimeDemand> get() = entries.values

    val isEmpty: Boolean get() = entries.isEmpty()
}
