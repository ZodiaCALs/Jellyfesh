package org.jellyfesh.logic.settings

import kotlin.math.roundToInt
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CrossfadeSettings
import org.jellyfesh.logic.core.EqSettings
import org.jellyfesh.logic.core.HueSource
import org.jellyfesh.logic.core.Settings

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
