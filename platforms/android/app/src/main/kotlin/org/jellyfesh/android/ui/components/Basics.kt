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
