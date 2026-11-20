package org.jellyfesh.android.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.statusBars
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.skydoves.cloudy.CloudyProgressive
import com.skydoves.cloudy.Sky
import com.skydoves.cloudy.cloudy
import com.skydoves.cloudy.rememberSky
import com.skydoves.cloudy.sky
import org.jellyfesh.android.system.GlassTier
import org.jellyfesh.shared.glassAlpha
import org.jellyfesh.tokens.GlassGrade as TokenGrade
import org.jellyfesh.tokens.Tokens

// Glass, ADR-0005: GlassSurface is the app's only entry point to Cloudy (pinned at 0.7.1), so a
// swap to Haze touches this file alone. Screens never import Cloudy.
//
//  API 33+ (and not low-RAM, battery saver, reduce effects or the system's blur and contrast
//    settings): Cloudy backdrop blur (RenderEffect), progressive at an edge where asked, with the
//    Misty tint and edge.
//  API 31-32: Cloudy uniform blur; a progressive edge becomes a gradient scrim.
//  Below 31 (Luminous, v0.0.6: blur only on Android 12 and later), low-RAM, battery saver, reduce
//    effects, the system's blur turned off or more contrast asked for: solid Misty Cloud (MAT-08).
//
// The blur's radius, saturation and tint never animate (MAT-10). At most two live glass layers
// per screen (Tokens.GlassBudget.liveLayersMobile): the mini-player and the tab bar on library
// screens, the dock on Now Playing. Grouped surfaces (settings groups, lists, the Now Playing top
// controls) take a glass tint without live blur ([GlassSurface] with `blur = false`), so they cost
// no layer.
//
// The Transparency setting (v0.0.4) sets each grade's tint alpha through the shared glass-alpha
// port (platforms/shared/kotlin, tests/contract/glass-alpha.json): the token's alpha at 50 %,
// opaque toward Solid (solid Cloud at 0 %, so no blur runs), half the alpha toward Clear.

/** Misty's glass grades, clearest first; Cloud is the solid fallback of each. */
enum class GlassGrade(val token: TokenGrade) {
    Clear(Tokens.Glass.clear),
    Mist(Tokens.Glass.mist),
    Veil(Tokens.Glass.veil),
}

/** Where a glass surface fades out (Misty's progressive edge blur). */
enum class GlassEdge { None, Top, Bottom }

/** The backdrop glass surfaces blur: set by the nearest [GlassBackdrop]. */
class GlassBackdropState internal constructor(val sky: Sky?)

private val LocalGlassBackdrop = staticCompositionLocalOf { GlassBackdropState(null) }

/**
 * The container whose content the glass surfaces inside it blur: everything drawn in [content]
 * before them is captured (one recorder per screen). With solid glass nothing is captured.
 * [invalidateKey] re-captures once when it changes (a new theme or artwork), as ADR-0005 asks,
 * instead of per frame.
 */
@Composable
fun GlassBackdrop(modifier: Modifier = Modifier, invalidateKey: Any? = null, content: @Composable BoxScope.() -> Unit) {
    val tier = Jf.effects.glassTier
    // At 0 % transparency every grade is opaque (solid Cloud): nothing to capture.
    if (tier == GlassTier.Solid || Jf.effects.transparency <= 0f) {
        CompositionLocalProvider(LocalGlassBackdrop provides GlassBackdropState(null)) {
            Box(modifier, content = content)
        }
        return
    }
    val sky = rememberSky()
    LaunchedEffect(invalidateKey) {
        // The theme crossfade lasts dur-4: keep the capture fresh for its whole length.
        sky.invalidate(Tokens.Motion.dur4Ms.toLong())
    }
    CompositionLocalProvider(LocalGlassBackdrop provides GlassBackdropState(sky)) {
        Box(modifier.sky(sky), content = content)
    }
}

/**
 * A Misty glass surface: tinted backdrop blur where the device can afford it, else solid Cloud.
 * One 1 px inset edge and, when [floating], shadow-2 (MAT-04: no simulated thickness); [raised]
 * gives shadow-1 instead (Luminous grouped surfaces). With [blur] off it is the tint alone over
 * whatever lies behind: no live layer, for surfaces nothing scrolls under.
 */
@Composable
fun GlassSurface(
    modifier: Modifier = Modifier,
    grade: GlassGrade = GlassGrade.Veil,
    shape: Shape = RectangleShape,
    edge: GlassEdge = GlassEdge.None,
    floating: Boolean = false,
    showEdge: Boolean = true,
    blur: Boolean = true,
    raised: Boolean = false,
    content: @Composable BoxScope.() -> Unit,
) {
    val colors = Jf.colors
    val effects = Jf.effects
    val tint = tint(colors, grade).let { it.copy(alpha = glassAlpha(it.alpha, effects.transparency)) }
    // Opaque glass is the solid Cloud look, and costs no blur.
    val tier = if (tint.alpha >= 0.999f) GlassTier.Solid else effects.glassTier
    val sky = LocalGlassBackdrop.current.sky
    val edgeColor = colors.glassEdge
    var m = modifier
    when {
        floating -> m = m.jfShadow(2, shape)
        raised -> m = m.jfShadow(1, shape)
    }
    when {
        tier == GlassTier.Solid ->
            // Misty Cloud: the solid surface every glass grade falls back to.
            m = m.background(if (grade == GlassGrade.Clear) colors.cloudSoft else colors.cloud, shape)
        !blur ->
            // The tint alone: the surface sits on the field or the backdrop, where a blur would change nothing.
            m = m.background(tint, shape)
        sky == null ->
            m = m.background(if (grade == GlassGrade.Clear) colors.cloudSoft else colors.cloud, shape)
        tier == GlassTier.Uniform && edge != GlassEdge.None ->
            // API 31-32 blur only uniformly: a progressive edge becomes a gradient scrim.
            m = m.edgeScrim(colors.sky, edge)
        else -> {
            val density = LocalDensity.current
            // The look's glass scales the blur with the tint (ADR-0020).
            val radius = with(density) { cloudyRadius(grade.token.blur.dp.toPx() * Jf.look.glass) }
            val progressive = when (edge) {
                GlassEdge.Top -> CloudyProgressive.BottomToTop(start = 1f, end = 0f)
                GlassEdge.Bottom -> CloudyProgressive.TopToBottom(start = 0f, end = 1f)
                GlassEdge.None -> CloudyProgressive.None
            }
            m = m.cloudy(sky = sky, radius = radius, progressive = progressive, tint = tint, shape = shape)
        }
    }
    if (showEdge) m = m.border(1.dp, edgeColor, shape)
    Box(m, content = content)
}

/**
 * One of Misty's shadows (elevation.shadow-1 to -3) under [shape]: its two layers' colours from
 * the theme, at about the depth of its larger layer.
 */
@Composable
fun Modifier.jfShadow(level: Int, shape: Shape): Modifier {
    val colors = Jf.colors
    val (elevation, ambient, spot) = when (level) {
        1 -> Triple(4.dp, Tokens.Elevation.shadow1Layer0, Tokens.Elevation.shadow1Layer1)
        2 -> Triple(10.dp, Tokens.Elevation.shadow2Layer0, Tokens.Elevation.shadow2Layer1)
        else -> Triple(20.dp, Tokens.Elevation.shadow3Layer0, Tokens.Elevation.shadow3Layer1)
    }
    return shadow(elevation = elevation, shape = shape, clip = false, ambientColor = colors[ambient], spotColor = colors[spot])
}

/** The tint of a grade from the theme (glass.tint-clear, -mist, -veil), resolved at the hue. */
private fun tint(colors: JfColors, grade: GlassGrade): Color = when (grade) {
    GlassGrade.Clear -> colors[Tokens.Glass.tintClear]
    GlassGrade.Mist -> colors[Tokens.Glass.tintMist]
    GlassGrade.Veil -> colors[Tokens.Glass.tintVeil]
}

/**
 * Cloudy's radius for a CSS blur of [sigmaPx]: the platform blur turns a radius into
 * `sigma = 0.57735 * radius + 0.5` (Cloudy's documentation), and Misty's radii are CSS sigmas.
 */
private fun cloudyRadius(sigmaPx: Float): Int = ((sigmaPx - 0.5f) / 0.57735f).toInt().coerceAtLeast(1)

/** A gradient scrim of the field colour that fades out towards [edge], drawn behind the content. */
private fun Modifier.edgeScrim(color: Color, edge: GlassEdge): Modifier = drawWithContent {
    val brush = when (edge) {
        GlassEdge.Top -> Brush.verticalGradient(0f to color.copy(alpha = 0f), 1f to color.copy(alpha = 0.88f))
        GlassEdge.Bottom -> Brush.verticalGradient(0f to color.copy(alpha = 0.88f), 1f to color.copy(alpha = 0f))
        GlassEdge.None -> null
    }
    if (brush != null) drawRect(brush)
    drawContent()
}

/** A scrim from the field colour to clear, under the status bar where content scrolls past. */
fun Modifier.topEdgeFade(color: Color, height: Dp): Modifier = drawWithContent {
    drawContent()
    val h = height.toPx()
    drawRect(
        Brush.verticalGradient(0f to color.copy(alpha = 0.92f), 1f to color.copy(alpha = 0f), startY = 0f, endY = h),
        size = size.copy(height = h),
    )
}

/**
 * Fades what this draws out towards the floating chrome (Luminous): opaque until [fade] above
 * [bottom] (measured from the bottom edge), transparent from there down, so rows scrolling under
 * the mini-player and the tab bar fade out just above them and the glass shows the field.
 */
fun Modifier.fadeOutAbove(bottom: Dp, fade: Dp = Tokens.EdgeBlur.bottomHeight.dp): Modifier =
    graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            val end = size.height - bottom.toPx()
            val start = end - fade.toPx()
            drawRect(Brush.verticalGradient(listOf(Color.Black, Color.Transparent), startY = start, endY = end), blendMode = BlendMode.DstIn)
        }

/** [topEdgeFade] under the status bar and a little below it, in the field colour. */
@Composable
fun Modifier.statusBarFade(): Modifier {
    val inset = androidx.compose.foundation.layout.WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    return topEdgeFade(Jf.colors.sky, inset + 12.dp)
}
