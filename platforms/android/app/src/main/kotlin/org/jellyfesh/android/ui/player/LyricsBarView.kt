package org.jellyfesh.android.ui.player

import org.jellyfesh.android.ui.theme.sungInk
import org.jellyfesh.android.ui.theme.fontFamily
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.layout.MeasureResult
import androidx.compose.ui.layout.MeasureScope
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.LayoutModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.node.invalidateMeasurement
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.logic.core.AuxKind
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.lyrics.LyricsBars
import org.jellyfesh.logic.lyrics.LyricsLayout
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.android.ui.theme.rounded
import org.jellyfesh.tokens.Tokens

/** The lyrics bar's height: one line, or two with the line's translation. */
fun lyricsBarHeight(withTranslation: Boolean): Dp = if (withTranslation) 58.dp else 42.dp

/**
 * One line that may be wider than its column (the bar's sung line; docs/ui.md, "The player bar",
 * v0.0.9): laid out at its own width, it shows its start (the right end of a right-to-left line,
 * [rtl]) and, when it is too long, fades out at the other edge instead of ending with "…": over
 * its last [fade], or its last 40 % in a narrow column, as Jellyfesh Web's mask. Nothing of it
 * draws outside the column, so it never reaches the controls or the times.
 */
internal fun Modifier.fadeOverflow(rtl: Boolean, fade: Dp = Tokens.Space.s7.dp): Modifier = this then FadeOverflowElement(rtl, fade)

/** The most of a narrow column the fade takes. */
private const val MAX_FADE_FRACTION = 0.4f

private data class FadeOverflowElement(val rtl: Boolean, val fade: Dp) : ModifierNodeElement<FadeOverflowNode>() {
    override fun create(): FadeOverflowNode = FadeOverflowNode(rtl, fade)

    override fun update(node: FadeOverflowNode) {
        if (node.rtl != rtl) {
            node.rtl = rtl
            node.invalidateMeasurement()
        }
        node.fade = fade
        node.invalidateDraw()
    }
}

private class FadeOverflowNode(var rtl: Boolean, var fade: Dp) : Modifier.Node(), LayoutModifierNode, DrawModifierNode {
    private var overflows = false

    override fun MeasureScope.measure(measurable: Measurable, constraints: Constraints): MeasureResult {
        val line = measurable.measure(constraints.copy(minWidth = 0, maxWidth = Constraints.Infinity))
        val width = if (constraints.hasBoundedWidth) line.width.coerceIn(constraints.minWidth, constraints.maxWidth) else maxOf(line.width, constraints.minWidth)
        val over = line.width > width
        if (over != overflows) {
            overflows = over
            invalidateDraw()
        }
        return layout(width, line.height) {
            // By the line's own direction: its start stays in view.
            line.place(if (rtl) width - line.width else 0, 0)
        }
    }

    override fun ContentDrawScope.draw() {
        clipRect {
            if (!overflows || size.width <= 0f) {
                this@draw.drawContent()
                return@clipRect
            }
            val edge = (fade.toPx() / size.width).coerceIn(0f, MAX_FADE_FRACTION)
            val mask = if (rtl) {
                Brush.horizontalGradient(0f to Color.Transparent, edge to Color.Black, 1f to Color.Black)
            } else {
                Brush.horizontalGradient(0f to Color.Black, 1f - edge to Color.Black, 1f to Color.Transparent)
            }
            drawIntoCanvas { it.saveLayer(Rect(Offset.Zero, size), Paint()) }
            this@draw.drawContent()
            drawRect(mask, blendMode = BlendMode.DstIn)
            drawIntoCanvas { it.restore() }
        }
    }
}

/** Whether the lyrics bar shows a translation line under the current one. */
fun lyricsBarTranslates(state: CoreState): Boolean =
    state.settings.showTranslation && state.currentLyrics?.doc?.tracks?.any { it.kind == AuxKind.Translation } == true

/**
 * The lyrics bar above the mini-player (docs/ui.md, "Lyrics, word by word"; ADR-0022 §6): the
 * line being sung, filling word by word in the highlight colour as the core's `lyrics_bar` says
 * (ported, [LyricsBars], so it runs every frame without asking the core), and its translation
 * when Show Translations is on. Before the first line it shows the one to come; in a gap, the last
 * one sung. A line-timed line lights whole, as in Now Playing, and under reduced motion every line
 * does. Choosing it opens Now Playing's lyrics. Tint only (no live blur): the tab bar and the
 * mini-player are the screen's two blur layers.
 */
@Composable
fun LyricsBarStrip(state: CoreState, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val lyrics = state.currentLyrics ?: return
    val doc = lyrics.doc ?: return
    val timeline = lyrics.timeline ?: return
    val colors = Jf.colors
    val time = LocalPlaybackTime.current
    val reduce = Jf.effects.systemReducesMotion
    val words = doc.timing == Timing.Word
    val playing = PlayerModel.isPlaying(state.playback.status)
    TimeDemandEffect(TimeDemand.Lyrics(timeline, sweep = words && playing && !reduce))
    val translates = lyricsBarTranslates(state)
    val rows = remember(doc, translates) { LyricsLayout.rows(doc, translates, false) }
    // Recomposes only when the line changes; the sweep is read while drawing.
    val line by remember(timeline) { derivedStateOf { LyricsBars.shownLine(timeline.bar(Math.floorDiv(time.positionUs, 1_000L))) } }
    val row = line?.let { rows.getOrNull(it) }
    val lyricsLabel = stringResource(R.string.np_lyrics)
    val shape = rounded(lyricsBarHeight(translates) / 2)
    GlassSurface(
        modifier
            .fillMaxWidth()
            .height(lyricsBarHeight(translates))
            .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button, onClick = onOpen)
            .semantics { contentDescription = listOfNotNull(lyricsLabel, row?.text).joinToString(", ") },
        grade = GlassGrade.Veil,
        shape = shape,
        blur = false,
        floating = true,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).align(Alignment.Center),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(1.dp, Alignment.CenterVertically),
        ) {
            if (row == null || row.isGap) {
                BasicText("• • •", style = JfType.listTitle.copy(color = colors.ink50))
            } else {
                // The look's weight and face; its highlight colours the sung words (the accent in Luminous).
                val look = Jf.look
                val style = JfType.listTitle.copy(
                    fontSize = 16.sp,
                    fontWeight = androidx.compose.ui.text.font.FontWeight(look.lyricsWeight(700)),
                    fontFamily = look.fontFamily(),
                    textAlign = TextAlign.Center,
                )
                val sung = if (look.sungInk == org.jellyfesh.logic.theme.SungInk.Token) colors.accentText else sungInk()
                val rtl = row.direction == TextDir.Rtl || LocalLayoutDirection.current == LayoutDirection.Rtl && row.direction == TextDir.Auto
                SweepText(
                    row,
                    { LyricsLayout.barSweep(row, timeline.bar(Math.floorDiv(time.positionUs, 1_000L)), reduceMotion = reduce || !words) },
                    style,
                    sung = sung,
                    unsung = colors.ink70,
                    rtl = rtl,
                    fadesOut = true,
                )
                row.translation?.let {
                    BasicText(it, style = JfType.caption.copy(color = colors.ink70, textAlign = TextAlign.Center), maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/**
 * The lyrics bar's line inside the full player bar (docs/ui.md, "The player bar and the mini
 * player"): under the song's title instead of its artist, as on Jellyfesh Web, the line filling
 * word by word in the highlight colour with its translation when Show Translations is on. Phones
 * show the strip above the capsule instead ([LyricsBarStrip]).
 */
@Composable
fun BarLyricsLine(state: CoreState, modifier: Modifier = Modifier) {
    val lyrics = state.currentLyrics ?: return
    val doc = lyrics.doc ?: return
    val timeline = lyrics.timeline ?: return
    val colors = Jf.colors
    val time = LocalPlaybackTime.current
    val reduce = Jf.effects.systemReducesMotion
    val words = doc.timing == Timing.Word
    val playing = PlayerModel.isPlaying(state.playback.status)
    TimeDemandEffect(TimeDemand.Lyrics(timeline, sweep = words && playing && !reduce))
    val translates = lyricsBarTranslates(state)
    val rows = remember(doc, translates) { LyricsLayout.rows(doc, translates, false) }
    val line by remember(timeline) { derivedStateOf { LyricsBars.shownLine(timeline.bar(Math.floorDiv(time.positionUs, 1_000L))) } }
    val row = line?.let { rows.getOrNull(it) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(1.dp)) {
        if (row == null || row.isGap) {
            BasicText("• • •", style = JfType.listSecondary.copy(color = colors.ink50), maxLines = 1)
        } else {
            val look = Jf.look
            val style = JfType.listSecondary.copy(
                fontSize = 13.sp,
                fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold,
                fontFamily = look.fontFamily(),
            )
            val sung = if (look.sungInk == org.jellyfesh.logic.theme.SungInk.Token) colors.accentText else sungInk()
            val rtl = row.direction == TextDir.Rtl || LocalLayoutDirection.current == LayoutDirection.Rtl && row.direction == TextDir.Auto
            SweepText(
                row,
                { LyricsLayout.barSweep(row, timeline.bar(Math.floorDiv(time.positionUs, 1_000L)), reduceMotion = reduce || !words) },
                style,
                sung = sung,
                unsung = colors.ink70,
                rtl = rtl,
                fadesOut = true,
            )
            row.translation?.let {
                BasicText(it, style = JfType.caption.copy(fontSize = 11.sp, color = colors.ink70), maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/**
 * The look's Artwork layout (ADR-0020): the line being sung under the cover, large, filling word
 * by word in the highlight colour, with its translation when Show Translations is on; nothing
 * without timed lyrics.
 */
@Composable
fun SungLine(state: CoreState, modifier: Modifier = Modifier) {
    val lyrics = state.currentLyrics ?: return
    val doc = lyrics.doc ?: return
    val timeline = lyrics.timeline?.takeIf { !it.isEmpty && doc.timing != Timing.Unsynced } ?: return
    val colors = Jf.colors
    val time = LocalPlaybackTime.current
    val reduce = Jf.effects.systemReducesMotion
    val words = doc.timing == Timing.Word
    val playing = PlayerModel.isPlaying(state.playback.status)
    TimeDemandEffect(TimeDemand.Lyrics(timeline, sweep = words && playing && !reduce))
    val translates = state.settings.showTranslation
    val rows = remember(doc, translates) { LyricsLayout.rows(doc, translates, false) }
    val line by remember(timeline) { derivedStateOf { LyricsBars.shownLine(timeline.bar(Math.floorDiv(time.positionUs, 1_000L))) } }
    val row = line?.let { rows.getOrNull(it) } ?: return
    val look = Jf.look
    val style = JfType.lyricLine(26.sp, row.cjk).copy(
        fontWeight = androidx.compose.ui.text.font.FontWeight(look.lyricsWeight(Tokens.Lyrics.line.weight)),
        fontFamily = look.fontFamily(),
        textAlign = TextAlign.Center,
    )
    Column(modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (row.isGap) {
            BasicText("• • •", style = style.copy(color = colors.ink50))
        } else {
            val rtl = row.direction == TextDir.Rtl || LocalLayoutDirection.current == LayoutDirection.Rtl && row.direction == TextDir.Auto
            SweepText(
                row,
                { LyricsLayout.barSweep(row, timeline.bar(Math.floorDiv(time.positionUs, 1_000L)), reduceMotion = reduce || !words) },
                style,
                sung = sungInk(),
                unsung = colors.ink70,
                rtl = rtl,
                maxLines = 2,
            )
            row.translation?.let {
                BasicText(it, style = JfType.small.copy(color = colors.ink70, textAlign = TextAlign.Center), maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
