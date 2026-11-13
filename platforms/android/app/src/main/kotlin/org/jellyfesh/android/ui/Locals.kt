package org.jellyfesh.android.ui

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.SharedFlow

/** OAuth redirects (`jellyfesh://auth?...`) for the sign-in sheet that waits for one. */
val LocalAuthRedirects = staticCompositionLocalOf<SharedFlow<String>?> { null }

/**
 * Space the screens leave free at the bottom for the floating chrome (mini-player and tab bar),
 * so their last rows can scroll above it.
 */
val LocalChromePadding = staticCompositionLocalOf { PaddingValues(0.dp) }
