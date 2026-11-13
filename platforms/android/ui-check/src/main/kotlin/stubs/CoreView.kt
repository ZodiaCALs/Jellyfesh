// Stand-ins for androidx.core.view (core-ktx 1.19). Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.core.view

import android.view.View
import android.view.Window

class WindowInsetsControllerCompat private constructor() {
    var isAppearanceLightStatusBars: Boolean = false
    var isAppearanceLightNavigationBars: Boolean = false
}

object WindowCompat {
    @JvmStatic fun getInsetsController(window: Window, view: View): WindowInsetsControllerCompat = TODO()

    @JvmStatic fun setDecorFitsSystemWindows(window: Window, decorFitsSystemWindows: Boolean) {}
}
