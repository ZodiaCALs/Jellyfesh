// Stand-ins for the Android-only parts of Jetpack Compose ui 1.12 that Compose Multiplatform's
// desktop artifacts do not have: resources by id, the Android composition locals and Bitmap
// conversion. Same names and signatures as on Android. Bodies never run.
@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.compose.ui.res

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.painter.Painter

@Composable fun stringResource(id: Int): String = TODO()

@Composable fun stringResource(id: Int, vararg formatArgs: Any): String = TODO()

@Composable fun pluralStringResource(id: Int, count: Int): String = TODO()

@Composable fun pluralStringResource(id: Int, count: Int, vararg formatArgs: Any): String = TODO()

@Composable fun painterResource(id: Int): Painter = TODO()
