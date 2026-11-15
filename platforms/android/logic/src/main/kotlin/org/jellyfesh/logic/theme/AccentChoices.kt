package org.jellyfesh.logic.theme

import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import org.jellyfesh.logic.core.HueSource

/**
 * OKLCH to 8-bit sRGB as jf_palette converts (core/jf-palette/src/color.rs: the CSS Color 4 OKLab
 * matrices in double, each channel clipped to 0..1, rounded as floor(v·255 + 0.5)), so a swatch
 * shows the accent the core resolves for its hue. Presentation only: themes still come from the core.
 */
object Oklch {
    private val OKLAB_TO_LMS = arrayOf(
        doubleArrayOf(1.0, 0.3963377773761749, 0.2158037573099136),
        doubleArrayOf(1.0, -0.1055613458156586, -0.0638541728258133),
        doubleArrayOf(1.0, -0.0894841775298119, -1.2914855480194092),
    )
    private val LMS_TO_XYZ = arrayOf(
        doubleArrayOf(1.2268798758459243, -0.5578149944602171, 0.2813910456659647),
        doubleArrayOf(-0.0405757452148008, 1.112286803280317, -0.0717110580655164),
        doubleArrayOf(-0.0763729366746601, -0.4214933324022432, 1.5869240198367816),
    )
    private val XYZ_TO_LINEAR_SRGB = arrayOf(
        doubleArrayOf(12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0),
        doubleArrayOf(-851781.0 / 878810.0, 1648619.0 / 878810.0, 36519.0 / 878810.0),
        doubleArrayOf(705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0),
    )

    /** oklch(l c hue) as 0xAARRGGBB (opaque), Android's colour int. */
    fun toArgb(l: Double, c: Double, hue: Double): Int {
        val h = Math.toRadians(hue)
        val lms = multiply(OKLAB_TO_LMS, doubleArrayOf(l, c * cos(h), c * sin(h))).map { it * it * it }.toDoubleArray()
        val rgb = multiply(XYZ_TO_LINEAR_SRGB, multiply(LMS_TO_XYZ, lms))
        val (r, g, b) = rgb.map { quantize(encode(it)) }
        return (0xFF shl 24) or (r shl 16) or (g shl 8) or b
    }

    private fun multiply(m: Array<DoubleArray>, v: DoubleArray): DoubleArray =
        DoubleArray(3) { i -> m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] }

    private fun encode(v: Double): Double {
        val a = abs(v)
        val encoded = if (a > 0.0031308) 1.055 * a.pow(1 / 2.4) - 0.055 else 12.92 * a
        return Math.copySign(encoded, v).coerceIn(0.0, 1.0)
    }

    private fun quantize(v: Double): Int = floor(v.coerceIn(0.0, 1.0) * 255 + 0.5).toInt()
}

/**
 * Preferences → Appearance → Accent colour (docs/design-controls.md, "v0.0.4"): follow the
 * artwork, one of ten swatches, or a custom hue. All three are the core's hue source.
 */
object AccentChoices {
    /** The swatches' hues in the order shown: Misty blue first, then round the wheel. */
    val SWATCH_HUES: List<Float> = listOf(250f, 210f, 180f, 150f, 120f, 60f, 30f, 0f, 330f, 290f)

    /**
     * The swatches' names in [SWATCH_HUES] order, as stable ids the app translates (the Linux
     * app's names; docs/ui.md: the last is Violet, not Purple).
     */
    val SWATCH_NAMES: List<String> = listOf("blue", "cyan", "teal", "green", "olive", "orange", "red", "rose", "magenta", "violet")

    /** A hue in degrees, normalised to 0 until 360 (NaN counts as Misty's 250). */
    fun normalize(hue: Float): Float {
        if (!hue.isFinite()) return 250f
        val h = ((hue % 360f) + 360f) % 360f
        return if (h >= 360f) 0f else h
    }

    /** The swatch a hue source shows as chosen: its index, or -1 for the artwork's hue or a custom one. */
    fun swatchIndex(source: HueSource): Int {
        val fixed = source as? HueSource.Fixed ?: return -1
        val hue = normalize(fixed.hue)
        return SWATCH_HUES.indexOfFirst { swatch ->
            val d = abs(hue - swatch)
            min(d, 360f - d) < 0.5f
        }
    }

    /** The accent at [hue]: accent.accent is oklch(0.46 0.15 h) light and oklch(0.78 0.12 h) dark. */
    fun accent(hue: Float, dark: Boolean): Int =
        if (dark) Oklch.toArgb(0.78, 0.12, normalize(hue).toDouble()) else Oklch.toArgb(0.46, 0.15, normalize(hue).toDouble())

    /** The custom hue slider's track: the accent every 30 degrees (13 stops, red at both ends). */
    fun hueTrack(dark: Boolean): List<Int> = (0..12).map { accent((it * 30f) % 360f, dark) }
}
