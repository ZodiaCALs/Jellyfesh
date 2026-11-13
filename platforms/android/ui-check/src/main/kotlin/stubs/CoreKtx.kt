// Stand-ins for androidx.core (core-ktx 1.19). Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.core.content

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.drawable.Drawable

object ContextCompat {
    const val RECEIVER_VISIBLE_TO_INSTANT_APPS: Int = 0x1
    const val RECEIVER_EXPORTED: Int = 0x2
    const val RECEIVER_NOT_EXPORTED: Int = 0x4

    @JvmStatic fun checkSelfPermission(context: Context, permission: String): Int = TODO()

    @JvmStatic fun getDrawable(context: Context, id: Int): Drawable? = TODO()

    @JvmStatic fun getColor(context: Context, id: Int): Int = TODO()

    @JvmStatic fun registerReceiver(context: Context, receiver: BroadcastReceiver?, filter: IntentFilter, flags: Int): Intent? = TODO()
}
