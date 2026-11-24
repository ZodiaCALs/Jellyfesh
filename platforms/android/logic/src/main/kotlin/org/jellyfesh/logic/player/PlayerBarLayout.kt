package org.jellyfesh.logic.player

/** What the full player bar shows besides the quality badge, the mini player button and More. */
data class PlayerBarTools(
    /** The volume's line beside its mute button. */
    val volumeLine: Boolean,
    /** Lyrics and Queue (Now Playing has both). */
    val paneButtons: Boolean,
    /** The mute button, with its line when [volumeLine]. */
    val volume: Boolean,
)

/**
 * The full player bar's measures (docs/ui.md, "The player bar and the mini player"), as Jellyfesh
 * Web's grid and container queries set them: the title column at least 112 dp, the transport 232
 * to 340 dp, the tools as wide as they need. A narrower bar keeps the mute button and drops the
 * volume's line (under 1000 dp), then Lyrics and Queue (under 760), then the mute button (under
 * 640), so the title keeps room to be read.
 */
object PlayerBarLayout {
    const val START_MIN_DP = 112f
    const val MIDDLE_MIN_DP = 232f
    const val MIDDLE_MAX_DP = 340f

    /** The volume with its line, as wide as the web's. */
    const val VOLUME_DP = 132f
    const val VOLUME_LINE_MIN_DP = 1000f
    const val PANE_BUTTONS_MIN_DP = 760f
    const val VOLUME_MIN_DP = 640f

    /** What a bar [barWidthDp] wide shows. */
    fun tools(barWidthDp: Float): PlayerBarTools = PlayerBarTools(
        volumeLine = barWidthDp >= VOLUME_LINE_MIN_DP,
        paneButtons = barWidthDp >= PANE_BUTTONS_MIN_DP,
        volume = barWidthDp >= VOLUME_MIN_DP,
    )

    /**
     * The transport's width, from what the tools leave of the bar ([freeDp]): as wide as it may be
     * while the title keeps its least, and never under its own least.
     */
    fun middleWidth(freeDp: Float): Float = (freeDp - START_MIN_DP).coerceIn(MIDDLE_MIN_DP, MIDDLE_MAX_DP)
}
