package org.jellyfesh.android.ui.theme

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.logic.seek.SeekBarState
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.seek.SeekKey
import org.jellyfesh.tokens.Tokens
import kotlin.math.roundToInt

// Control style D as built in v0.0.4 (docs/design-controls.md, "v0.0.4: the controls as built";
// every bar and button style lives in this one file): Apple's glass grouping for transport, a
// straight seek line with inline times (Media Player's ringed thumb with a mouse), Misty's
// segmented control, accent-soft circles for toggles. Touch targets are at least 48 dp (PLT-09);
// presses scale to 92 % with the Spring curve; the focus ring is Misty's (2 dp field, 4 dp
// accent). Exactly one accent fill per view (COL-04): the play button on Now Playing.

/** The minimum touch target on Android (Misty PLT-09). */
val TouchTarget: Dp = 48.dp

/** Scales a control to 92 % while pressed (Spring, dur-0); instant under reduced motion. */
@Composable
fun Modifier.pressScale(interaction: MutableInteractionSource, pressedScale: Float = 0.92f): Modifier {
    val pressed by interaction.collectIsPressedAsState()
    val reduce = Jf.effects.reduceMotion
    val scale by animateFloatAsState(
        if (pressed) pressedScale else 1f,
        JfMotion.spring(Tokens.Motion.dur0Ms, reduce),
        label = "press",
    )
    return graphicsLayer {
        scaleX = scale
        scaleY = scale
    }
}

/** Misty's focus ring for keyboards, D-pads and switch access: 2 dp of field, then 4 dp of accent. */
@Composable
fun Modifier.focusRing(interaction: MutableInteractionSource, shape: androidx.compose.ui.graphics.Shape = CircleShape): Modifier {
    val focused by interaction.collectIsFocusedAsState()
    if (!focused) return this
    val colors = Jf.colors
    return border(4.dp, colors.accentText, shape).padding(2.dp).border(2.dp, colors.sky, shape)
}

/** A glyph button: transparent at rest, a soft fill and 92 % while pressed. */
@Composable
fun JfIconButton(
    icon: Int,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = Jf.colors.ink70,
    size: Dp = 44.dp,
    iconSize: Dp = 22.dp,
    enabled: Boolean = true,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val colors = Jf.colors
    Box(
        modifier
            .size(maxOf(size, TouchTarget))
            .focusRing(interaction)
            .clickable(interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(size)
                .pressScale(interaction)
                .background(if (pressed) colors.fillPress else Color.Transparent, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            JfIcon(icon, if (enabled) tint else tint.copy(alpha = tint.alpha * 0.4f), size = iconSize)
        }
    }
}

/** A toggle (shuffle, repeat, lyrics): an accent-soft circle with an accent glyph when on. */
@Composable
fun JfToggleIconButton(
    icon: Int,
    description: String,
    stateText: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 44.dp,
) {
    val interaction = remember { MutableInteractionSource() }
    val colors = Jf.colors
    Box(
        modifier
            .size(maxOf(size, TouchTarget))
            .focusRing(interaction)
            .toggleable(checked, interaction, indication = null, role = Role.Switch, onValueChange = onToggle)
            .semantics {
                contentDescription = description
                stateDescription = stateText
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(size)
                .pressScale(interaction)
                .background(if (checked) colors.accentSoft else Color.Transparent, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            JfIcon(icon, if (checked) colors.accentText else colors.ink70, size = 22.dp)
        }
    }
}

/** The one accent action of Now Playing: an accent circle with a filled play or pause glyph. */
@Composable
fun PlayPauseButton(playing: Boolean, description: String, onClick: () -> Unit, modifier: Modifier = Modifier, size: Dp = 60.dp) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val colors = Jf.colors
    Box(
        modifier
            .size(size)
            .focusRing(interaction)
            .pressScale(interaction)
            .background(if (pressed) colors.accentPress else colors.accent, CircleShape)
            .clickable(interaction, indication = null, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        PlayPauseGlyph(playing, colors.onAccent, size * 0.43f)
    }
}

/**
 * Play and pause morph into each other (docs/design-controls.md, "Now Playing"): the glyph that
 * goes fades and shrinks while the one that comes fades in and springs to size, dur-2 on the
 * Spring curve. Reduced motion swaps them.
 */
@Composable
fun PlayPauseGlyph(playing: Boolean, tint: Color, size: Dp) {
    val reduce = Jf.effects.reduceMotion
    AnimatedContent(
        targetState = playing,
        transitionSpec = {
            if (reduce) {
                fadeIn(tween(0)) togetherWith fadeOut(tween(0))
            } else {
                (fadeIn(JfMotion.spring(Tokens.Motion.dur2Ms, false)) + scaleIn(JfMotion.spring(Tokens.Motion.dur2Ms, false), initialScale = 0.6f)) togetherWith
                    (fadeOut(JfMotion.fluid(Tokens.Motion.dur1Ms, false)) + scaleOut(JfMotion.fluid(Tokens.Motion.dur1Ms, false), targetScale = 0.6f))
            }
        },
        contentAlignment = Alignment.Center,
        label = "play-pause",
    ) { isPlaying ->
        JfIcon(if (isPlaying) R.drawable.ic_pause else R.drawable.ic_play, tint, size = size)
    }
}

/** Previous, play and next grouped in one glass capsule (Controls D). */
@Composable
fun TransportCapsule(
    playing: Boolean,
    playDescription: String,
    previousDescription: String,
    nextDescription: String,
    onPrevious: () -> Unit,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    modifier: Modifier = Modifier,
) {
    GlassSurface(modifier, grade = GlassGrade.Veil, shape = CircleShape, floating = true) {
        Row(Modifier.padding(6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            JfIconButton(R.drawable.ic_previous, previousDescription, onPrevious, tint = Jf.colors.ink90, size = 52.dp, iconSize = 24.dp)
            PlayPauseButton(playing, playDescription, onPlayPause)
            JfIconButton(R.drawable.ic_next, nextDescription, onNext, tint = Jf.colors.ink90, size = 52.dp, iconSize = 24.dp)
        }
    }
}

/**
 * The seek bar of v0.0.4 (docs/design-controls.md): a straight line with round caps and times at
 * both ends, the played part ink-90 over ink-30 (the caller passes `line-strong` on plain
 * surfaces). On touch it is 6 dp and thickens to 12 dp while dragged, with no thumb; with a mouse
 * it is 4 dp, 6 dp with a 14 dp ringed thumb on hover or focus, 8 dp while dragged (Media Player).
 * Thickness and thumb change over dur-1 Fluid; nothing loops: [fraction] is read in the draw phase
 * and the ticker asks for a redraw only when the fill moves by a device pixel. TalkBack reads it as
 * a slider whose value is [valueText] ("1:23 of 3:45"). It takes keyboard focus: Left and Right
 * (mirrored right to left) step 5 s, Page Up and Page Down 30 s, Home and End go to the ends, each
 * through [onStep].
 */
@Composable
fun SeekBar(
    fraction: () -> Float,
    enabled: Boolean,
    dragging: Boolean,
    valueText: String,
    description: String,
    onDragStart: (Float) -> Unit,
    onDrag: (Float) -> Unit,
    onDragEnd: () -> Unit,
    onDragCancel: () -> Unit,
    onSeekTo: (Float) -> Unit,
    modifier: Modifier = Modifier,
    onStep: (SeekKey) -> Unit = {},
    rest: Color = Jf.colors.ink30,
    onWidthPx: (Int) -> Unit = {},
    leading: (@Composable RowScope.() -> Unit)? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    // A mouse hovering is a pointer screen (desktop mode); fingers get the touch variant.
    val touch = !hovered
    val state = when {
        dragging -> SeekBarState.Drag
        hovered || focused -> SeekBarState.Hover
        else -> SeekBarState.Rest
    }
    val thickness by animateFloatAsState(SeekGeometry.thickness(state, touch), JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "track")
    val thumb by animateFloatAsState(if (SeekGeometry.showsThumb(state, touch) && enabled) 1f else 0f, JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "thumb")
    val played = colors.ink90
    val ring = colors.glassEdge
    val currentFraction by rememberUpdatedState(fraction)
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        leading?.invoke(this)
        var widthPx by remember { mutableFloatStateOf(1f) }
        val density = androidx.compose.ui.platform.LocalDensity.current.density
        fun at(x: Float): Float = SeekGeometry.fractionAt(x / density, widthPx / density, touch, rtl)
        Box(
            Modifier
                .weight(1f)
                .height(TouchTarget)
                .onSizeChanged {
                    widthPx = it.width.toFloat().coerceAtLeast(1f)
                    onWidthPx(SeekGeometry.steps(widthPx, touch = true, density = density))
                }
                .hoverable(interaction)
                .focusRing(interaction, RoundedCornerShape(12.dp))
                .onKeyEvent { event ->
                    if (!enabled || event.type != KeyEventType.KeyDown) return@onKeyEvent false
                    val step = seekKey(event.key, rtl) ?: return@onKeyEvent false
                    onStep(step)
                    true
                }
                .focusable(enabled, interaction)
                .semantics {
                    contentDescription = description
                    stateDescription = valueText
                    progressBarRangeInfo = ProgressBarRangeInfo(currentFraction().coerceIn(0f, 1f), 0f..1f)
                    if (enabled) {
                        setProgress { target ->
                            onSeekTo(target.coerceIn(0f, 1f))
                            true
                        }
                    }
                }
                .then(
                    if (!enabled) {
                        Modifier
                    } else {
                        Modifier
                            .pointerInput(touch) {
                                detectTapGestures { onSeekTo(at(it.x)) }
                            }
                            .pointerInput(touch) {
                                detectHorizontalDragGestures(
                                    onDragStart = { onDragStart(at(it.x)) },
                                    onDragEnd = onDragEnd,
                                    onDragCancel = onDragCancel,
                                    onHorizontalDrag = { change, _ -> onDrag(at(change.position.x)) },
                                )
                            }
                    },
                ),
            contentAlignment = Alignment.Center,
        ) {
            Canvas(Modifier.fillMaxWidth().height(24.dp)) {
                val f = currentFraction().coerceIn(0f, 1f)
                drawSeekLine(f, thickness.dp.toPx(), SeekGeometry.inset(touch).dp.toPx(), rtl, played, rest)
                if (thumb > 0f) {
                    val x = SeekGeometry.thumbCentre(f, size.width / density, touch, rtl).dp.toPx()
                    drawHoverThumb(x, (SeekGeometry.THUMB / 2f).dp.toPx() * (0.6f + 0.4f * thumb), thumb, played, ring)
                }
            }
        }
        trailing?.invoke(this)
    }
}

/** One keyboard step of the volume line. */
private const val VOLUME_STEP = 0.05f

/** The seek bar's keys: Left and Right (mirrored right to left), Page Down and Page Up, Home and End. */
private fun seekKey(key: Key, rtl: Boolean): SeekKey? = when (key) {
    Key.DirectionLeft -> if (rtl) SeekKey.Forward else SeekKey.Back
    Key.DirectionRight -> if (rtl) SeekKey.Back else SeekKey.Forward
    Key.PageDown -> SeekKey.PageBack
    Key.PageUp -> SeekKey.PageForward
    Key.MoveHome -> SeekKey.Start
    Key.MoveEnd -> SeekKey.End
    else -> null
}

/** The straight line: the whole track in [rest], the played part in [played], round caps. */
private fun DrawScope.drawSeekLine(f: Float, thickness: Float, inset: Float, rtl: Boolean, played: Color, rest: Color) {
    val cy = size.height / 2f
    val length = (size.width - 2 * inset).coerceAtLeast(0f)
    drawLine(rest, Offset(inset, cy), Offset(inset + length, cy), thickness, StrokeCap.Round)
    val end = f * length
    if (end <= 0f) return
    val from = if (rtl) size.width - inset else inset
    val to = if (rtl) from - end else from + end
    drawLine(played, Offset(from, cy), Offset(to, cy), thickness, StrokeCap.Round)
}

/** The hover thumb: ink-90 with a glass-edge ring and a soft shadow (shadow-1). */
private fun DrawScope.drawHoverThumb(x: Float, radius: Float, alpha: Float, fill: Color, ring: Color) {
    val cy = size.height / 2f
    drawCircle(Color.Black.copy(alpha = 0.16f * alpha), radius + 1.dp.toPx(), Offset(x, cy + 1.dp.toPx()))
    drawCircle(fill.copy(alpha = fill.alpha * alpha), radius, Offset(x, cy))
    drawCircle(ring.copy(alpha = ring.alpha * alpha), radius - 1.dp.toPx(), Offset(x, cy), style = Stroke(2.dp.toPx()))
}

/**
 * The volume in the seek bar's style: the same straight line between two speaker glyphs (the low
 * one mutes), thickening while it is dragged, with the thumb when a mouse hovers it or it has
 * keyboard focus. Left and Right step 5 % (mirrored right to left), Home and End go to the ends.
 */
@Composable
fun VolumeBar(
    volume: Float,
    muted: Boolean,
    description: String,
    valueText: String,
    muteDescription: String,
    onVolume: (Float) -> Unit,
    onVolumeEnd: () -> Unit,
    onToggleMute: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val shown = if (muted) 0f else volume.coerceIn(0f, 1f)
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    var dragging by remember { mutableStateOf(false) }
    val touch = !hovered
    val state = when {
        dragging -> SeekBarState.Drag
        hovered || focused -> SeekBarState.Hover
        else -> SeekBarState.Rest
    }
    val thickness by animateFloatAsState(SeekGeometry.thickness(state, touch), JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "volume")
    val thumb by animateFloatAsState(if (SeekGeometry.showsThumb(state, touch)) 1f else 0f, JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "volume-thumb")
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        JfIconButton(R.drawable.ic_volume_low, muteDescription, onToggleMute, tint = colors.ink50, size = 40.dp, iconSize = 18.dp)
        var widthPx by remember { mutableFloatStateOf(1f) }
        val density = androidx.compose.ui.platform.LocalDensity.current.density
        fun at(x: Float): Float = SeekGeometry.fractionAt(x / density, widthPx / density, touch, rtl)
        Box(
            Modifier
                .weight(1f)
                .height(TouchTarget)
                .onSizeChanged { widthPx = it.width.toFloat().coerceAtLeast(1f) }
                .hoverable(interaction)
                .focusRing(interaction, RoundedCornerShape(12.dp))
                .onKeyEvent { event ->
                    if (event.type != KeyEventType.KeyDown) return@onKeyEvent false
                    val target = when (event.key) {
                        Key.DirectionLeft -> shown + if (rtl) VOLUME_STEP else -VOLUME_STEP
                        Key.DirectionRight -> shown + if (rtl) -VOLUME_STEP else VOLUME_STEP
                        Key.MoveHome -> 0f
                        Key.MoveEnd -> 1f
                        else -> return@onKeyEvent false
                    }
                    onVolume(target.coerceIn(0f, 1f))
                    onVolumeEnd()
                    true
                }
                .focusable(true, interaction)
                .semantics {
                    contentDescription = description
                    stateDescription = valueText
                    progressBarRangeInfo = ProgressBarRangeInfo(shown, 0f..1f)
                    setProgress {
                        onVolume(it.coerceIn(0f, 1f))
                        onVolumeEnd()
                        true
                    }
                }
                .pointerInput(touch) {
                    detectTapGestures {
                        onVolume(at(it.x))
                        onVolumeEnd()
                    }
                }
                .pointerInput(touch) {
                    detectHorizontalDragGestures(
                        onDragStart = { dragging = true },
                        onDragEnd = {
                            dragging = false
                            onVolumeEnd()
                        },
                        onDragCancel = {
                            dragging = false
                            onVolumeEnd()
                        },
                        onHorizontalDrag = { change, _ -> onVolume(at(change.position.x)) },
                    )
                },
            contentAlignment = Alignment.Center,
        ) {
            Canvas(Modifier.fillMaxWidth().height(24.dp)) {
                drawSeekLine(shown, thickness.dp.toPx(), SeekGeometry.inset(touch).dp.toPx(), rtl, colors.ink90, colors.ink30)
                if (thumb > 0f) {
                    val x = SeekGeometry.thumbCentre(shown, size.width / density, touch, rtl).dp.toPx()
                    drawHoverThumb(x, (SeekGeometry.THUMB / 2f).dp.toPx() * (0.6f + 0.4f * thumb), thumb, colors.ink90, colors.glassEdge)
                }
            }
        }
        JfIcon(R.drawable.ic_volume_high, colors.ink50, Modifier.padding(horizontal = 11.dp), size = 18.dp)
    }
}

/**
 * The mini-player's progress (v0.0.4): a straight 2 dp line along the capsule's bottom edge,
 * inset by its corners (player.mini-radius), read in the draw phase so it redraws without
 * recomposing. Decorative: the capsule's button carries the state for TalkBack.
 */
@Composable
fun MiniProgressLine(fraction: () -> Float, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    Canvas(modifier.fillMaxWidth().height(SeekGeometry.MINI_PROGRESS.dp)) {
        val inset = SeekGeometry.MINI_RADIUS.dp.toPx()
        val cy = size.height / 2f
        val length = (size.width - 2 * inset).coerceAtLeast(0f)
        drawLine(colors.ink10, Offset(inset, cy), Offset(inset + length, cy), size.height, StrokeCap.Round)
        val end = fraction().coerceIn(0f, 1f) * length
        if (end > 0f) {
            val from = if (rtl) size.width - inset else inset
            drawLine(colors.ink90, Offset(from, cy), Offset(if (rtl) from - end else from + end, cy), size.height, StrokeCap.Round)
        }
    }
}

/**
 * The playing row's level glyph (after Apple Music): three bars that move only while music plays
 * and stand still when paused or with reduced motion (the infinite transition then never starts).
 */
@Composable
fun LevelGlyph(running: Boolean, modifier: Modifier = Modifier, tint: Color = Jf.colors.accentText) {
    val moving = running && !Jf.effects.reduceMotion
    val rests = floatArrayOf(0.55f, 0.95f, 0.7f)
    val heights = if (moving) {
        val transition = rememberInfiniteTransition(label = "level")
        listOf(420, 580, 500).mapIndexed { i, period ->
            transition.animateFloat(0.25f, rests[i], infiniteRepeatable(tween(period, easing = JfMotion.Fluid), RepeatMode.Reverse), label = "bar$i")
        }.map { it.value }
    } else {
        rests.toList()
    }
    Canvas(modifier.size(width = 13.dp, height = 14.dp).clearAndSetSemantics {}) {
        val bar = 3.dp.toPx()
        val gap = 2.dp.toPx()
        heights.forEachIndexed { i, h ->
            val height = size.height * h
            drawRoundRect(tint, Offset(i * (bar + gap), size.height - height), Size(bar, height), CornerRadius(bar / 2f))
        }
    }
}

/** Misty's segmented control: a recessed track, the chosen segment raised. Scrolls when it overflows. */
@Composable
fun <T> SegmentedControl(
    options: List<T>,
    selected: T?,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    description: String,
    modifier: Modifier = Modifier,
) {
    val colors = Jf.colors
    Row(
        modifier
            .clip(RoundedCornerShape(10.dp))
            .background(colors.ink06, RoundedCornerShape(10.dp))
            .border(1.dp, colors.lineSoft, RoundedCornerShape(10.dp))
            .horizontalScroll(rememberScrollState())
            .padding(3.dp)
            .semantics { contentDescription = description },
    ) {
        options.forEach { option ->
            val isSelected = option == selected
            val interaction = remember { MutableInteractionSource() }
            Box(
                Modifier
                    .heightIn(min = 36.dp)
                    .clip(RoundedCornerShape(7.dp))
                    .then(
                        if (isSelected) {
                            Modifier.background(colors.cloud, RoundedCornerShape(7.dp)).border(1.dp, colors.glassEdgeSoft, RoundedCornerShape(7.dp))
                        } else {
                            Modifier
                        },
                    )
                    .selectable(isSelected, interaction, indication = null, role = Role.Tab, onClick = { onSelect(option) })
                    .padding(horizontal = 14.dp, vertical = 7.dp),
                contentAlignment = Alignment.Center,
            ) {
                JfText(label(option), style = JfType.label, color = if (isSelected) colors.ink90 else colors.ink70, maxLines = 1)
            }
        }
    }
}

/** A chip such as "Speakers": a capsule with a glyph and a label. */
@Composable
fun ChipButton(icon: Int, text: String, onClick: () -> Unit, modifier: Modifier = Modifier, description: String? = null) {
    val colors = Jf.colors
    val interaction = remember { MutableInteractionSource() }
    Row(
        modifier
            .heightIn(min = 40.dp)
            .focusRing(interaction, CircleShape)
            .pressScale(interaction, 0.96f)
            .clip(CircleShape)
            .background(colors.fill, CircleShape)
            .border(1.dp, colors.line, CircleShape)
            .clickable(interaction, indication = null, role = Role.Button, onClick = onClick)
            .then(if (description != null) Modifier.semantics { contentDescription = description } else Modifier)
            .padding(start = 10.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        JfIcon(icon, colors.ink90, size = 18.dp)
        JfText(text, style = JfType.label, maxLines = 1, modifier = Modifier.widthIn(max = 160.dp))
    }
}

enum class ButtonStyle { Accent, Tonal, Quiet, Danger }

/** A text button: accent (the view's one primary action), tonal, quiet or danger. */
@Composable
fun JfButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    style: ButtonStyle = ButtonStyle.Tonal,
    icon: Int? = null,
    enabled: Boolean = true,
) {
    val colors = Jf.colors
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val (bg, fg) = when (style) {
        ButtonStyle.Accent -> (if (pressed) colors.accentPress else colors.accent) to colors.onAccent
        ButtonStyle.Tonal -> (if (pressed) colors.fillPress else colors.fill) to colors.ink90
        ButtonStyle.Quiet -> (if (pressed) colors.fillPress else Color.Transparent) to colors.accentText
        ButtonStyle.Danger -> (if (pressed) colors.fillPress else colors.fill) to colors.danger
    }
    val shape = RoundedCornerShape(Tokens.Radius.rMd.dp)
    Row(
        modifier
            .heightIn(min = TouchTarget)
            .focusRing(interaction, shape)
            .pressScale(interaction, 0.96f)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f }
            .clip(shape)
            .background(bg, shape)
            .then(if (style == ButtonStyle.Tonal || style == ButtonStyle.Danger) Modifier.border(1.dp, colors.lineSoft, shape) else Modifier)
            .clickable(interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    ) {
        if (icon != null) JfIcon(icon, fg, size = 18.dp)
        JfText(text, style = JfType.label, color = fg, maxLines = 1)
    }
}

/** An on/off switch drawn in the theme (no Material): accent track when on. */
@Composable
fun JfSwitch(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    val interaction = remember { MutableInteractionSource() }
    val position by animateFloatAsState(if (checked) 1f else 0f, JfMotion.fluid(Tokens.Motion.dur1Ms, reduce), label = "switch")
    val track = if (checked) colors.accent else colors.lineStrong
    val knob = if (checked) colors.onAccent else colors.cloud
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    Box(
        modifier
            .size(width = 52.dp, height = TouchTarget)
            .focusRing(interaction, RoundedCornerShape(16.dp))
            .toggleable(checked, interaction, indication = null, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(width = 46.dp, height = 28.dp)) {
            drawRoundRect(track, cornerRadius = CornerRadius(size.height / 2f))
            val r = size.height / 2f - 3.dp.toPx()
            val p = if (rtl) 1f - position else position
            val cx = size.height / 2f + p * (size.width - size.height)
            drawCircle(knob, r, Offset(cx, size.height / 2f))
        }
    }
}

/**
 * A horizontal slider: the value between [range], snapped to [step] when it is above zero.
 * [onChange] follows the finger; [onChangeFinished] marks the end of a drag or tap.
 */
@Composable
fun JfSlider(
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    onChange: (Float) -> Unit,
    description: String,
    valueText: String,
    modifier: Modifier = Modifier,
    step: Float = 0f,
    enabled: Boolean = true,
    onChangeFinished: () -> Unit = {},
    track: Brush? = null,
) {
    val colors = Jf.colors
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val span = (range.endInclusive - range.start).takeIf { it > 0f } ?: 1f
    val fraction = ((value - range.start) / span).coerceIn(0f, 1f)
    var widthPx by remember { mutableFloatStateOf(1f) }
    fun valueAt(x: Float): Float {
        val f = ((if (rtl) widthPx - x else x) / widthPx).coerceIn(0f, 1f)
        val raw = range.start + f * span
        return if (step > 0f) (Math.round((raw - range.start) / step) * step + range.start).coerceIn(range) else raw
    }
    Box(
        modifier
            .fillMaxWidth()
            .height(TouchTarget)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f }
            .onSizeChanged { widthPx = it.width.toFloat().coerceAtLeast(1f) }
            .semantics {
                contentDescription = description
                stateDescription = valueText
                progressBarRangeInfo = ProgressBarRangeInfo(value, range, if (step > 0f) ((span / step).roundToInt() - 1).coerceAtLeast(0) else 0)
                if (enabled) {
                    setProgress {
                        onChange(it.coerceIn(range))
                        onChangeFinished()
                        true
                    }
                }
            }
            .then(
                if (!enabled) {
                    Modifier
                } else {
                    Modifier
                        .pointerInput(range, step) {
                            detectTapGestures {
                                onChange(valueAt(it.x))
                                onChangeFinished()
                            }
                        }
                        .pointerInput(range, step) {
                            detectHorizontalDragGestures(
                                onDragEnd = onChangeFinished,
                                onDragCancel = onChangeFinished,
                                onHorizontalDrag = { change, _ -> onChange(valueAt(change.position.x)) },
                            )
                        }
                },
            ),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxWidth().height(24.dp)) {
            val h = 4.dp.toPx()
            val cy = size.height / 2f
            val r = 10.dp.toPx()
            val usable = size.width - 2 * r
            if (track != null) {
                drawRoundRect(track, Offset(r, cy - h), Size(usable, 2 * h), CornerRadius(h))
            } else {
                drawRoundRect(colors.lineStrong, Offset(r, cy - h / 2), Size(usable, h), CornerRadius(h / 2))
                val filled = fraction * usable
                val left = if (rtl) r + usable - filled else r
                drawRoundRect(colors.accent, Offset(left, cy - h / 2), Size(filled, h), CornerRadius(h / 2))
            }
            val x = r + (if (rtl) 1f - fraction else fraction) * usable
            drawCircle(Color.Black.copy(alpha = 0.16f), r + 1.dp.toPx(), Offset(x, cy + 1.dp.toPx()))
            drawCircle(colors.cloud, r, Offset(x, cy))
            drawCircle(colors.lineStrong, r, Offset(x, cy), style = Stroke(1.dp.toPx()))
        }
    }
}

/** A vertical slider for one EQ band: up is louder. */
@Composable
fun VerticalBandSlider(
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    step: Float,
    onChange: (Float) -> Unit,
    onChangeFinished: () -> Unit,
    description: String,
    valueText: String,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val colors = Jf.colors
    val span = range.endInclusive - range.start
    val fraction = ((value - range.start) / span).coerceIn(0f, 1f)
    var heightPx by remember { mutableFloatStateOf(1f) }
    fun valueAt(y: Float): Float {
        val f = (1f - y / heightPx).coerceIn(0f, 1f)
        return (Math.round(f * span / step) * step + range.start).coerceIn(range)
    }
    Box(
        modifier
            .width(TouchTarget)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f }
            .onSizeChanged { heightPx = it.height.toFloat().coerceAtLeast(1f) }
            .semantics {
                contentDescription = description
                stateDescription = valueText
                progressBarRangeInfo = ProgressBarRangeInfo(value, range, ((span / step).roundToInt() - 1).coerceAtLeast(0))
                if (enabled) {
                    setProgress {
                        onChange(it.coerceIn(range))
                        onChangeFinished()
                        true
                    }
                }
            }
            .then(
                if (!enabled) {
                    Modifier
                } else {
                    Modifier
                        .pointerInput(range, step) {
                            detectTapGestures {
                                onChange(valueAt(it.y))
                                onChangeFinished()
                            }
                        }
                        .pointerInput(range, step) {
                            detectVerticalDragGestures(
                                onDragEnd = onChangeFinished,
                                onDragCancel = onChangeFinished,
                                onVerticalDrag = { change, _ -> onChange(valueAt(change.position.y)) },
                            )
                        }
                },
            ),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.width(24.dp).padding(vertical = 10.dp).fillMaxHeight()) {
            val w = 4.dp.toPx()
            val cx = size.width / 2f
            drawRoundRect(colors.lineStrong, Offset(cx - w / 2, 0f), Size(w, size.height), CornerRadius(w / 2))
            val zero = size.height * (1f - ((0f - range.start) / span).coerceIn(0f, 1f))
            val y = size.height * (1f - fraction)
            drawRoundRect(colors.accent, Offset(cx - w / 2, minOf(y, zero)), Size(w, kotlin.math.abs(zero - y)), CornerRadius(w / 2))
            val r = 9.dp.toPx()
            drawCircle(colors.cloud, r, Offset(cx, y))
            drawCircle(colors.lineStrong, r, Offset(cx, y), style = Stroke(1.dp.toPx()))
        }
    }
}

/** A text field drawn in the theme: label above, a quiet filled box, masked when [secret]. */
@Composable
fun JfTextField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    secret: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    error: String? = null,
    singleLine: Boolean = true,
    supporting: String? = null,
) {
    val colors = Jf.colors
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val shape = RoundedCornerShape(Tokens.Radius.rMd.dp)
    androidx.compose.foundation.layout.Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        JfText(label, style = JfType.label, color = colors.ink70)
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            singleLine = singleLine,
            textStyle = JfType.body.copy(color = colors.ink90),
            cursorBrush = androidx.compose.ui.graphics.SolidColor(colors.accentText),
            visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(
                keyboardType = if (secret) KeyboardType.Password else keyboard,
                autoCorrectEnabled = !secret && keyboard == KeyboardType.Text,
            ),
            interactionSource = interaction,
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = label },
            decorationBox = { inner ->
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = TouchTarget)
                        .background(colors.fill, shape)
                        .border(if (focused) 2.dp else 1.dp, if (error != null) colors.danger else if (focused) colors.accentText else colors.line, shape)
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    if (value.isEmpty() && placeholder != null) JfText(placeholder, style = JfType.body, color = colors.ink50, maxLines = 1)
                    inner()
                }
            },
        )
        when {
            error != null -> JfText(error, style = JfType.caption, color = colors.danger)
            supporting != null -> JfText(supporting, style = JfType.caption, color = colors.ink50)
        }
    }
}

/** The badge of a source: Official (accent) or Community · unofficial (warning). */
@Composable
fun Badge(text: String, warning: Boolean, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    val fg = if (warning) colors.warning else colors.accentText
    Box(
        modifier
            .background(if (warning) colors.fill else colors.accentSoft, CircleShape)
            .border(1.dp, if (warning) colors.line else Color.Transparent, CircleShape)
            .padding(horizontal = 8.dp, vertical = 2.dp),
    ) {
        JfText(text, style = JfType.micro, color = fg, maxLines = 1)
    }
}
