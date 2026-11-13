package org.jellyfesh.logic.time

import org.jellyfesh.logic.core.PlaybackAnchor

// The drawing-time helpers the shell uses, as interfaces. The implementations are the shared
// Kotlin ports in platforms/shared/kotlin (DisplayClock, LyricTimeline.cursor and nextChange),
// wrapped by the app's FFI adapter; tests use small fakes. Views never ask the core per frame.

/** Position of the playhead in the lyrics (the port's `LyricCursor`). */
data class LyricCursor(val line: Int?, val word: Int?, val wordProgress: Float) {
    companion object {
        val None = LyricCursor(null, null, 0f)
    }
}

/** The ported `LyricTimeline.cursor` and `nextChange` over one track's timeline. */
interface LyricTimelineLookup {
    /** True when the timeline has no timed lines (unsynced lyrics). */
    val isEmpty: Boolean

    /** The cursor at [tMs]: two binary searches, cheap enough for every frame. */
    fun cursor(tMs: Long): LyricCursor

    /**
     * The earliest time after [tMs] at which the line or word changes or the current word
     * finishes, in milliseconds; null when nothing changes after [tMs].
     */
    fun nextChange(tMs: Long): Long?
}

/**
 * The ported `DisplayClock`: a smoothed playback position that never steps backwards while
 * playing. Not thread-safe; the app uses it from the main thread only.
 */
interface DrawClock {
    /** Feeds an anchor from the playback slice, at [nowNs] on the core's monotonic clock. */
    fun update(anchor: PlaybackAnchor, nowNs: Long)

    /** The position to draw at [nowNs], in microseconds within the track. */
    fun positionUs(nowNs: Long): Long

    /** True while the position moves. */
    val isRunning: Boolean

    /**
     * Nanoseconds until the drawn position reaches [targetUs]: zero when it already has, null
     * while stopped or paused.
     */
    fun waitUntilNs(targetUs: Long, nowNs: Long): Long?
}

/**
 * The raw extrapolated position at [nowNs] (`PlaybackAnchor::position_at`), for consumers that
 * want the audible position without the drawing clock's smoothing: the system media session.
 * Views draw with [DrawClock] instead.
 */
fun PlaybackAnchor.positionAtUs(nowNs: Long): Long {
    if (rate == 0f) return positionUs
    val advance = (nowNs - anchorMonoNs).toDouble() * rate / 1_000.0
    val rounded = if (advance >= 0) Math.floor(advance + 0.5) else -Math.floor(-advance + 0.5)
    return positionUs + rounded.toLong()
}

/** The next whole second after [positionUs] (`timing::next_second_us`), when `m:ss` changes. */
fun nextSecondUs(positionUs: Long): Long = (Math.floorDiv(positionUs, 1_000_000L) + 1) * 1_000_000L

/** The next multiple of [stepUs] after [positionUs] (`timing::next_step_us`); null for no step. */
fun nextStepUs(positionUs: Long, stepUs: Long): Long? =
    if (stepUs > 0) (Math.floorDiv(positionUs, stepUs) + 1) * stepUs else null
