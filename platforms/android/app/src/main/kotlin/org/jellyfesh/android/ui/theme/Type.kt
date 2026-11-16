package org.jellyfesh.android.ui.theme

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.tokens.Typography

/**
 * Misty's interface scale and Jellyfesh's player roles (Tokens.TypeRoles, Tokens.Lyrics) in the
 * system face (Roboto and its CJK fallbacks: no bundled fonts, Misty TYP-01). Sizes are sp, so
 * they follow the person's font size; no negative tracking, so CJK titles are not squeezed
 * (TYP-08).
 */
object JfType {
    private val centred = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None)

    fun style(size: Float, lineHeight: Float, weight: Int): TextStyle = TextStyle(
        fontSize = size.sp,
        lineHeight = (size * lineHeight).sp,
        fontWeight = FontWeight(weight),
        lineHeightStyle = centred,
    )

    fun of(t: Typography): TextStyle = style(t.size, t.lineHeight, t.weight)

    // Misty interface scale. Large titles (v0.0.4, after Apple Music): type.text-h2, bold.
    val title = style(Tokens.TypeScale.textH2, 1.1f, 700)

    /** Page titles (Luminous: type.text-h1, bold) at [size] for the window ([org.jellyfesh.logic.theme.Luminous.pageTitleSp]). */
    fun pageTitle(size: Float): TextStyle = style(size, 1.05f, 700)
    val heading = style(20f, 1.2f, 700)
    val body = style(16f, 1.7f, 400)
    val small = style(14f, 1.6f, 400)
    val label = style(13f, 1.4f, 600)
    val caption = style(12f, 1.45f, 600)
    val micro = style(11f, 1.4f, 700)

    // Player roles.
    val section = of(Tokens.TypeRoles.section)
    val listTitle = of(Tokens.TypeRoles.listTitle)
    val listSecondary = of(Tokens.TypeRoles.listSecondary)
    val nowPlayingTitle = of(Tokens.TypeRoles.nowPlayingTitle)
    val nowPlayingArtist = of(Tokens.TypeRoles.nowPlayingArtist)

    /** Clock labels: tabular figures, so the digits do not jiggle as they change. */
    val time = caption.copy(fontFeatureSettings = "tnum")

    /** A lyric line at [size] (fluid between the tokens' bounds), with the CJK line height when [cjk]. */
    fun lyricLine(size: TextUnit, cjk: Boolean): TextStyle {
        val t = Tokens.Lyrics.line
        val lh = if (cjk) Tokens.Lyrics.lineCjkLineHeight else t.lineHeight
        return TextStyle(
            fontSize = size,
            lineHeight = (size.value * lh).sp,
            fontWeight = FontWeight(t.weight),
            lineHeightStyle = centred,
        )
    }

    val lyricTranslation = of(Tokens.Lyrics.translation)
    val lyricRomanization = of(Tokens.Lyrics.romanization)
}
