package org.jellyfesh.logic.settings

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CrossfadeSettings
import org.jellyfesh.logic.core.EqSettings
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.LyricsAlign
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.core.Settings

class SettingsModelTest {
    @Test
    fun automaticIsIceOnAndroid() {
        assertEquals(AppIcon.Ice, AppIcons.launcherIcon("automatic"))
        assertEquals(AppIcon.SeaGlass, AppIcons.launcherIcon("sea-glass"))
        assertEquals(AppIcon.Ice, AppIcons.launcherIcon("unknown"))
        assertNull(AppIcons.chosen("automatic"))
        assertEquals(10, AppIcon.entries.size)
    }

    @Test
    fun switchingIconsTouchesTheAliases() {
        assertNull(AppIcons.switch("automatic", "ice"), "Automatic and Ice share the Ice alias")
        val (enable, disable) = AppIcons.switch("ice", "navy-tide")!!
        assertEquals(AppIcon.NavyTide, enable)
        assertEquals(9, disable.size)
        assertEquals("org.jellyfesh.android.LauncherNavyTide", AppIcons.aliasClass("org.jellyfesh.android", enable))
    }

    @Test
    fun iconIdsMatchTheTokens() {
        // design/tokens.json brand.icon-options, in order.
        val tokens = "ice, steel, original, navy-tide, royal, teal-deep, sea-glass, electric, abyss, lagoon"
        assertEquals(tokens.split(", "), AppIcon.entries.map { it.id })
    }

    @Test
    fun settingsEditsSendACopy() {
        val settings = Settings()
        val command = SettingsEdits.set(settings) { copy(reduceEffects = true) }
        assertEquals(Command.SetSettings(settings.copy(reduceEffects = true)), command)
    }

    @Test
    fun defaultsMatchTheCore() {
        // core/jf-core/src/types.rs, Settings::default().
        val settings = Settings()
        assertEquals(0.5f, settings.transparency)
        assertEquals(LyricsSize.Medium, settings.lyricsSize)
        assertEquals(LyricsAlign.Start, settings.lyricsAlign)
        val changed = SettingsEdits.set(settings) { copy(transparency = 0.2f, lyricsSize = LyricsSize.Large, lyricsAlign = LyricsAlign.Center) }
        assertEquals(Command.SetSettings(Settings(transparency = 0.2f, lyricsSize = LyricsSize.Large, lyricsAlign = LyricsAlign.Center)), changed)
    }

    @Test
    fun crossfadeIsWholeSecondsWithinRange() {
        assertEquals(1_000, SettingsEdits.crossfade(CrossfadeSettings.Default, 0.2f).durationMs)
        assertEquals(7_000, SettingsEdits.crossfade(CrossfadeSettings.Default, 6.6f).durationMs)
        assertEquals(12_000, SettingsEdits.crossfade(CrossfadeSettings.Default, 30f).durationMs)
    }

    @Test
    fun eqGainsAreClampedAndStepped() {
        val eq = SettingsEdits.eqGain(EqSettings.Default, 2, 4.3f)
        assertEquals(4.5f, eq.bands[2].gainDb)
        assertEquals(12f, SettingsEdits.eqGain(eq, 0, 40f).bands[0].gainDb)
        assertEquals(eq, SettingsEdits.eqGain(eq, 99, 1f))
        assertEquals(0f, SettingsEdits.eqFlat(eq).bands[2].gainDb)
        assertEquals(10, EqSettings.Default.bands.size)
    }

    @Test
    fun huesAreNormalised() {
        assertEquals(HueSource.Fixed(10f), SettingsEdits.hue(370f))
        assertEquals(HueSource.Fixed(350f), SettingsEdits.hue(-10f))
        assertEquals(HueSource.Artwork, SettingsEdits.hue(null))
    }

    @Test
    fun labels() {
        assertEquals("31", EqLabels.frequency(31f))
        assertEquals("1k", EqLabels.frequency(1_000f))
        assertEquals("16k", EqLabels.frequency(16_000f))
        assertEquals("2.5k", EqLabels.frequency(2_500f))
        assertEquals("+3", EqLabels.gain(3f))
        assertEquals("−2.5", EqLabels.gain(-2.5f))
        assertEquals("0", EqLabels.gain(0.1f))
        assertEquals(-15f, SettingsEdits.preamp(-40f))
    }
}
