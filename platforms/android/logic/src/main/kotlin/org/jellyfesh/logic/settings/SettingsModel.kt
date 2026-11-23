package org.jellyfesh.logic.settings

import kotlin.math.abs
import kotlin.math.roundToInt
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CrossfadeSettings
import org.jellyfesh.logic.core.EqPreset
import org.jellyfesh.logic.core.EqSettings
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.Settings
import org.jellyfesh.logic.nav.SettingsSection

/**
 * The ten app icons (ADR-0017, design/icons/round4). Each has an `activity-alias` in the
 * manifest named `.Launcher<aliasName>`; Automatic is Ice, because launchers do not reliably
 * follow dark mode for app icons.
 */
enum class AppIcon(val id: String, val aliasName: String) {
    Ice("ice", "Ice"),
    Steel("steel", "Steel"),
    Original("original", "Original"),
    NavyTide("navy-tide", "NavyTide"),
    Royal("royal", "Royal"),
    TealDeep("teal-deep", "TealDeep"),
    SeaGlass("sea-glass", "SeaGlass"),
    Electric("electric", "Electric"),
    Abyss("abyss", "Abyss"),
    Lagoon("lagoon", "Lagoon"),
}

object AppIcons {
    const val AUTOMATIC = "automatic"

    /** The icon a setting names, or null for Automatic (and for an unknown value). */
    fun chosen(setting: String): AppIcon? = AppIcon.entries.firstOrNull { it.id == setting }

    /** The icon the launcher shows for a setting: Automatic is Ice on Android (ADR-0017). */
    fun launcherIcon(setting: String): AppIcon = chosen(setting) ?: AppIcon.Ice

    /** The full class name of an icon's activity alias. */
    fun aliasClass(namespace: String, icon: AppIcon): String = "$namespace.Launcher${icon.aliasName}"

    /**
     * What to switch when the setting changes from [old] to [new]: the alias to enable and the
     * aliases to disable; null when the launcher icon stays the same.
     */
    fun switch(old: String, new: String): Pair<AppIcon, List<AppIcon>>? {
        val target = launcherIcon(new)
        if (launcherIcon(old) == target) return null
        return target to AppIcon.entries.filter { it != target }
    }
}

/**
 * Edits of the Settings slice. The core owns the settings; the shell sends a modified copy back
 * with `SetSettings` (or the narrower commands the core offers).
 */
object SettingsEdits {
    fun set(settings: Settings, change: Settings.() -> Settings): Command = Command.SetSettings(settings.change())

    /** Crossfade length in whole seconds, 1 to 12. */
    fun crossfade(current: CrossfadeSettings, seconds: Float): CrossfadeSettings =
        current.copy(durationMs = (seconds.roundToInt() * 1_000).coerceIn(CrossfadeSettings.MIN_MS, CrossfadeSettings.MAX_MS))

    /** One EQ band's gain, clamped to ±12 dB and rounded to 0.5 dB steps. */
    fun eqGain(eq: EqSettings, band: Int, gainDb: Float): EqSettings {
        if (band !in eq.bands.indices) return eq
        val gain = (Math.round(gainDb.coerceIn(-EQ_RANGE_DB, EQ_RANGE_DB) * 2f) / 2f)
        return eq.copy(bands = eq.bands.mapIndexed { i, b -> if (i == band) b.copy(gainDb = gain) else b })
    }

    /** Every band back to 0 dB (the core's flat preset keeps the bands' shapes). */
    fun eqFlat(eq: EqSettings): EqSettings = eq.copy(preampDb = 0f, bands = eq.bands.map { it.copy(gainDb = 0f) })

    /** The equalizer's own preamp, clamped to ±12 dB (the bands' range) and rounded to 0.5 dB steps. */
    fun eqPreamp(eq: EqSettings, db: Float): EqSettings =
        eq.copy(preampDb = Math.round(db.coerceIn(-EQ_RANGE_DB, EQ_RANGE_DB) * 2f) / 2f)

    /** A fixed hue in degrees, normalised to 0 until 360; null returns to the artwork's hue. */
    fun hue(hue: Float?): HueSource = if (hue == null) HueSource.Artwork else HueSource.Fixed(((hue % 360f) + 360f) % 360f)

    /** The Transparency slider's percent (0 Solid … 100 Clear) as the core's 0..1, in whole percent. */
    fun transparency(percent: Float): Float =
        if (percent.isNaN()) Settings.DEFAULT_TRANSPARENCY else Math.round(percent.coerceIn(0f, 100f)) / 100f

    /** The core's transparency as the slider's whole percent. */
    fun transparencyPercent(transparency: Float): Int =
        if (transparency.isNaN()) 50 else Math.round(transparency.coerceIn(0f, 1f) * 100f)

    /** ReplayGain pre-amp, ±15 dB in 0.5 dB steps. */
    fun preamp(db: Float): Float = Math.round(db.coerceIn(-15f, 15f) * 2f) / 2f

    const val EQ_RANGE_DB = 12f
}

/** The label of an EQ band's centre frequency: `31`, `250`, `1k`, `16k`. */
object EqLabels {
    fun frequency(hz: Float): String = when {
        hz >= 1_000f -> {
            val k = hz / 1_000f
            if (k == k.toInt().toFloat()) "${k.toInt()}k" else "%.1fk".format(java.util.Locale.ROOT, k)
        }
        else -> hz.roundToInt().toString()
    }

    /**
     * The name of a preset the app has no translation for (a newer core's): its id in words,
     * `loudness_plus` as `Loudness Plus`. The four presets of this release are translated.
     */
    fun presetName(id: String): String =
        id.split('_', '-', ' ').filter { it.isNotEmpty() }.joinToString(" ") { word -> word.replaceFirstChar { it.uppercase(java.util.Locale.ROOT) } }

    /** `+3`, `0`, `−2.5` with a true minus sign. */
    fun gain(db: Float): String {
        val rounded = Math.round(db * 2f) / 2f
        val text = if (rounded == rounded.toInt().toFloat()) kotlin.math.abs(rounded.toInt()).toString() else "%.1f".format(java.util.Locale.ROOT, kotlin.math.abs(rounded))
        return when {
            rounded > 0f -> "+$text"
            rounded < 0f -> "−$text"
            else -> "0"
        }
    }
}

/** What the equalizer's Preset picker shows: one of the core's presets, or Custom once a band moved. */
sealed interface EqChoice {
    /** A preset of `eq_presets()`, by its stable id. */
    data class Preset(val id: String) : EqChoice

    /** The bands match none of the presets. */
    data object Custom : EqChoice
}

/**
 * The equalizer's presets (docs/ui.md, Settings › Playback › Equalizer). The list and the bands
 * come from the core (`eq_presets()`); this only recognises which one the bands match and applies
 * one. A preset is its bands: the switch and the equalizer's preamp are the person's own, so
 * moving the preamp never turns a preset into Custom, and choosing a preset keeps both.
 */
object EqPresets {
    /** The id of the core's flat preset, which Reset to Flat applies. */
    const val FLAT = "flat"

    /** The preset whose bands [eq] has (same shapes and centres, gains within 0.05 dB), or null. */
    fun matching(eq: EqSettings, presets: List<EqPreset>): EqPreset? =
        presets.firstOrNull { sameBands(it.settings, eq) }

    /** The picker's current choice. */
    fun choice(eq: EqSettings, presets: List<EqPreset>): EqChoice =
        matching(eq, presets)?.let { EqChoice.Preset(it.id) } ?: EqChoice.Custom

    /** The picker's options: the core's presets in its order, then Custom while the bands match none. */
    fun options(eq: EqSettings, presets: List<EqPreset>): List<EqChoice> {
        val named = presets.map { EqChoice.Preset(it.id) }
        return if (presets.isNotEmpty() && matching(eq, presets) != null) named else named + EqChoice.Custom
    }

    /** [preset]'s bands, keeping whether the equalizer is on and its preamp. */
    fun apply(current: EqSettings, preset: EqPreset): EqSettings = current.copy(bands = preset.settings.bands)

    /** The settings a choice sets: a preset's bands, or [current] for Custom (nothing to apply). */
    fun select(current: EqSettings, choice: EqChoice, presets: List<EqPreset>): EqSettings = when (choice) {
        is EqChoice.Preset -> presets.firstOrNull { it.id == choice.id }?.let { apply(current, it) } ?: current
        EqChoice.Custom -> current
    }

    /**
     * Reset to Flat: the core's flat preset with a 0 dB preamp, keeping the switch; every band at
     * 0 dB in its place when the core lists no flat preset.
     */
    fun resetToFlat(current: EqSettings, presets: List<EqPreset>): EqSettings {
        val flat = presets.firstOrNull { it.id == FLAT } ?: return SettingsEdits.eqFlat(current)
        return current.copy(preampDb = 0f, bands = flat.settings.bands)
    }

    private fun sameBands(preset: EqSettings, eq: EqSettings): Boolean =
        preset.bands.size == eq.bands.size &&
            preset.bands.zip(eq.bands).all { (a, b) ->
                a.kind == b.kind && abs(a.freqHz - b.freqHz) < 0.5f && abs(a.gainDb - b.gainDb) < 0.05f && a.enabled == b.enabled
            }
}

/** A group of rows on a settings page, under its heading (docs/ui.md, "Settings"). */
enum class SettingsGroup {
    // Appearance
    Theme,
    AccentColour,
    Transparency,
    NowPlayingBackdrop,
    Lyrics,

    /** Mini Player (v0.0.8, ADR-0026 §4): on Android, Mini Player in Picture-in-Picture. */
    MiniPlayer,
    AppIcon,

    /** Customize…, which opens Extensions › Customize (v0.0.7). */
    Customize,

    // Playback
    Output,
    Crossfade,
    VolumeLevelling,
    Equalizer,

    /** Streaming Quality (v0.0.8, ADR-0026 §1), the same setting as the bar's quality menu. */
    Streaming,
    Startup,

    // Library
    /** Android's own first group (an allowed difference): access to music and Find Music on This Phone. */
    MusicOnThisPhone,
    MusicFolders,
    WatchFolders,
    RescanLibrary,

    // Downloads (v0.0.8, ADR-0025)
    DownloadFolder,
    DownloadQuality,
    DownloadFileNames,
    DownloadTags,

    /** Wi-Fi Only (Android) and Downloads at Once. */
    DownloadNetwork,

    /** Space Used and Remove All Downloads…. */
    DownloadSpace,

    // Sources
    AddSource,
    Installed,
    CommunityPlugins,

    // Extensions
    JellyfeshPro,

    /** The installed extensions, with Install from File… in the heading. */
    InstalledExtensions,

    // About
    About,

    /** Check for Updates, in direct downloads only (the Update slice's `supported`). */
    Updates,
}

/** Which groups each settings page shows, in the order every app shows them (docs/ui.md). */
object SettingsLayout {
    fun groups(section: SettingsSection): List<SettingsGroup> = when (section) {
        SettingsSection.Appearance -> listOf(
            SettingsGroup.Theme,
            SettingsGroup.AccentColour,
            SettingsGroup.Transparency,
            SettingsGroup.NowPlayingBackdrop,
            SettingsGroup.Lyrics,
            SettingsGroup.MiniPlayer,
            SettingsGroup.AppIcon,
            SettingsGroup.Customize,
        )
        SettingsSection.Playback -> listOf(
            SettingsGroup.Output,
            SettingsGroup.Crossfade,
            SettingsGroup.VolumeLevelling,
            SettingsGroup.Equalizer,
            SettingsGroup.Streaming,
            SettingsGroup.Startup,
        )
        SettingsSection.Library -> listOf(
            SettingsGroup.MusicOnThisPhone,
            SettingsGroup.MusicFolders,
            SettingsGroup.WatchFolders,
            SettingsGroup.RescanLibrary,
        )
        SettingsSection.Downloads -> listOf(
            SettingsGroup.DownloadFolder,
            SettingsGroup.DownloadQuality,
            SettingsGroup.DownloadFileNames,
            SettingsGroup.DownloadTags,
            SettingsGroup.DownloadNetwork,
            SettingsGroup.DownloadSpace,
        )
        SettingsSection.Sources -> listOf(SettingsGroup.AddSource, SettingsGroup.Installed, SettingsGroup.CommunityPlugins)
        SettingsSection.Extensions -> listOf(SettingsGroup.JellyfeshPro, SettingsGroup.InstalledExtensions)
        SettingsSection.About -> listOf(SettingsGroup.About, SettingsGroup.Updates)
    }
}

/**
 * Settings › About: the version from the build (never typed in), and the website with the
 * third-party notices it publishes (`THIRD_PARTY_NOTICES.html`, tools/release/build_site.py) and
 * the legal pages (`legal/`, legal/README.md): the Licence Agreement, the Privacy Policy and the
 * Terms of Sale, which About, Settings › Extensions › Jellyfesh Pro and the Pro sheet link to.
 */
object AboutInfo {
    /** The page the website publishes the third-party notices on, relative to the website. */
    const val NOTICES_PAGE = "THIRD_PARTY_NOTICES.html"

    /** The legal pages, relative to the website (tools/release/legal_pages.py writes them). */
    const val LICENCE_AGREEMENT_PAGE = "legal/eula.html"
    const val PRIVACY_POLICY_PAGE = "legal/privacy.html"
    const val TERMS_OF_SALE_PAGE = "legal/terms-of-sale.html"

    /** The website and the pages it publishes; null when the build was given no website. */
    data class Links(
        val website: String,
        val licences: String,
        val licenceAgreement: String,
        val privacyPolicy: String,
        val termsOfSale: String,
    )

    /** [website] as the build gave it (blank: none); a trailing slash is added. */
    fun links(website: String?): Links? {
        val site = website?.trim()?.takeIf { it.startsWith("https://") || it.startsWith("http://") } ?: return null
        val base = if (site.endsWith('/')) site else "$site/"
        return Links(
            website = base,
            licences = base + NOTICES_PAGE,
            licenceAgreement = base + LICENCE_AGREEMENT_PAGE,
            privacyPolicy = base + PRIVACY_POLICY_PAGE,
            termsOfSale = base + TERMS_OF_SALE_PAGE,
        )
    }

    /**
     * The version line: the app's version from its build, with the core library's after it only
     * when they differ (they are released together, so normally one number shows).
     */
    fun version(app: String, core: String?): String {
        val coreVersion = core?.trim()?.takeIf { it.isNotEmpty() && it != app.trim() }
        return if (coreVersion == null) app.trim() else "${app.trim()} (core $coreVersion)"
    }
}
