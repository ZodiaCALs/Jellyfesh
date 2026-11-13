package org.jellyfesh.android.ui.theme

import android.graphics.Bitmap
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
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
import org.jellyfesh.tokens.Tokens
import kotlin.random.Random

/**
 * Misty's lit field behind every view (MAT-02): the sky colour, three aurora glows at the theme
 * hue and a fine grain. Static: it draws again only when its size or colours change, never per
 * frame.
 */
@Composable
fun MistyField(modifier: Modifier = Modifier, intensity: Float = 1f) {
    val colors = Jf.colors
    val sky = colors.sky
    val aurora1 = colors[Tokens.Field.aurora1]
    val aurora2 = colors[Tokens.Field.aurora2]
    val aurora3 = colors[Tokens.Field.aurora3]
    val grainAlpha = if (colors.dark) Tokens.Field.grain.dark else Tokens.Field.grain.light
    Box(
        modifier.drawWithCache {
            val w = size.width
            val h = size.height
            val grain = ShaderBrush(ImageShader(Grain.image, TileMode.Repeated, TileMode.Repeated))
            // The glows of the design canvas: radial-gradient(60% 55% at 10% 8%), (55% 55% at
            // 92% 92%) and (45% 50% at 4% 98%), each fading out at 70 %.
            val glows = listOf(
                Glow(Offset(w * 0.10f, h * 0.08f), w * 0.60f, h * 0.55f, aurora1.copy(alpha = 0.75f * intensity)),
                Glow(Offset(w * 0.92f, h * 0.92f), w * 0.55f, h * 0.55f, aurora2.copy(alpha = 0.80f * intensity)),
                Glow(Offset(w * 0.04f, h * 0.98f), w * 0.45f, h * 0.50f, aurora3.copy(alpha = 0.75f * intensity)),
            )
            onDrawBehind {
                drawRect(sky)
                for (g in glows) {
                    // A circle of the ellipse's width, scaled vertically into the ellipse.
                    val sy = g.ry / g.rx
                    scale(1f, sy, pivot = g.centre) {
                        drawCircle(
                            Brush.radialGradient(
                                0f to g.color,
                                0.7f to g.color.copy(alpha = 0f),
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

private class Glow(val centre: Offset, val rx: Float, val ry: Float, val color: Color)

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
