package org.jellyfesh.android.ui.theme

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.invalidateDraw
import kotlinx.coroutines.launch
import org.jellyfesh.android.system.GlassTier
import org.jellyfesh.logic.core.ThemeSnapshot
import org.jellyfesh.logic.theme.ColorMath
import org.jellyfesh.logic.theme.Palette
import org.jellyfesh.tokens.ThemeColorKey
import org.jellyfesh.tokens.Tokens

/**
 * The colours views draw with: the core's Theme slice for the current appearance (every colour
 * resolved from one hue in the core, ADR-0013), with the generated hue-250 fallback before the
 * first slice. A hue change crossfades over dur-4 with the Fluid curve (architecture section 6);
 * reading a colour during that crossfade recomposes the reader, which happens once per track.
 */
@Stable
class JfColors internal constructor(
    private val from: Palette,
    private val to: Palette,
    private val progress: State<Float>,
) {
    val dark: Boolean get() = to.dark

    /** The theme hue in degrees. */
    val hue: Float get() = to.hue

    operator fun get(key: ThemeColorKey): Color {
        val t = progress.value
        val argb = if (t >= 1f) to[key.name] else ColorMath.lerp(from[key.name], to[key.name], t)
        return Color(argb)
    }

    val ink90: Color get() = get(Tokens.Ink.ink90)
    val ink70: Color get() = get(Tokens.Ink.ink70)
    val ink50: Color get() = get(Tokens.Ink.ink50)
    val ink30: Color get() = get(Tokens.Ink.ink30)
    val ink10: Color get() = get(Tokens.Ink.ink10)
    val ink06: Color get() = get(Tokens.Ink.ink06)
    val accent: Color get() = get(Tokens.Accent.accent)
    val accentPress: Color get() = get(Tokens.Accent.accentPress)
    val accentSoft: Color get() = get(Tokens.Accent.accentSoft)
    val accentText: Color get() = get(Tokens.Accent.accentText)
    val onAccent: Color get() = get(Tokens.Accent.onAccent)
    val sky: Color get() = get(Tokens.Field.sky)
    val skyHigh: Color get() = get(Tokens.Field.skyHigh)
    val cloud: Color get() = get(Tokens.Glass.cloud)
    val cloudSoft: Color get() = get(Tokens.Glass.cloudSoft)
    val glassEdge: Color get() = get(Tokens.Glass.glassEdge)
    val glassEdgeSoft: Color get() = get(Tokens.Glass.glassEdgeSoft)
    val line: Color get() = get(Tokens.Line.line)
    val lineSoft: Color get() = get(Tokens.Line.lineSoft)
    val lineStrong: Color get() = get(Tokens.Line.lineStrong)
    val fill: Color get() = get(Tokens.Line.fill)
    val fillPress: Color get() = get(Tokens.Line.fillPress)
    val scrim: Color get() = get(Tokens.Elevation.scrim)
    val danger: Color get() = get(Tokens.Semantic.danger)
    val warning: Color get() = get(Tokens.Semantic.warning)
    val success: Color get() = get(Tokens.Semantic.success)
}

/** Durations and curves (Tokens.Motion), and the rule that reduced motion completes at once (MOT-12). */
object JfMotion {
    val Fluid = Tokens.Motion.easeFluid.let { CubicBezierEasing(it.x1, it.y1, it.x2, it.y2) }
    val Spring = Tokens.Motion.easeSpring.let { CubicBezierEasing(it.x1, it.y1, it.x2, it.y2) }
    val Exit = Tokens.Motion.easeExit.let { CubicBezierEasing(it.x1, it.y1, it.x2, it.y2) }

    /** Misty "Deeper": the mini-player opening Now Playing. */
    const val OPEN_NOW_PLAYING_MS = 320

    /** Misty "Back": Now Playing closing. */
    const val CLOSE_NOW_PLAYING_MS = 300

    fun <T> fluid(durationMs: Int, reduce: Boolean): FiniteAnimationSpec<T> =
        if (reduce) snap() else tween(durationMs, easing = Fluid)

    fun <T> exit(durationMs: Int, reduce: Boolean): FiniteAnimationSpec<T> =
        if (reduce) snap() else tween(durationMs, easing = Exit)

    fun <T> spring(durationMs: Int, reduce: Boolean): FiniteAnimationSpec<T> =
        if (reduce) snap() else tween(durationMs, easing = Spring)

    /**
     * Opacity coming in. Under reduced motion every transition becomes this short fade (dur-1)
     * while what moves or scales snaps ([fluid], [exit], [spring]).
     */
    fun <T> fade(durationMs: Int, reduce: Boolean): FiniteAnimationSpec<T> =
        tween(if (reduce) Tokens.Motion.dur1Ms else durationMs, easing = Fluid)

    /** Opacity going out: on Exit, or the short fade under reduced motion. */
    fun <T> fadeOut(durationMs: Int, reduce: Boolean): FiniteAnimationSpec<T> =
        if (reduce) tween(Tokens.Motion.dur1Ms, easing = Fluid) else tween(durationMs, easing = Exit)
}

/** What the device and the person allow: glass grade, motion and how see-through glass is. */
data class JfEffects(
    val glassTier: GlassTier,
    /** The system removes animations, or the person reduced effects: nothing moves, transitions are short fades. */
    val reduceMotion: Boolean,
    /** The system removes animations: no karaoke sweep, the active line lights whole (Misty MOT-12). */
    val systemReducesMotion: Boolean,
    /** The Transparency setting, 0 (solid) to 1 (clearest); 0.5 keeps the glass tokens' alphas. */
    val transparency: Float = 0.5f,
)

/** Whether playback is running now: the playing row's level glyph moves only then. */
val LocalPlaybackRunning = staticCompositionLocalOf { false }

val LocalJfColors = staticCompositionLocalOf<JfColors> { error("JfTheme is missing") }
val LocalJfEffects = staticCompositionLocalOf { JfEffects(GlassTier.Solid, reduceMotion = false, systemReducesMotion = false) }

/** Shorthands inside composables. */
object Jf {
    val colors: JfColors
        @Composable get() = LocalJfColors.current

    val effects: JfEffects
        @Composable get() = LocalJfEffects.current
}

/**
 * Provides the theme: the palette for [dark] from [theme] (crossfading on hue changes, snapping
 * on appearance flips and under reduced motion) and the device's effects.
 */
@Composable
fun JfTheme(theme: ThemeSnapshot?, dark: Boolean, effects: JfEffects, content: @Composable () -> Unit) {
    val target = remember(theme, dark) { Palette.of(theme, dark) }
    var from by remember { mutableStateOf(target) }
    var to by remember { mutableStateOf(target) }
    val progress = remember { Animatable(1f) }
    LaunchedEffect(target) {
        if (target === to) return@LaunchedEffect
        val snapNow = effects.reduceMotion || target.dark != to.dark
        // Start from what is on screen now, even in the middle of a crossfade.
        from = if (progress.value >= 1f) to else frozen(from, to, progress.value)
        to = target
        if (snapNow) {
            progress.snapTo(1f)
        } else {
            progress.snapTo(0f)
            progress.animateTo(1f, tween(Tokens.Motion.dur4Ms, easing = JfMotion.Fluid))
        }
    }
    val colors = remember(from, to) { JfColors(from, to, progress.asState()) }
    val indication = remember(colors) { PressFill(colors) }
    CompositionLocalProvider(
        LocalJfColors provides colors,
        LocalJfEffects provides effects,
        LocalIndication provides indication,
        content = content,
    )
}

/**
 * The press feedback of rows, cards and other plain clickables: the theme's press fill over the
 * content while a finger is down (controls draw their own, Controls.kt). The colour is read at
 * draw time, so a hue crossfade redraws pressed rows without recomposing them.
 */
private class PressFill(private val colors: JfColors) : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode = PressFillNode(interactionSource, colors)

    override fun equals(other: Any?): Boolean = other is PressFill && other.colors === colors

    override fun hashCode(): Int = System.identityHashCode(colors)
}

private class PressFillNode(private val source: InteractionSource, private val colors: JfColors) : Modifier.Node(), DrawModifierNode {
    private var pressed = false

    override fun onAttach() {
        coroutineScope.launch {
            var presses = 0
            source.interactions.collect { interaction ->
                when (interaction) {
                    is PressInteraction.Press -> presses++
                    is PressInteraction.Release, is PressInteraction.Cancel -> presses = (presses - 1).coerceAtLeast(0)
                }
                val now = presses > 0
                if (now != pressed) {
                    pressed = now
                    invalidateDraw()
                }
            }
        }
    }

    override fun ContentDrawScope.draw() {
        drawContent()
        if (pressed) drawRect(colors.fillPress)
    }
}

/** A palette fixed at the blend of [a] and [b] at [t], to start a new crossfade from. */
private fun frozen(a: Palette, b: Palette, t: Float): Palette {
    val keys = THEME_KEYS
    val blended = keys.associateWith { ColorMath.lerp(a[it], b[it], t) }
    return Palette(blended, emptyMap(), b.dark, b.hue)
}

/** Every theme colour the views read (the fallback theme's keys are the full set). */
private val THEME_KEYS: Set<String> = org.jellyfesh.logic.theme.FallbackTheme.light.keys
