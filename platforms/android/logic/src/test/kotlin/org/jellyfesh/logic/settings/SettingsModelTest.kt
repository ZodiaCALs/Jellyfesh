package org.jellyfesh.logic.settings

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.jellyfesh.logic.core.BandKind
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CrossfadeSettings
import org.jellyfesh.logic.core.EqBand
import org.jellyfesh.logic.core.EqPreset
import org.jellyfesh.logic.core.EqSettings
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.LyricsAlign
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.nav.SettingsSection
import org.jellyfesh.logic.theme.AccentChoices

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

    // ---- docs/ui.md, "Settings" ----------------------------------------------------------------

    @Test
    fun sectionsAreTheSpecsWithAboutAndNoLyricsSection() {
        assertEquals(
            listOf(
                SettingsSection.Appearance,
                SettingsSection.Playback,
                SettingsSection.Library,
                SettingsSection.Downloads,
                SettingsSection.Sources,
                SettingsSection.Extensions,
                SettingsSection.About,
            ),
            SettingsSection.entries,
        )
    }

    @Test
    fun groupsComeInTheSpecsOrder() {
        assertEquals(
            listOf(
                SettingsGroup.Theme,
                SettingsGroup.AccentColour,
                SettingsGroup.Transparency,
                SettingsGroup.NowPlayingBackdrop,
                SettingsGroup.Lyrics,
                SettingsGroup.MiniPlayer,
                SettingsGroup.AppIcon,
                SettingsGroup.Customize,
            ),
            SettingsLayout.groups(SettingsSection.Appearance),
        )
        assertEquals(
            listOf(
                SettingsGroup.Output,
                SettingsGroup.Crossfade,
                SettingsGroup.VolumeLevelling,
                SettingsGroup.Equalizer,
                SettingsGroup.Streaming,
                SettingsGroup.Startup,
            ),
            SettingsLayout.groups(SettingsSection.Playback),
        )
        // Android asks for access to music and offers Find Music on This Phone before the folders.
        assertEquals(
            listOf(SettingsGroup.MusicOnThisPhone, SettingsGroup.MusicFolders, SettingsGroup.WatchFolders, SettingsGroup.RescanLibrary),
            SettingsLayout.groups(SettingsSection.Library),
        )
        assertEquals(
            listOf(
                SettingsGroup.DownloadFolder,
                SettingsGroup.DownloadQuality,
                SettingsGroup.DownloadFileNames,
                SettingsGroup.DownloadTags,
                SettingsGroup.DownloadNetwork,
                SettingsGroup.DownloadSpace,
            ),
            SettingsLayout.groups(SettingsSection.Downloads),
        )
        assertEquals(
            listOf(SettingsGroup.AddSource, SettingsGroup.Installed, SettingsGroup.CommunityPlugins),
            SettingsLayout.groups(SettingsSection.Sources),
        )
        assertEquals(listOf(SettingsGroup.JellyfeshPro, SettingsGroup.InstalledExtensions), SettingsLayout.groups(SettingsSection.Extensions))
        assertEquals(listOf(SettingsGroup.About, SettingsGroup.Updates), SettingsLayout.groups(SettingsSection.About))
    }

    @Test
    fun everyGroupShowsOnceAndLyricsOnlyInAppearance() {
        val all = SettingsSection.entries.flatMap { SettingsLayout.groups(it) }
        assertEquals(SettingsGroup.entries.toSet(), all.toSet())
        assertEquals(all.size, all.toSet().size)
        SettingsSection.entries.filter { it != SettingsSection.Appearance }.forEach { section ->
            assertFalse(SettingsGroup.Lyrics in SettingsLayout.groups(section), "lyrics settings in $section")
        }
        assertEquals(SettingsGroup.Startup, SettingsLayout.groups(SettingsSection.Playback).last())
    }

    // ---- the equalizer's presets (the core's eq_presets()) --------------------------------------

    private fun graphic(vararg gains: Float) = EqSettings(
        enabled = true,
        preampDb = 0f,
        bands = EqSettings.GRAPHIC_FREQS.zip(gains.toList()) { f, g -> EqBand(BandKind.Peaking, f, g, 1.41f, true) },
    )

    /** As core/jf-engine's EqSettings::presets(). */
    private val presets = listOf(
        EqPreset("flat", graphic(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f)),
        EqPreset("bass_boost", graphic(6f, 5f, 4f, 2f, 0.5f, 0f, 0f, 0f, 0f, 0f)),
        EqPreset("vocal", graphic(-2f, -2f, -1f, 0f, 1.5f, 3f, 3f, 2f, 0f, -1f)),
        EqPreset("treble", graphic(0f, 0f, 0f, 0f, 0f, 0f, 1.5f, 3f, 4.5f, 6f)),
    )

    @Test
    fun thePickerShowsThePresetTheBandsMatch() {
        // The core's default is the flat preset, switched off.
        assertEquals(EqChoice.Preset("flat"), EqPresets.choice(EqSettings.Default, presets))
        assertEquals(EqChoice.Preset("vocal"), EqPresets.choice(presets[2].settings.copy(enabled = false), presets))
        // The preamp is the person's own: moving it keeps the preset.
        assertEquals(EqChoice.Preset("treble"), EqPresets.choice(presets[3].settings.copy(preampDb = -4f), presets))
        assertEquals(listOf("flat", "bass_boost", "vocal", "treble"), EqPresets.options(EqSettings.Default, presets).map { (it as EqChoice.Preset).id })
    }

    @Test
    fun customAppearsOnceABandMoves() {
        val moved = SettingsEdits.eqGain(presets[1].settings, 4, 3f)
        assertEquals(EqChoice.Custom, EqPresets.choice(moved, presets))
        assertNull(EqPresets.matching(moved, presets))
        assertEquals(EqChoice.Custom, EqPresets.options(moved, presets).last())
        assertEquals(5, EqPresets.options(moved, presets).size)
        // Without the core's list there is nothing to match.
        assertEquals(listOf<EqChoice>(EqChoice.Custom), EqPresets.options(moved, emptyList()))
    }

    @Test
    fun choosingAPresetKeepsTheSwitchAndThePreamp() {
        val current = SettingsEdits.eqPreamp(EqSettings.Default, -3f)
        val bass = EqPresets.select(current, EqChoice.Preset("bass_boost"), presets)
        assertEquals(presets[1].settings.bands, bass.bands)
        assertFalse(bass.enabled)
        assertEquals(-3f, bass.preampDb)
        assertEquals(current, EqPresets.select(current, EqChoice.Custom, presets))
        assertEquals(current, EqPresets.select(current, EqChoice.Preset("unknown"), presets))
    }

    @Test
    fun resetToFlatIsTheCoresFlatPresetWithNoPreamp() {
        val moved = SettingsEdits.eqPreamp(SettingsEdits.eqGain(presets[2].settings, 0, 9f), 5f)
        val flat = EqPresets.resetToFlat(moved, presets)
        assertEquals(presets[0].settings.bands, flat.bands)
        assertEquals(0f, flat.preampDb)
        assertTrue(flat.enabled)
        assertEquals(EqChoice.Preset("flat"), EqPresets.choice(flat, presets))
        // Without the core's list, every band goes to 0 dB in place.
        assertTrue(EqPresets.resetToFlat(moved, emptyList()).bands.all { it.gainDb == 0f })
    }

    @Test
    fun theEqualizersPreampIsClampedAndStepped() {
        assertEquals(2.5f, SettingsEdits.eqPreamp(EqSettings.Default, 2.4f).preampDb)
        assertEquals(-12f, SettingsEdits.eqPreamp(EqSettings.Default, -30f).preampDb)
        assertEquals(12f, SettingsEdits.eqPreamp(EqSettings.Default, 99f).preampDb)
    }

    @Test
    fun aPresetThisVersionDoesNotKnowShowsItsId() {
        assertEquals("Bass Boost", EqLabels.presetName("bass_boost"))
        assertEquals("Loudness Plus", EqLabels.presetName("loudness_plus"))
    }

    // ---- About and the accent swatches ----------------------------------------------------------

    @Test
    fun aboutLinksTheWebsiteAndItsNotices() {
        val links = AboutInfo.links("https://example.github.io/jellyfesh")!!
        assertEquals("https://example.github.io/jellyfesh/", links.website)
        assertEquals("https://example.github.io/jellyfesh/THIRD_PARTY_NOTICES.html", links.licences)
        // The legal pages (legal/README.md): About and the Pro sheet link to them.
        assertEquals("https://example.github.io/jellyfesh/legal/eula.html", links.licenceAgreement)
        assertEquals("https://example.github.io/jellyfesh/legal/privacy.html", links.privacyPolicy)
        assertEquals("https://example.github.io/jellyfesh/legal/terms-of-sale.html", links.termsOfSale)
        assertEquals(links, AboutInfo.links(" https://example.github.io/jellyfesh/ "))
        // A build without a website shows the version only.
        assertNull(AboutInfo.links(""))
        assertNull(AboutInfo.links(null))
        assertNull(AboutInfo.links("example.org"))
    }

    @Test
    fun theVersionIsTheBuildsWithTheCoresOnlyWhenTheyDiffer() {
        assertEquals("0.0.4", AboutInfo.version("0.0.4", "0.0.4"))
        assertEquals("0.0.4", AboutInfo.version("0.0.4", null))
        assertEquals("0.0.5 (core 0.0.4)", AboutInfo.version("0.0.5", "0.0.4"))
    }

    @Test
    fun theSwatchesAreNamedAndTheLastIsViolet() {
        assertEquals(AccentChoices.SWATCH_HUES.size, AccentChoices.SWATCH_NAMES.size)
        assertEquals("violet", AccentChoices.SWATCH_NAMES.last())
        assertFalse("purple" in AccentChoices.SWATCH_NAMES)
    }
}
