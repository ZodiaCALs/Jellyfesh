// Stand-ins for androidx.compose.ui.platform's Android composition locals (Compose ui 1.12).
@file:Suppress("unused")

package androidx.compose.ui.platform

import android.content.Context
import android.content.res.Configuration
import android.view.View
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.staticCompositionLocalOf

val LocalContext: ProvidableCompositionLocal<Context> = staticCompositionLocalOf { error("stub") }

val LocalConfiguration: ProvidableCompositionLocal<Configuration> = staticCompositionLocalOf { error("stub") }

val LocalView: ProvidableCompositionLocal<View> = staticCompositionLocalOf { error("stub") }
