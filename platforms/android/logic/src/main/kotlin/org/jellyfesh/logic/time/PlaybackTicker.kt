package org.jellyfesh.logic.time

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import org.jellyfesh.logic.core.PlaybackAnchor

/**
 * Publishes the playback position for drawing, only as often as something visible changes.
 *
 * Feed it every anchor ([onAnchor]) and run [run] while the UI is visible (the app runs it in
 * `repeatOnLifecycle(STARTED)`, so nothing ticks with the screen off: Still mode). Views read
 * [positionUs] in their draw phase and register what they draw in [demands]; between the
 * scheduled moments the loop sleeps, and while paused it waits for the next anchor.
 *
 * Confined to one thread (the main thread in the app): the ported `DisplayClock` is not
 * thread-safe.
 *
 * @param now the core's monotonic clock in nanoseconds (see [MonotonicBridge]).
 * @param awaitFrame suspends until the next display frame (`withFrameNanos` in Compose).
 */
class PlaybackTicker(
    private val clock: DrawClock,
    private val now: () -> Long,
    private val awaitFrame: suspend () -> Unit,
) {
    val demands = TimeDemands()

    private val _positionUs = MutableStateFlow(0L)

    /** The position to draw, in microseconds; changes only when something visible would change. */
    val positionUs: StateFlow<Long> = _positionUs.asStateFlow()

    private var durationUs: Long? = null

    /** Bumped on every anchor and demand change, so a sleeping loop re-plans. */
    private val changes = MutableStateFlow(0L)

    /** True while the position moves (playing with a running output). */
    val isRunning: Boolean get() = clock.isRunning

    /** Feeds an anchor from a Playback slice. */
    fun onAnchor(anchor: PlaybackAnchor) {
        clock.update(anchor, now())
        durationUs = anchor.durationUs
        _positionUs.value = clock.positionUs(now())
        changes.value++
    }

    /** Registers what a view draws; null removes it. */
    fun setDemand(key: Any, demand: TimeDemand?) {
        val before = demands.version
        demands.set(key, demand)
        if (demands.version != before) changes.value++
    }

    /** Draws the current position once (after a seek bar drag, for example). */
    fun refresh() {
        _positionUs.value = clock.positionUs(now())
    }

    /** The scheduling loop; returns only by cancellation. */
    suspend fun run() {
        while (true) {
            val seen = changes.value
            val nowNs = now()
            val position = clock.positionUs(nowNs)
            _positionUs.value = position
            when (val wake = FrameScheduler.next(clock, position, nowNs, durationUs, demands.all)) {
                Wake.Idle -> changes.first { it != seen }
                is Wake.After -> withTimeoutOrNull(nanosToMillisCeil(wake.ns)) { changes.first { it != seen } }
                Wake.NextFrame -> awaitFrame()
            }
        }
    }

    private fun nanosToMillisCeil(ns: Long): Long = (ns + 999_999) / 1_000_000
}

/**
 * The core stamps anchors with its own monotonic clock (`monotonicNowNs()`); the display runs on
 * `System.nanoTime()` (frame times included). Both are CLOCK_MONOTONIC on Android, so they differ
 * by a constant: this measures it once and converts, instead of calling the core every frame.
 */
class MonotonicBridge(
    private val coreNow: () -> Long,
    private val platformNow: () -> Long,
    samples: Int = 5,
) {
    /** `core - platform`, from the sample with the tightest bracket. */
    val offsetNs: Long

    init {
        var best = Long.MAX_VALUE
        var offset = 0L
        repeat(samples.coerceAtLeast(1)) {
            val before = platformNow()
            val core = coreNow()
            val after = platformNow()
            val bracket = after - before
            if (bracket < best) {
                best = bracket
                offset = core - (before + bracket / 2)
            }
        }
        offsetNs = offset
    }

    /** Now, on the core's clock. */
    fun nowNs(): Long = platformNow() + offsetNs

    /** A platform timestamp (a frame time) on the core's clock. */
    fun toCore(platformNs: Long): Long = platformNs + offsetNs
}
