package org.jellyfesh.logic.player

import kotlin.math.min
import org.jellyfesh.logic.core.LyricsSize

/** Where a lyric line is relative to the line being sung. */
enum class LyricLineRole {
    /** Not yet sung: lyrics.ink-inactive (ink-50). */
    Upcoming,

    /** The line being sung: ink-90 at full size. */
    Active,

    /** Already sung: ink-30. */
    Sung,
}

/** One keyframe of the lyrics cascade: at [progress] (0 to 1) a line is drawn [offset] below the list. */
data class CascadeKeyframe(val progress: Float, val offset: Float)

/** A CSS cubic-bezier easing curve (Misty's motion tokens), evaluated in code. */
class CubicBezier(private val x1: Float, private val y1: Float, private val x2: Float, private val y2: Float) {
    /** The eased progress at time fraction [x] (0 to 1). */
    fun ease(x: Float): Float {
        if (x <= 0f) return 0f
        if (x >= 1f) return 1f
        var lo = 0f
        var hi = 1f
        var t = x
        repeat(24) {
            t = (lo + hi) / 2f
            if (sample(x1, x2, t) < x) lo = t else hi = t
        }
        return sample(y1, y2, t)
    }

    private fun sample(p1: Float, p2: Float, t: Float): Float {
        val u = 1f - t
        return 3f * u * u * t * p1 + 3f * u * t * t * p2 + t * t * t
    }
}

/**
 * Now Playing's motion after Apple Music (docs/design-controls.md, "v0.0.4"), as numbers the views
 * animate with Compose: the artwork's pause scale, lyric sizes and roles, and the lyrics cascade.
 * With reduced motion the artwork stays at full size and the lyrics jump.
 */
object NowPlayingMotion {
    /** player.artwork-paused-scale. */
    const val ARTWORK_PAUSED_SCALE = 0.88f

    /** lyrics.scale-inactive (Luminous: 92 %). */
    const val INACTIVE_LINE_SCALE = 0.92f

    /** lyrics.scale-small. */
    const val SMALL_LYRICS = 0.8f

    /** lyrics.scale-large. */
    const val LARGE_LYRICS = 1.2f

    /** lyrics.stagger: each following line starts this much after the one above (Luminous: 40 ms). */
    const val STAGGER_MS = 40

    /** Lines further below the active one start together, so the cascade never drags. */
    const val MAX_STAGGERED_LINES = 8

    /** 1 while playing, 0.88 paused; always 1 with reduced motion. */
    fun artworkScale(playing: Boolean, reduceMotion: Boolean): Float = if (playing || reduceMotion) 1f else ARTWORK_PAUSED_SCALE

    /** The Lyrics size setting as a factor of the token sizes (line, translation, romanization). */
    fun lyricsScale(size: LyricsSize): Float = when (size) {
        LyricsSize.Small -> SMALL_LYRICS
        LyricsSize.Medium -> 1f
        LyricsSize.Large -> LARGE_LYRICS
    }

    /** The role of line [index] while line [active] is sung (null: none yet). */
    fun role(index: Int, active: Int?): LyricLineRole = when {
        active == null -> LyricLineRole.Upcoming
        index == active -> LyricLineRole.Active
        index < active -> LyricLineRole.Sung
        else -> LyricLineRole.Upcoming
    }

    fun lineScale(role: LyricLineRole): Float = if (role == LyricLineRole.Active) 1f else INACTIVE_LINE_SCALE

    /** How much later than the scroll line [index] starts moving: 0 at and above [active], then 40 ms per line. */
    fun cascadeDelayMs(index: Int, active: Int?): Int =
        if (active == null || index <= active) 0 else STAGGER_MS * min(index - active, MAX_STAGGERED_LINES)

    /**
     * The extra offset that makes a line start [delayMs] after a scroll of [distance] that runs
     * [durationMs] on [ease]: drawn distance·(E(t/D) − E((t−delay)/D)) lower, sampled over the
     * delay plus the duration (0 at both ends) for a keyframes animation.
     */
    fun cascadeLag(distance: Float, delayMs: Int, durationMs: Int, ease: (Float) -> Float, samples: Int = 12): List<CascadeKeyframe> {
        val d = durationMs.coerceAtLeast(1).toFloat()
        val lag = delayMs.coerceAtLeast(0).toFloat()
        val total = d + lag
        return (0..samples).map { i ->
            val t = total * i / samples
            val offset = if (i == samples) 0f else distance * (ease(t / d) - ease((t - lag) / d))
            CascadeKeyframe(i.toFloat() / samples, offset)
        }
    }
}
