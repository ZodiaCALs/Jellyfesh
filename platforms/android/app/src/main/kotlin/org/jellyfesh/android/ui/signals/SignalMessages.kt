package org.jellyfesh.android.ui.signals

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.ScanStatus

/**
 * Turns the core's discrete events into short messages: failed commands (a feature still being
 * integrated reads as "not available in this build yet"), playback errors, the end of a scan, a
 * new playlist, and why a remote session ended ("Sign in to Spotify again."), once: the store
 * forwards only the Remote slice that ends a session. Search results and sign-in steps go to the
 * screens waiting for them.
 */
@Composable
fun SignalMessages(graph: AppGraph, overlays: Overlays) {
    val context = LocalContext.current
    LaunchedEffect(graph) {
        graph.store.signals.collect { event ->
            val text = when (event) {
                is CoreEvent.CommandFailed -> describeFailure(context, event.error)
                is CoreEvent.Error -> event.message
                is CoreEvent.Scan -> when (val s = event.status) {
                    is ScanStatus.Finished ->
                        if (s.added + s.updated + s.removed > 0) {
                            context.getString(R.string.toast_scan_done, s.added, s.updated, s.removed)
                        } else {
                            null
                        }
                    is ScanStatus.Failed -> context.getString(R.string.toast_scan_failed, s.message)
                    is ScanStatus.Progress -> null
                }
                is CoreEvent.PlaylistCreated -> context.getString(R.string.toast_playlist_created)
                is CoreEvent.Remote -> event.snapshot.error?.takeIf { it.isNotBlank() }
                else -> null
            }
            if (text != null) overlays.toast(text)
        }
    }
}

/**
 * A command failure as the person reads it. The core's messages are English sentences for now;
 * the one the shell recognises is the "not available in this build yet" of features still being
 * integrated in this release.
 */
fun describeFailure(context: android.content.Context, error: String): String =
    if (isNotAvailable(error)) context.getString(R.string.not_available_yet) else error

fun isNotAvailable(error: String): Boolean = error.contains("not available", ignoreCase = true)
