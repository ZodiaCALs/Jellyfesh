package org.jellyfesh.android.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.fontFamily

/** Text in the theme's ink (foundation's BasicText: the app uses no Material components). */
@Composable
fun JfText(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = JfType.body,
    color: Color = Jf.colors.ink90,
    maxLines: Int = Int.MAX_VALUE,
    textAlign: TextAlign? = null,
    overflow: TextOverflow = TextOverflow.Ellipsis,
) {
    // The look's face for text that has none of its own (ADR-0020).
    val family = style.fontFamily ?: Jf.look.fontFamily()
    BasicText(
        text = text,
        modifier = modifier,
        style = style.copy(color = color, textAlign = textAlign ?: style.textAlign, fontFamily = family),
        maxLines = maxLines,
        overflow = overflow,
    )
}

/** A glyph from res/drawable (24-unit outline icons), tinted. Decorative unless [description]. */
@Composable
fun JfIcon(id: Int, tint: Color, modifier: Modifier = Modifier, size: Dp = 24.dp, description: String? = null) {
    Image(
        painter = painterResource(id),
        contentDescription = description,
        modifier = modifier.size(size),
        colorFilter = ColorFilter.tint(tint),
    )
}

/**
 * Buttons in a row that wraps onto more lines when they do not fit (a source's Sign In, Settings
 * and Uninstall… on a narrow phone), [gap] apart both ways.
 */
@Composable
fun WrapRow(modifier: Modifier = Modifier, gap: Dp = 8.dp, content: @Composable () -> Unit) {
    androidx.compose.ui.layout.Layout(content, modifier) { measurables, constraints ->
        val gapPx = gap.roundToPx()
        val placeables = measurables.map { it.measure(constraints.copy(minWidth = 0, minHeight = 0)) }
        var x = 0
        var y = 0
        var line = 0
        val positions = placeables.map { p ->
            if (x > 0 && x + p.width > constraints.maxWidth) {
                x = 0
                y += line + gapPx
                line = 0
            }
            val position = x to y
            x += p.width + gapPx
            line = maxOf(line, p.height)
            position
        }
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth else placeables.sumOf { it.width }
        layout(width, y + line) {
            placeables.forEachIndexed { i, p -> p.placeRelative(positions[i].first, positions[i].second) }
        }
    }
}
