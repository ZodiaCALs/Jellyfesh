package org.jellyfesh.logic.seek

/**
 * The seek bar's own state: while the person drags, the thumb follows the finger; after the drop
 * it holds the requested position until the core's anchor shows the seek (a seek starts a new
 * track generation) or [settleMs] pass, so the thumb never jumps back to the old position.
 */
data class SeekState(
    val dragging: Boolean = false,
    /** Where the finger is, 0 to 1. */
    val dragFraction: Float = 0f,
    /** The position sent with the last Seek, while waiting for the core to show it. */
    val pendingMs: Long? = null,
    val pendingSinceMs: Long = 0,
    /** The anchor generation when the seek was sent. */
    val pendingGeneration: Long = 0,
)

object SeekModel {
    const val SETTLE_MS: Long = 1_500

    fun start(fraction: Float): SeekState = SeekState(dragging = true, dragFraction = fraction.coerceIn(0f, 1f))

    fun move(state: SeekState, fraction: Float): SeekState =
        if (state.dragging) state.copy(dragFraction = fraction.coerceIn(0f, 1f)) else state

    /** Ends the drag; returns the new state and the position to seek to (null without a duration). */
    fun end(state: SeekState, durationMs: Long?, generation: Long, nowMs: Long): Pair<SeekState, Long?> {
        if (!state.dragging || durationMs == null || durationMs <= 0) return SeekState() to null
        val target = (state.dragFraction * durationMs).toLong().coerceIn(0, durationMs)
        return SeekState(pendingMs = target, pendingSinceMs = nowMs, pendingGeneration = generation) to target
    }

    /** Cancels a drag (the gesture was taken over by a scroll, for example). */
    fun cancel(): SeekState = SeekState()

    /**
     * The fraction to draw: the finger while dragging, the pending seek until the core shows it,
     * else the playback position.
     */
    fun displayFraction(
        state: SeekState,
        positionMs: Long,
        durationMs: Long?,
        generation: Long,
        nowMs: Long,
    ): Float {
        if (state.dragging) return state.dragFraction
        val duration = durationMs?.takeIf { it > 0 } ?: return 0f
        val pending = state.pendingMs
        val shown = if (pending != null && generation == state.pendingGeneration && nowMs - state.pendingSinceMs < SETTLE_MS) {
            pending
        } else {
            positionMs
        }
        return (shown.toFloat() / duration).coerceIn(0f, 1f)
    }

    /** Clears a pending seek once the core has shown it. */
    fun settle(state: SeekState, generation: Long, nowMs: Long): SeekState =
        if (state.pendingMs != null && (generation != state.pendingGeneration || nowMs - state.pendingSinceMs >= SETTLE_MS)) {
            state.copy(pendingMs = null)
        } else {
            state
        }
}
