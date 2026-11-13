// Stand-ins for Cloudy 0.7.1's backdrop API (com.github.skydoves:cloudy), signatures copied from
// its sources jar (Sky.kt, CloudyBackground.kt, CloudyProgressive.kt, CloudyDefaults.kt). The
// desktop artifact on Maven Central needs a newer Compose than :ui-check can resolve. Bodies
// never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package com.skydoves.cloudy

import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape

@Stable
class Sky internal constructor() {
    fun invalidate() {}

    fun invalidate(durationMillis: Long) {}
}

@Composable
fun rememberSky(): Sky = TODO()

@Composable
fun Modifier.sky(sky: Sky): Modifier = TODO()

object CloudyDefaults {
    const val BACKGROUND_RADIUS: Int = 20
    const val PROGRESSIVE_FADE_END: Float = 0.5f
    const val EDGES_FADE_DISTANCE: Float = 0.2f
    const val CPP_BLUR_ENABLED: Boolean = false
}

@Stable
sealed interface CloudyProgressive {
    data object None : CloudyProgressive

    data class TopToBottom(
        val start: Float = 0f,
        val end: Float = CloudyDefaults.PROGRESSIVE_FADE_END,
        val easing: Easing = FastOutSlowInEasing,
    ) : CloudyProgressive

    data class BottomToTop(
        val start: Float = 1f,
        val end: Float = 1f - CloudyDefaults.PROGRESSIVE_FADE_END,
        val easing: Easing = FastOutSlowInEasing,
    ) : CloudyProgressive
}

sealed interface CloudyState

class LiquidGlassLight private constructor()

@Composable
fun Modifier.cloudy(
    sky: Sky,
    radius: Int = CloudyDefaults.BACKGROUND_RADIUS,
    progressive: CloudyProgressive = CloudyProgressive.None,
    tint: Color = Color.Transparent,
    light: LiquidGlassLight? = null,
    enabled: Boolean = true,
    cpuBlurEnabled: Boolean = CloudyDefaults.CPP_BLUR_ENABLED,
    shape: Shape = RectangleShape,
    onStateChanged: (CloudyState) -> Unit = {},
): Modifier = TODO()
