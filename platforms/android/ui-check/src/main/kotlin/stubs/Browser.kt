// Stand-ins for androidx.browser.customtabs (browser 1.10). Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.browser.customtabs

import android.content.Context
import android.net.Uri

class CustomTabsIntent private constructor() {
    fun launchUrl(context: Context, url: Uri) {}

    class Builder {
        fun setShowTitle(showTitle: Boolean): Builder = this
        fun build(): CustomTabsIntent = TODO()
    }
}
