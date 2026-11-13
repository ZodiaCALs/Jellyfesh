// Stand-ins for androidx.core.graphics.drawable and androidx.core.app (core-ktx 1.19).
@file:Suppress("unused", "UNUSED_PARAMETER")
@file:JvmName("DrawableKt")

package androidx.core.graphics.drawable

import android.graphics.Bitmap
import android.graphics.drawable.Drawable

fun Drawable.toBitmap(width: Int = intrinsicWidth, height: Int = intrinsicHeight, config: Bitmap.Config? = null): Bitmap = TODO()
