package org.jellyfesh.android.ui.player

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LyricsAlign
import org.jellyfesh.logic.core.LyricsSize
import org.jellyfesh.logic.core.LyricsSnapshot
import org.jellyfesh.logic.core.TextDir
import org.jellyfesh.logic.core.Timing
import org.jellyfesh.logic.lyrics.Box
import org.jellyfesh.logic.lyrics.LyricRow
import org.jellyfesh.logic.lyrics.LyricsFollow
import org.jellyfesh.logic.lyrics.LyricsLayout
import org.jellyfesh.logic.lyrics.SweepGeometry
import org.jellyfesh.logic.player.LyricLineRole
import org.jellyfesh.logic.player.NowPlayingMotion
import org.jellyfesh.logic.theme.Luminous
import org.jellyfesh.logic.time.LyricCursor
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.tokens.Tokens

/**
 * The lyrics of the track playing (docs/lyrics-model.md, "Display behaviour"; Luminous in
 * v0.0.6): bold lines up to 40 sp (lyrics.line, scaled for phones as its fluid range allows), the
 * active line in ink-90 at full size, lines still to come in lyrics.ink-inactive (ink-50) and sung
 * ones in ink-30, both at 92 % from their start edge, the karaoke sweep filling words linearly as
 * they are sung. The active line sits at 38 % of the view and moves there in 380 ms (Fluid), each
 * following line starting 40 ms after the one above so they settle in a cascade; scrolling by
 * hand pauses that for 4 s. The Lyrics size and alignment settings apply
 * live. Under reduced motion there is no sweep, no cascade and no scrolling animation: the
 * active line lights whole and the view jumps.
 *
 * Time costs nothing between changes: the view registers the lyric timeline with the ticker, which
 * wakes at its next change, and every frame only while a word is being sung, the view is visible
 * and playback runs. Only the active line reads the position, in its draw phase.
 */
@Composable
fun LyricsView(
    lyrics: LyricsSnapshot?,
    showTranslation: Boolean,
    showRomanization: Boolean,
    playing: Boolean,
    modifier: Modifier = Modifier,
    size: LyricsSize = LyricsSize.Medium,
    align: LyricsAlign = LyricsAlign.Start,
) {
    val doc = lyrics?.doc
    if (doc == null || doc.lines.isEmpty()) {
        EmptyState(R.drawable.ic_lyrics, stringResource(R.string.lyrics_none_title), stringResource(R.string.lyrics_none_body), modifier)
        return
    }
    val graph = LocalGraph.current
    val time = LocalPlaybackTime.current
    val effects = Jf.effects
    val rows = remember(doc, showTranslation, showRomanization) { LyricsLayout.rows(doc, showTranslation, showRomanization) }
    val timeline = lyrics.timeline?.takeIf { !it.isEmpty && doc.timing != Timing.Unsynced }
    val reduce = effects.systemReducesMotion
    val sweeping = timeline != null && playing && !reduce && doc.timing == Timing.Word
    TimeDemandEffect(timeline?.let { TimeDemand.Lyrics(it, sweeping) })

    val cursor: State<LyricCursor> = remember(timeline) {
        derivedStateOf { timeline?.cursor(Math.floorDiv(time.positionUs, 1_000L)) ?: LyricCursor.None }
    }
    val active by remember(cursor) { derivedStateOf { cursor.value.line } }
    val listState = rememberLazyListState()
    val follow = remember(lyrics.uid) { LyricsFollow() }
    var cascade by remember { mutableStateOf<Cascade?>(null) }
    LaunchedEffect(listState) {
        listState.interactionSource.interactions.collect { if (it is DragInteraction.Start) follow.userScrolled(System.currentTimeMillis()) }
    }

    BoxWithConstraints(modifier.fillMaxSize()) {
        val viewport = with(LocalDensity.current) { maxHeight.toPx() }
        val anchor = viewport * Tokens.Lyrics.anchorPosition
        val scale = NowPlayingMotion.lyricsScale(size)
        val fontSize = lyricSize(maxWidth.value) * scale
        LaunchedEffect(active, viewport) {
            val line = active ?: return@LaunchedEffect
            val wait = follow.resumesInMs(System.currentTimeMillis())
            if (wait > 0) delay(wait)
            follow.reset()
            scrollToAnchor(listState, line, anchor, animate = !effects.reduceMotion) { distance ->
                if (distance > 0f) cascade = Cascade((cascade?.serial ?: 0) + 1, distance, line)
            }
        }
        val listLabel = stringResource(R.string.np_lyrics)
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().semantics { contentDescription = listLabel },
            contentPadding = PaddingValues(
                start = 24.dp,
                end = 24.dp,
                top = with(LocalDensity.current) { anchor.toDp() } - 24.dp,
                bottom = with(LocalDensity.current) { (viewport - anchor).toDp() },
            ),
            verticalArrangement = Arrangement.spacedBy(Tokens.Lyrics.lineGap.dp),
        ) {
            if (timeline == null) {
                item(key = "unsynced") {
                    JfText(stringResource(R.string.lyrics_unsynced), style = JfType.caption, color = Jf.colors.ink50)
                }
            }
            itemsIndexed(rows, key = { i, _ -> i }) { i, row ->
                val isActive = timeline != null && i == active
                LyricLine(
                    row = row,
                    role = if (timeline == null) LyricLineRole.Active else NowPlayingMotion.role(i, active),
                    synced = timeline != null,
                    cursor = if (isActive && !reduce) cursor else null,
                    fontSize = fontSize,
                    scale = scale,
                    centred = align == LyricsAlign.Center,
                    lag = cascade?.takeIf { i > it.active && !effects.reduceMotion }?.let { it to i },
                    onClick = row.startMs?.takeIf { timeline != null }?.let { start -> { graph.send(Command.Seek(start)) } },
                )
            }
        }
    }
}

/** Lyric size: fluid from 28 to 40 sp with the view's width (Tokens.Lyrics.lineFluid, Luminous). */
private fun lyricSize(widthDp: Float): androidx.compose.ui.unit.TextUnit {
    val fluid = Tokens.Lyrics.lineFluid
    return Luminous.lyricLineSp(widthDp, fluid.min, fluid.max).sp
}

/** A line change's scroll: lines below [active] follow [distance] (px) one after another (the cascade). */
private data class Cascade(val serial: Int, val distance: Float, val active: Int)

/**
 * Puts line [index] with its centre at [anchorPx] of the view, in 380 ms (Fluid) or at once;
 * [onAnimate] hears the distance of an animated scroll just before it starts.
 */
private suspend fun scrollToAnchor(state: LazyListState, index: Int, anchorPx: Float, animate: Boolean, onAnimate: (Float) -> Unit) {
    // Item offsets count from the end of the top content padding; viewportStartOffset is minus
    // that padding, so this is the item's centre in the view's own coordinates.
    fun delta(): Float? {
        val info = state.layoutInfo
        return info.visibleItemsInfo.firstOrNull { it.key == index }?.let { it.offset - info.viewportStartOffset + it.size / 2f - anchorPx }
    }
    var d = delta()
    if (d == null) {
        state.scrollToItem(index)
        d = delta() ?: return
        state.scrollBy(d)
        return
    }
    if (kotlin.math.abs(d) < 1f) return
    if (animate) {
        onAnimate(d)
        state.animateScrollBy(d, tween(Tokens.Motion.dur3Ms, easing = JfMotion.Fluid))
    } else {
        state.scrollBy(d)
    }
}

/**
 * One lyric line: its ink and scale from its [role], drawn lower by the cascade's lag while the
 * list scrolls ([lag]: the change and this line's index), scaled from its start edge (or centre).
 */
@Composable
private fun LyricLine(
    row: LyricRow,
    role: LyricLineRole,
    synced: Boolean,
    cursor: State<LyricCursor>?,
    fontSize: androidx.compose.ui.unit.TextUnit,
    scale: Float,
    centred: Boolean,
    lag: Pair<Cascade, Int>?,
    onClick: (() -> Unit)?,
) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    val active = role == LyricLineRole.Active
    val dir = when (row.direction) {
        TextDir.Ltr -> TextDirection.Ltr
        TextDir.Rtl -> TextDirection.Rtl
        TextDir.Auto -> TextDirection.Content
    }
    val textAlign = if (centred) TextAlign.Center else TextAlign.Start
    val base = JfType.lyricLine(fontSize, row.cjk).copy(textDirection = dir, textAlign = textAlign)
    val inactive = if (role == LyricLineRole.Sung) colors.ink30 else colors.ink50
    val lineColor = if (!synced || active) colors.ink90 else inactive
    val lineScale by animateFloatAsState(
        if (synced) NowPlayingMotion.lineScale(role) else 1f,
        JfMotion.fluid(Tokens.Motion.dur3Ms, reduce),
        label = "line-scale",
    )
    val offset = remember { Animatable(0f) }
    LaunchedEffect(lag?.first?.serial) {
        val (change, index) = lag ?: return@LaunchedEffect
        val delayMs = NowPlayingMotion.cascadeDelayMs(index, change.active)
        val total = Tokens.Motion.dur3Ms + delayMs
        val frames = NowPlayingMotion.cascadeLag(change.distance, delayMs, Tokens.Motion.dur3Ms, JfMotion.Fluid::transform)
        offset.snapTo(0f)
        offset.animateTo(
            0f,
            keyframes {
                durationMillis = total
                frames.forEach { f -> f.offset at (f.progress * total).toInt() }
            },
        )
    }
    val rtl = row.direction == TextDir.Rtl || LocalLayoutDirection.current == LayoutDirection.Rtl && row.direction != TextDir.Ltr
    val originX = if (centred) 0.5f else if (rtl) 1f else 0f
    Column(
        Modifier
            .fillMaxWidth()
            .graphicsLayer {
                translationY = offset.value
                scaleX = lineScale
                scaleY = lineScale
                transformOrigin = TransformOrigin(originX, 0.5f)
            }
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .semantics { selected = active },
        horizontalAlignment = if (centred) Alignment.CenterHorizontally else Alignment.Start,
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        when {
            row.isGap -> BasicText("• • •", style = base.copy(color = if (active) colors.ink70 else inactive))
            active && cursor != null && row.wordRanges.isNotEmpty() ->
                SweepText(
                    row,
                    { LyricsLayout.sweep(row, cursor.value, reduceMotion = false) },
                    base,
                    sung = colors.ink90,
                    unsung = colors.ink70,
                    rtl = row.direction == TextDir.Rtl || LocalLayoutDirection.current == LayoutDirection.Rtl && row.direction == TextDir.Auto,
                )
            else -> BasicText(row.text, style = base.copy(color = lineColor))
        }
        val aux = if (active || !synced) colors.ink70 else inactive
        val translation = JfType.lyricTranslation.let { it.copy(fontSize = it.fontSize * scale, color = aux, textDirection = TextDirection.Content, textAlign = textAlign) }
        val romanization = JfType.lyricRomanization.let { it.copy(fontSize = it.fontSize * scale, color = aux, textAlign = textAlign) }
        row.translation?.let { BasicText(it, style = translation) }
        row.romanization?.let { BasicText(it, style = romanization) }
        row.background?.let { BasicText(it, style = romanization.copy(color = if (active) colors.ink70 else inactive)) }
    }
}

/**
 * A line being sung: drawn in the unsung ink, then again in the sung ink clipped to the words
 * already sung plus the sung part of the current word. Reads [sweep] in the draw phase, so a
 * moving sweep redraws this one line and recomposes nothing (Now Playing's active line, and the
 * lyrics bar's).
 */
@Composable
internal fun SweepText(
    row: LyricRow,
    sweep: () -> org.jellyfesh.logic.lyrics.Sweep,
    style: TextStyle,
    sung: Color,
    unsung: Color,
    rtl: Boolean,
    modifier: Modifier = Modifier,
    maxLines: Int = Int.MAX_VALUE,
) {
    val layout = remember(row) { mutableStateOf<TextLayoutResult?>(null) }
    BasicText(
        row.text,
        style = style.copy(color = unsung),
        onTextLayout = { layout.value = it },
        maxLines = maxLines,
        overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        modifier = modifier.drawWithContent {
            drawContent()
            val l = layout.value ?: return@drawWithContent
            val sweep = sweep()
            val path = Path()
            if (sweep.sungEnd > 0) path.addPath(l.getPathForRange(0, sweep.sungEnd.coerceAtMost(row.text.length)))
            val current = sweep.current
            if (current != null && sweep.progress > 0f && current.first < row.text.length) {
                val boxes = (current.first..minOf(current.last, row.text.length - 1)).map { i ->
                    val r = l.getBoundingBox(i)
                    Box(r.left, r.top, r.right, r.bottom)
                }
                SweepGeometry.lit(SweepGeometry.rows(boxes), sweep.progress, rtl).forEach { b ->
                    path.addRect(Rect(b.left, b.top, b.right, b.bottom))
                }
            }
            clipPath(path) { drawText(l, color = sung) }
        },
    )
}
