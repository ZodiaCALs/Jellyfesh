package org.jellyfesh.logic.theme

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.Appearance
import org.jellyfesh.logic.core.AppearancePref
import org.jellyfesh.logic.core.Theme
import org.jellyfesh.logic.core.ThemeSnapshot

class PaletteTest {
    @Test
    fun rgbaBecomesArgb() {
        assertEquals(0xFF0059A7.toInt(), ColorMath.rgbaToArgb(0x0059A7FFL))
        assertEquals(0x1E101820, ColorMath.rgbaToArgb(0x1018201EL))
        assertEquals(0x00000000, ColorMath.rgbaToArgb(0L))
    }

    @Test
    fun alphaAndBlend() {
        assertEquals(0x80FFFFFF.toInt(), ColorMath.withAlpha(0xFFFFFFFF.toInt(), 0.5f))
        assertEquals(0xFF7F7F7F.toInt(), ColorMath.lerp(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0.5f) - 0x00010101)
        assertEquals(0xFF000000.toInt(), ColorMath.lerp(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), -1f))
    }

    @Test
    fun theFallbackHoldsMistysHue250Colours() {
        // Misty's own sRGB at hue 250 (docs/design-tokens.md; jf-palette's tests check the same).
        assertEquals(0xFF0059A7.toInt(), FallbackTheme.light.getValue("accent.accent"))
        assertEquals(0xFFEEF3F9.toInt(), FallbackTheme.light.getValue("field.sky"))
        assertEquals(0xFF0A1016.toInt(), FallbackTheme.dark.getValue("field.sky"))
        assertEquals(0xFF7ABDFF.toInt(), FallbackTheme.dark.getValue("accent.accent"))
        assertEquals(FallbackTheme.light.keys, FallbackTheme.dark.keys)
        assertEquals(42, FallbackTheme.light.size)
    }

    @Test
    fun thePaletteUsesTheCoresThemeAndFallsBackPerColour() {
        val slice = ThemeSnapshot(
            uid = 1,
            hue = 30f,
            fromArtwork = true,
            light = Theme(30f, Appearance.Light, mapOf("accent.accent" to 0xFFAA3300.toInt())),
            dark = Theme(30f, Appearance.Dark, mapOf("accent.accent" to 0xFFFF9A80.toInt())),
            backdropLight = null,
            backdropDark = "/cache/backdrop/x-dark.png",
        )
        val dark = Palette.of(slice, dark = true)
        assertEquals(0xFFFF9A80.toInt(), dark["accent.accent"])
        assertEquals(FallbackTheme.dark.getValue("ink.ink-90"), dark["ink.ink-90"])
        assertEquals(30f, dark.hue)
        assertEquals(Palette.MISSING, dark["no.such-colour"])
        val first = Palette.of(null, dark = false)
        assertEquals(FallbackTheme.light.getValue("accent.accent"), first["accent.accent"])
        assertEquals(250f, first.hue)
    }

    @Test
    fun appearanceFollowsTheChoiceOrTheSystem() {
        assertTrue(AppearanceRule.isDark(AppearancePref.System, systemDark = true))
        assertFalse(AppearanceRule.isDark(AppearancePref.Light, systemDark = true))
        assertTrue(AppearanceRule.isDark(AppearancePref.Dark, systemDark = false))
    }
}
