package org.jellyfesh.logic.theme

/** Which aurora colour of the theme a glow takes (field.aurora-1 to -3). */
enum class AuroraColour { Aurora1, Aurora2, Aurora3 }

/**
 * One glow of the field: an ellipse centred at ([cx], [cy]) with radii ([rx], [ry]), all as
 * fractions of the field's width and height, in [colour] at [alpha] in its centre, fading to
 * nothing at [fade] of its radius.
 */
data class AuroraGlow(
    val cx: Float,
    val cy: Float,
    val rx: Float,
    val ry: Float,
    val colour: AuroraColour,
    val alpha: Float,
    val fade: Float,
)

/**
 * Luminous, the v0.0.6 design (docs/ui.md, "Luminous"): the numbers the Compose code draws with,
 * so they are tested here.
 */
object Luminous {
    /** field.aurora-strength at which the glows have the reference alphas below (the token's value in v0.0.6). */
    const val REFERENCE_STRENGTH = 0.75f

    /**
     * The four larger glows of the Luminous field (the reference mock-up's phone and desktop
     * screens), their alphas scaled by [strength] (field.aurora-strength, 0 to 1): at 0.75 they
     * are 80, 72, 66 and 46 %. Every screen, light and dark, draws the same four.
     */
    fun glows(strength: Float): List<AuroraGlow> {
        val k = (if (strength.isNaN()) 0f else strength.coerceIn(0f, 1f)) / REFERENCE_STRENGTH
        fun a(reference: Float) = (reference * k).coerceIn(0f, 1f)
        return listOf(
            AuroraGlow(0.04f, 0.00f, 0.62f, 0.58f, AuroraColour.Aurora1, a(0.80f), 0.72f),
            AuroraGlow(1.00f, 0.96f, 0.58f, 0.64f, AuroraColour.Aurora2, a(0.72f), 0.72f),
            AuroraGlow(0.00f, 1.00f, 0.44f, 0.52f, AuroraColour.Aurora3, a(0.66f), 0.70f),
            AuroraGlow(0.92f, 0.04f, 0.38f, 0.40f, AuroraColour.Aurora1, a(0.46f), 0.70f),
        )
    }

    /** A fluid size: [min] up to [widthFrom] dp of window width, [max] from [widthTo], linear between. */
    fun fluid(widthDp: Float, min: Float, max: Float, widthFrom: Float, widthTo: Float): Float {
        if (widthDp.isNaN() || widthTo <= widthFrom) return min
        val t = ((widthDp - widthFrom) / (widthTo - widthFrom)).coerceIn(0f, 1f)
        return min + (max - min) * t
    }

    /**
     * Lyric lines (lyrics.line, 40 at most): the token's fluid range over window widths from 360
     * to 1200 dp, so phones get its minimum (28 at 360 dp) and wide windows the full 40.
     */
    fun lyricLineSp(widthDp: Float, min: Float, max: Float): Float = fluid(widthDp, min, max, 360f, 1200f)

    /**
     * Page titles as Jellyfesh Web sets them (ADR-0023; its `.page-title`): type.text-h2 ([h2],
     * 30.4) on windows under [WIDE_DP], type.text-h1 ([h1], 48) from there.
     */
    fun pageTitleSp(widthDp: Float, h2: Float, h1: Float): Float = if (!widthDp.isNaN() && widthDp >= WIDE_DP) h1 else h2

    /** Where Jellyfesh Web's wide rules start (its `min-width: 900px`: the full bar, the large titles). */
    const val WIDE_DP = 900f

    /**
     * The Now Playing title (type-roles.now-playing-title, 32): the reference phone screen sets it
     * at 25 on a 390 dp phone, so it runs from 24 at 360 dp to the full size from 600 dp.
     */
    fun nowPlayingTitleSp(widthDp: Float, full: Float): Float = fluid(widthDp, minOf(24f, full), full, 360f, 600f)
}
