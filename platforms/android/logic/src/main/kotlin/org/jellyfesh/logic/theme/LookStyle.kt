package org.jellyfesh.logic.theme

import org.jellyfesh.logic.core.Density
import org.jellyfesh.logic.core.HomeSection
import org.jellyfesh.logic.core.LookSnapshot
import org.jellyfesh.logic.core.LyricsHighlight
import org.jellyfesh.logic.core.LyricsWeight
import org.jellyfesh.logic.core.MiniPlayerStyle
import org.jellyfesh.logic.core.NowPlayingLayout
import org.jellyfesh.logic.core.Place
import org.jellyfesh.logic.core.Typeface

/** The colour of sung lyrics: the lyrics' sung ink (v0.0.6), the accent, or the person's colour. */
sealed interface SungInk {
    data object Token : SungInk
    data object Accent : SungInk

    /** 0xRRGGBB. */
    data class Custom(val rgb: Int) : SungInk
}

/**
 * The look (ADR-0020, docs/ui.md "The Look slice") as the numbers the views draw with: radii ×
 * the corner scale, glass that the Transparency setting and Reduce Transparency still overrule,
 * the aurora's strength, the platform's own faces at the text scale, spacing for the density, the
 * lyrics' weight, glow and highlight, the order of the library's places and of Home, and the Now
 * Playing layout and mini-player style. With `customized` false every value is the Luminous
 * default, whatever the slice holds, so the app draws exactly as in v0.0.6.
 */
data class LookStyle(val state: LookSnapshot) {
    val customized: Boolean get() = state.customized

    /** Radii ×, 0–2 (1 is Luminous). */
    val cornerScale: Float get() = if (customized) clamp(state.cornerScale, 0f, 2f, 1f) else 1f

    fun radius(base: Float): Float = base * cornerScale

    /** How much glass shows what lies behind it: 0 (solid panels) to 1 (Luminous). */
    val glass: Float get() = if (customized) clamp(state.glass, 0f, 1f, 1f) else 1f

    /** The Transparency glass is drawn with: the setting's (0–1) times the look's glass, so the setting still wins. */
    fun transparency(setting: Float): Float = (if (setting.isNaN()) 0.5f else setting.coerceIn(0f, 1f)) * glass

    /** The field's aurora strength: [token] (`field.aurora-strength`) while not customized. */
    fun aurora(token: Float): Float = if (customized) clamp(state.aurora, 0f, 1f, token) else token

    /** The face: the platform's own (Android has no rounded system face: Rounded uses the system's). */
    val typeface: Typeface get() = if (customized) state.typeface else Typeface.System

    /** Every type size ×, 0.85–1.30. */
    val textScale: Float get() = if (customized) clamp(state.textScale, 0.85f, 1.3f, 1f) else 1f

    /** Spacing × 0.8, 1 or 1.2. */
    val densityScale: Float get() = if (!customized) 1f else when (state.density) {
        Density.Compact -> 0.8f
        Density.Standard -> 1f
        Density.Roomy -> 1.2f
    }

    fun spacing(base: Float): Float = base * densityScale

    /** The lyrics' weight (CSS 100–900): [token] while not customized. */
    fun lyricsWeight(token: Int): Int = if (!customized) token else when (state.lyricsWeight) {
        LyricsWeight.Regular -> 400
        LyricsWeight.Medium -> 500
        LyricsWeight.Semibold -> 600
        LyricsWeight.Bold -> 700
        LyricsWeight.Heavy -> 800
    }

    val sungInk: SungInk get() = if (!customized) SungInk.Token else when (val h = state.lyricsHighlight) {
        LyricsHighlight.Artwork -> SungInk.Token
        LyricsHighlight.Accent -> SungInk.Accent
        is LyricsHighlight.Custom -> SungInk.Custom(h.rgb and 0xFFFFFF)
    }

    /** The soft light around the line being sung: its blur radius for a font of [size] (none while not customized). */
    fun glowRadius(size: Float): Float = if (customized) size * 0.35f * clamp(state.lyricsGlow, 0f, 1f, 0f) else 0f

    /** The places shown, in order, each once; all of them when none of the library's would be left. */
    val places: List<Place> get() {
        if (!customized) return Place.entries
        val places = state.sidebar.distinct()
        return if (places.any { it in LIBRARY }) places else Place.entries
    }

    /** Home's sections, in order, each once. */
    val home: List<HomeSection> get() = if (customized) state.home.distinct() else LookSnapshot.Default.home

    val nowPlaying: NowPlayingLayout get() = if (customized) state.nowPlaying else NowPlayingLayout.Stage

    val miniPlayer: MiniPlayerStyle get() = if (customized) state.miniPlayer else MiniPlayerStyle.Bar

    companion object {
        val Luminous = LookStyle(LookSnapshot.Default)

        /** The places that are the library's (one always stays, ADR-0020). */
        val LIBRARY = setOf(Place.Home, Place.Albums, Place.Artists, Place.Songs, Place.Playlists, Place.Favourites)

        private fun clamp(value: Float, low: Float, high: Float, otherwise: Float): Float =
            if (value.isFinite()) value.coerceIn(low, high) else otherwise
    }
}

/**
 * The look's places on Android (docs/ui.md, Allowed differences): the four bottom tabs stay; the
 * Library tab's switcher shows the library's places the look keeps, in its order.
 */
object LookPlaces {
    fun section(place: Place): org.jellyfesh.logic.nav.LibrarySection? = when (place) {
        Place.Albums -> org.jellyfesh.logic.nav.LibrarySection.Albums
        Place.Artists -> org.jellyfesh.logic.nav.LibrarySection.Artists
        Place.Songs -> org.jellyfesh.logic.nav.LibrarySection.Songs
        Place.Playlists -> org.jellyfesh.logic.nav.LibrarySection.Playlists
        Place.Favourites -> org.jellyfesh.logic.nav.LibrarySection.Favorites
        Place.Home, Place.Search, Place.Sources -> null
    }

    /** The switcher's sections: the look's library places in order; all of them when it keeps none. */
    fun librarySections(style: LookStyle): List<org.jellyfesh.logic.nav.LibrarySection> =
        style.places.mapNotNull(::section).ifEmpty { org.jellyfesh.logic.nav.LibrarySection.entries }

    /** The section the Library tab shows: the one chosen while the switcher has it, else its first. */
    fun shown(chosen: org.jellyfesh.logic.nav.LibrarySection, sections: List<org.jellyfesh.logic.nav.LibrarySection>): org.jellyfesh.logic.nav.LibrarySection =
        if (chosen in sections) chosen else sections.firstOrNull() ?: chosen

    /** Home's summary (the library's size) sits under the title when it comes first, as in v0.0.6. */
    fun summaryUnderTitle(style: LookStyle): Boolean = style.home.firstOrNull() == HomeSection.Summary
}
