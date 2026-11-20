package org.jellyfesh.logic.player

import org.jellyfesh.logic.core.MiniPlayerStyle
import org.jellyfesh.logic.core.NowPlayingLayout

/**
 * The panes of Now Playing: beside the artwork on wide windows, or one at a time on phones.
 * Comments only for a song from a source that shows them (v0.0.7).
 */
enum class NowPlayingPanel { Lyrics, Queue, Info, Comments }

/** How Now Playing arranges itself for the look's layout (ADR-0020), the window and the chosen pane. */
sealed interface NowPlayingArrangement {
    /** Phones' stage without a pane: the artwork, the title and the dock. */
    data object Hero : NowPlayingArrangement

    /** Wide windows' stage: the artwork and the title beside the pane. */
    data class Side(val panel: NowPlayingPanel) : NowPlayingArrangement

    /** A pane over the dock, the title in the dock (phones). */
    data class Pane(val panel: NowPlayingPanel) : NowPlayingArrangement

    /** The Artwork layout: a large cover with the line being sung below it. */
    data object CoverAndLine : NowPlayingArrangement

    /** The Lyrics layout: the pane across the screen, a small cover and the title in its corner. */
    data class Across(val panel: NowPlayingPanel) : NowPlayingArrangement
}

/** The player at the bottom of the library screens. */
enum class MiniPlayerKind {
    /** The player bar (wide windows). */
    Bar,

    /** The mini-player pill: cover, title and Play. */
    Mini,

    /** A smaller floating capsule at the trailing side. */
    Floating,
}

object NowPlayingLayouts {
    /**
     * Stage is v0.0.6. Artwork shows a large cover with the sung line where the Lyrics pane was;
     * phones still open each pane with its button, and wide windows keep Queue and Info beside the
     * cover. Lyrics puts the lyrics (and on wide windows any pane) across the screen with a small
     * cover in the corner; phones open Queue and Info as panes.
     */
    fun arrangement(layout: NowPlayingLayout, wide: Boolean, panel: NowPlayingPanel?): NowPlayingArrangement = when (layout) {
        NowPlayingLayout.Stage -> when {
            wide -> NowPlayingArrangement.Side(panel ?: NowPlayingPanel.Lyrics)
            panel == null -> NowPlayingArrangement.Hero
            else -> NowPlayingArrangement.Pane(panel)
        }
        NowPlayingLayout.Artwork -> when {
            panel == null -> NowPlayingArrangement.CoverAndLine
            wide && panel == NowPlayingPanel.Lyrics -> NowPlayingArrangement.CoverAndLine
            wide -> NowPlayingArrangement.Side(panel)
            else -> NowPlayingArrangement.Pane(panel)
        }
        NowPlayingLayout.Lyrics -> when {
            wide -> NowPlayingArrangement.Across(panel ?: NowPlayingPanel.Lyrics)
            panel == null || panel == NowPlayingPanel.Lyrics -> NowPlayingArrangement.Across(NowPlayingPanel.Lyrics)
            else -> NowPlayingArrangement.Pane(panel)
        }
    }

    /** The pane button shown as chosen: the pane on screen, if any. */
    fun chosen(arrangement: NowPlayingArrangement): NowPlayingPanel? = when (arrangement) {
        NowPlayingArrangement.Hero, NowPlayingArrangement.CoverAndLine -> null
        is NowPlayingArrangement.Side -> arrangement.panel
        is NowPlayingArrangement.Pane -> arrangement.panel
        is NowPlayingArrangement.Across -> arrangement.panel
    }

    /** Bar is v0.0.6 (the player bar on wide windows, the pill on phones); Compact is the pill everywhere; Floating the small capsule. */
    fun miniPlayer(style: MiniPlayerStyle, wide: Boolean): MiniPlayerKind = when (style) {
        MiniPlayerStyle.Bar -> if (wide) MiniPlayerKind.Bar else MiniPlayerKind.Mini
        MiniPlayerStyle.Compact -> MiniPlayerKind.Mini
        MiniPlayerStyle.Floating -> MiniPlayerKind.Floating
    }
}
