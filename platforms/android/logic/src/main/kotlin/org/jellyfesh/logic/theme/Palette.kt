package org.jellyfesh.logic.theme

import org.jellyfesh.logic.core.AppearancePref
import org.jellyfesh.logic.core.ThemeSnapshot

/** Colour conversions between the core's packing and Android's. */
object ColorMath {
    /** The core's `0xRRGGBBAA` (ThemeColor.rgba) as Android's `0xAARRGGBB`. */
    fun rgbaToArgb(rgba: Long): Int {
        val v = rgba and 0xFFFFFFFFL
        val a = v and 0xFF
        val rgb = v ushr 8
        return ((a shl 24) or rgb).toInt()
    }

    /** [argb] with its alpha multiplied by [factor] (0 to 1). */
    fun withAlpha(argb: Int, factor: Float): Int {
        val a = ((argb ushr 24) and 0xFF) * factor.coerceIn(0f, 1f)
        return (Math.round(a) shl 24) or (argb and 0x00FFFFFF)
    }

    /** Linear blend from [from] to [to] at [t] (0 to 1), per channel including alpha. */
    fun lerp(from: Int, to: Int, t: Float): Int {
        val k = t.coerceIn(0f, 1f)
        fun ch(shift: Int): Int {
            val a = (from ushr shift) and 0xFF
            val b = (to ushr shift) and 0xFF
            return Math.round(a + (b - a) * k) and 0xFF
        }
        return (ch(24) shl 24) or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
    }
}

/**
 * The theme colours to draw with: the core's Theme slice for the current appearance, or the
 * hue-250 fallback for any colour the slice does not hold (and before the first slice).
 */
class Palette(private val colors: Map<String, Int>, private val fallback: Map<String, Int>, val dark: Boolean, val hue: Float) {
    /** The colour named [key] (`ink.ink-90`), as ARGB. */
    operator fun get(key: String): Int = colors[key] ?: fallback[key] ?: MISSING

    companion object {
        /** Magenta: a colour key that neither the theme nor the fallback knows (a token rename). */
        const val MISSING: Int = 0xFFFF00FF.toInt()

        fun of(theme: ThemeSnapshot?, dark: Boolean): Palette {
            val fallback = if (dark) FallbackTheme.dark else FallbackTheme.light
            val slice = theme?.theme(dark)
            return Palette(slice?.colors.orEmpty(), fallback, dark, slice?.hue ?: FallbackTheme.HUE)
        }
    }
}

/** Light or dark: the person's choice, or the system's when they follow it. */
object AppearanceRule {
    fun isDark(pref: AppearancePref, systemDark: Boolean): Boolean = when (pref) {
        AppearancePref.System -> systemDark
        AppearancePref.Light -> false
        AppearancePref.Dark -> true
    }
}
