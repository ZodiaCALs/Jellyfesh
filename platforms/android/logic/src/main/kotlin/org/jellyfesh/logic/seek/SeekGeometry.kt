package org.jellyfesh.logic.seek

import kotlin.math.max
import kotlin.math.roundToInt

/** What the person is doing with the seek bar (or the volume line in its style). */
enum class SeekBarState {
    /** Nothing: a thin line, no thumb. */
    Rest,

    /** Hovered by a mouse or focused: the line thickens and the thumb shows (pointer screens). */
    Hover,

    /** Being dragged: the line thickens further and the times follow the finger. */
    Drag,
}

/** The seek bar's keyboard (and D-pad) steps. */
enum class SeekKey { Back, Forward, PageBack, PageForward, Start, End }

/**
 * The straight seek bar of v0.0.4 (docs/design-controls.md): a line with round caps, 6 dp on
 * touch screens and 12 dp while dragged, never a thumb there; with a mouse (desktop mode,
 * Chromebooks) 4 dp, 6 dp with a 14 dp thumb on hover, 8 dp while dragged. The track is inset so
 * the thumb and the caps stay inside; the mini-player's 2 dp progress line; the width from which
 * the player bar replaces the capsule. Sizes are the player tokens (the tests read them from
 * design/tokens.json). Pure geometry in dp or px alike: the views only draw it.
 */
object SeekGeometry {
    /** player.scrubber-track. */
    const val TRACK = 4f

    /** player.scrubber-track-hover. */
    const val TRACK_HOVER = 6f

    /** player.scrubber-track-drag. */
    const val TRACK_DRAG = 8f

    /** player.scrubber-track-touch. */
    const val TRACK_TOUCH = 6f

    /** player.scrubber-track-touch-drag. */
    const val TRACK_TOUCH_DRAG = 12f

    /** player.scrubber-thumb. */
    const val THUMB = 14f

    /** player.mini-progress. */
    const val MINI_PROGRESS = 2f

    /** player.mini-radius: the capsule's corners, which inset its progress line. */
    const val MINI_RADIUS = 16f

    /** player.bar-height. */
    const val BAR_HEIGHT = 80f

    /** Windows this wide (dp) and wider show the player bar instead of the capsule. */
    const val PLAYER_BAR_MIN_WIDTH = 900f

    /** Left and Right (and D-pad) seek this far, in milliseconds. */
    const val SMALL_STEP_MS = 5_000L

    /** Page Up and Page Down seek this far, in milliseconds. */
    const val LARGE_STEP_MS = 30_000L

    fun thickness(state: SeekBarState, touch: Boolean): Float = if (touch) {
        if (state == SeekBarState.Drag) TRACK_TOUCH_DRAG else TRACK_TOUCH
    } else {
        when (state) {
            SeekBarState.Drag -> TRACK_DRAG
            SeekBarState.Hover -> TRACK_HOVER
            SeekBarState.Rest -> TRACK
        }
    }

    /** Pointer screens show the thumb while hovered, focused or dragged; touch screens never. */
    fun showsThumb(state: SeekBarState, touch: Boolean): Boolean = !touch && state != SeekBarState.Rest

    /** The space at each end, so the thumb (or the thickest line's cap) stays inside the control. */
    fun inset(touch: Boolean): Float = (if (touch) TRACK_TOUCH_DRAG else THUMB) / 2f

    fun trackLength(width: Float, touch: Boolean): Float = max(0f, width - 2 * inset(touch))

    /** The fraction of the track at [x] (control coordinates), clamped to 0..1, mirrored right to left. */
    fun fractionAt(x: Float, width: Float, touch: Boolean, rtl: Boolean): Float {
        val length = trackLength(width, touch)
        if (length <= 0f || x.isNaN()) return 0f
        val f = ((x - inset(touch)) / length).coerceIn(0f, 1f)
        return if (rtl) 1f - f else f
    }

    fun playedLength(fraction: Float, width: Float, touch: Boolean): Float = clamp01(fraction) * trackLength(width, touch)

    /** The thumb's centre (control coordinates). */
    fun thumbCentre(fraction: Float, width: Float, touch: Boolean, rtl: Boolean): Float {
        val played = playedLength(fraction, width, touch)
        return if (rtl) width - inset(touch) - played else inset(touch) + played
    }

    /** Visibly different positions: one per device pixel of the line's length (redraw only then). */
    fun steps(widthPx: Float, touch: Boolean, density: Float): Int =
        max(0, (trackLength(widthPx / safe(density), touch) * safe(density)).roundToInt())

    /** The position (ms) a key moves to from [positionMs], within 0..[durationMs]. */
    fun keyTarget(key: SeekKey, positionMs: Long, durationMs: Long): Long {
        val target = when (key) {
            SeekKey.Back -> positionMs - SMALL_STEP_MS
            SeekKey.Forward -> positionMs + SMALL_STEP_MS
            SeekKey.PageBack -> positionMs - LARGE_STEP_MS
            SeekKey.PageForward -> positionMs + LARGE_STEP_MS
            SeekKey.Start -> 0L
            SeekKey.End -> durationMs
        }
        return target.coerceIn(0L, max(0L, durationMs))
    }

    /** The mini-player's line runs along the capsule's bottom edge between its corners. */
    fun miniTrackLength(width: Float): Float = max(0f, width - 2 * MINI_RADIUS)

    fun miniProgressLength(fraction: Float, width: Float): Float = clamp01(fraction) * miniTrackLength(width)

    /** One visible step per device pixel of the mini-player's line. */
    fun miniSteps(widthPx: Float, density: Float): Int =
        max(0, (miniTrackLength(widthPx / safe(density)) * safe(density)).roundToInt())

    /** The player bar replaces the capsule from [PLAYER_BAR_MIN_WIDTH] on. */
    fun usesPlayerBar(widthDp: Float): Boolean = widthDp >= PLAYER_BAR_MIN_WIDTH

    private fun clamp01(f: Float): Float = if (f.isNaN()) 0f else f.coerceIn(0f, 1f)

    private fun safe(density: Float): Float = if (density.isFinite() && density > 0f) density else 1f
}
