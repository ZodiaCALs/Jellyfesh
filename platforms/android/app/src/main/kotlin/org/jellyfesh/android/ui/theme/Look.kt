package org.jellyfesh.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import org.jellyfesh.logic.core.Typeface
import org.jellyfesh.logic.theme.LookStyle
import org.jellyfesh.logic.theme.SungInk

/**
 * The look (ADR-0020), applied live: JfTheme provides it from the core's `Look` slice. Text
 * scales through the density's font scale and takes the look's face through [JfText] and the
 * lyrics; radii, spacing, glass and the field read it here. With the look not customized every
 * value is Luminous's, so the app draws as in v0.0.6.
 */
val LocalJfLook = staticCompositionLocalOf { LookStyle.Luminous }

/** A rounded shape of [radius] at the look's corner scale (0 squares it, 2 doubles it). */
@Composable
fun rounded(radius: Dp): RoundedCornerShape = RoundedCornerShape(radius * LocalJfLook.current.cornerScale)

/** A shape rounded only at the top (sheets), at the look's corner scale. */
@Composable
fun roundedTop(radius: Dp): RoundedCornerShape {
    val r = radius * LocalJfLook.current.cornerScale
    return RoundedCornerShape(topStart = r, topEnd = r)
}

/** A spacing at the look's density (× 0.8, 1 or 1.2). */
@Composable
fun spaced(base: Dp): Dp = base * LocalJfLook.current.densityScale

/** The platform's own face for the look's typeface; Android has no rounded system face. */
fun LookStyle.fontFamily(): FontFamily? = when (typeface) {
    Typeface.System, Typeface.Rounded -> null
    Typeface.Serif -> FontFamily.Serif
    Typeface.Mono -> FontFamily.Monospace
}

/** The colour of sung lyrics: the lyrics' sung ink (ink-90), the accent text, or the person's colour. */
@Composable
fun sungInk(): Color {
    val colors = Jf.colors
    return when (val ink = LocalJfLook.current.sungInk) {
        SungInk.Token -> colors.ink90
        SungInk.Accent -> colors.accentText
        is SungInk.Custom -> Color(0xFF000000.toInt() or ink.rgb)
    }
}
