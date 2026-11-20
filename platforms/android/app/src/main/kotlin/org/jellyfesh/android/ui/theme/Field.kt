package org.jellyfesh.android.ui.theme

import android.graphics.Bitmap
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageShader
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.scale
import org.jellyfesh.logic.theme.AuroraColour
import org.jellyfesh.logic.theme.Luminous
import org.jellyfesh.tokens.Tokens
import kotlin.random.Random

/**
 * The lit field behind every view (MAT-02; Luminous, v0.0.6): the sky colour, four large aurora
 * glows at the theme hue ([Luminous.glows] at field.aurora-strength, 75 %) and a fine grain, on
 * every screen, light and dark. Static: it draws again only when its size or colours change (the
 * colours crossfade over dur-4 on a song change), never per frame.
 */
@Composable
fun MistyField(modifier: Modifier = Modifier, strength: Float = Jf.look.aurora(Tokens.Field.auroraStrength)) {
    val colors = Jf.colors
    val sky = colors.sky
    val aurora1 = colors[Tokens.Field.aurora1]
    val aurora2 = colors[Tokens.Field.aurora2]
    val aurora3 = colors[Tokens.Field.aurora3]
    val grainAlpha = if (colors.dark) Tokens.Field.grain.dark else Tokens.Field.grain.light
    val spec = remember(strength) { Luminous.glows(strength) }
    Box(
        modifier.drawWithCache {
            val w = size.width
            val h = size.height
            val grain = ShaderBrush(ImageShader(Grain.image, TileMode.Repeated, TileMode.Repeated))
            val glows = spec.filter { it.alpha > 0f }.map { g ->
                val colour = when (g.colour) {
                    AuroraColour.Aurora1 -> aurora1
                    AuroraColour.Aurora2 -> aurora2
                    AuroraColour.Aurora3 -> aurora3
                }
                Glow(Offset(w * g.cx, h * g.cy), w * g.rx, h * g.ry, colour.copy(alpha = colour.alpha * g.alpha), g.fade)
            }
            onDrawBehind {
                drawRect(sky)
                for (g in glows) {
                    // A circle of the ellipse's width, scaled vertically into the ellipse.
                    val sy = g.ry / g.rx
                    scale(1f, sy, pivot = g.centre) {
                        drawCircle(
                            Brush.radialGradient(
                                0f to g.color,
                                g.fade to g.color.copy(alpha = 0f),
                                center = g.centre,
                                radius = g.rx,
                            ),
                            radius = g.rx,
                            center = g.centre,
                        )
                    }
                }
                drawRect(grain, alpha = grainAlpha)
            }
        },
    )
}

private class Glow(val centre: Offset, val rx: Float, val ry: Float, val color: Color, val fade: Float)

/** A tile of monochrome noise, made once per process. */
private object Grain {
    val image: ImageBitmap by lazy {
        val size = 96
        val random = Random(250)
        val pixels = IntArray(size * size) {
            val v = random.nextInt(256)
            // White or black specks of varying strength; the field draws the tile at low alpha.
            if (v < 128) (v * 2 shl 24) else ((v - 128) * 2 shl 24) or 0x00FFFFFF
        }
        Bitmap.createBitmap(pixels, size, size, Bitmap.Config.ARGB_8888).asImageBitmap()
    }
}
