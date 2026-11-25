package org.jellyfesh.android.ui.library

import android.app.Activity
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import org.jellyfesh.android.R
import org.jellyfesh.android.system.StoragePermission
import org.jellyfesh.android.system.openAppSettings
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.findActivity
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.library.StorageAccess
import org.jellyfesh.logic.library.StorageGrant
import org.jellyfesh.logic.library.StorageStep

/**
 * Downloads on Android 8 and 9 (ADR-0025 §2; [StorageAccess]): Android's storage dialog once,
 * before the first download, which then goes ahead if it is allowed. Once refused, a download fails
 * with why and the way to allow it later: Android's dialog again while it shows it, else
 * Jellyfesh's settings page. Android 10 and later download at once. One per window, from
 * [ProvideDownloadGate]. A download waiting for Android's dialog is lost if the window is recreated
 * meanwhile; the person chooses Download again.
 */
@Stable
class DownloadGate internal constructor(
    private val permission: StoragePermission,
    private val overlays: Overlays,
    private val context: android.content.Context,
    private val activity: () -> Activity?,
    private val send: (Command) -> Long,
) {
    private data class Facts(val granted: Boolean, val asked: Boolean, val rationale: Boolean)

    /** What Android said when last read: after an answer, and whenever the window comes back to the front. */
    private var facts by mutableStateOf(read())

    /** Shows Android's dialog; set by [ProvideDownloadGate]. */
    internal var request: () -> Unit = {}

    /** The download waiting for Android's dialog. */
    private var pending: Command? = null

    private fun read() = Facts(permission.granted(), permission.asked, permission.rationale(activity()))

    /** Whether Downloads and Settings › Downloads say storage access is off (snapshot state: views follow it). */
    val off: Boolean get() = StorageAccess.showsOff(Build.VERSION.SDK_INT, facts.granted, facts.asked)

    /** What Allow… does now: Android's dialog again while it shows it, else the settings page. */
    val grant: StorageGrant get() = StorageAccess.grant(facts.rationale)

    /** Download, Try Again: sent once storage allows it, asking Android first the one time. */
    fun download(command: Command) {
        facts = read()
        when (StorageAccess.beforeDownload(Build.VERSION.SDK_INT, facts.granted, facts.asked)) {
            StorageStep.Download -> send(command)
            StorageStep.Ask -> {
                pending = command
                request()
            }
            StorageStep.Refused -> refused()
        }
    }

    /** Allow… (the notice, the sheet): Android's dialog again while it shows it, else the settings page. */
    fun allow() {
        facts = read()
        when (grant) {
            StorageGrant.Ask -> request()
            StorageGrant.OpenSettings -> openAppSettings(context, activity())
        }
    }

    /** Android's dialog answered. */
    internal fun answered(granted: Boolean) {
        permission.asked = true
        facts = read()
        val command = pending ?: return
        pending = null
        when (StorageAccess.afterAnswer(granted)) {
            StorageStep.Download -> send(command)
            else -> refused()
        }
    }

    /** The window came back to the front: the person may have allowed it in Android's settings. */
    internal fun recheck() {
        facts = read()
    }

    /** The download fails: why, and Allow… or Open Settings. */
    private fun refused() {
        val settings = grant == StorageGrant.OpenSettings
        overlays.showSheet(context.getString(R.string.storage_off_title)) { dismiss ->
            Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                JfText(stringResource(R.string.storage_off_body), style = JfType.small, color = Jf.colors.ink90)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    JfButton(stringResource(R.string.pro_not_now), dismiss, style = ButtonStyle.Quiet)
                    JfButton(
                        stringResource(if (settings) R.string.local_network_open_settings else R.string.local_network_allow),
                        {
                            dismiss()
                            allow()
                        },
                        style = ButtonStyle.Accent,
                    )
                }
            }
        }
    }
}

/** The window's gate, or null outside the main UI (previews, the start-up screen). */
val LocalDownloadGate = staticCompositionLocalOf<DownloadGate?> { null }

/** Provides the window's [DownloadGate] to [content]: Android's permission dialog, and a check whenever the window comes back to the front. */
@Composable
fun ProvideDownloadGate(content: @Composable () -> Unit) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val gate = remember(graph, overlays, context) {
        DownloadGate(graph.storage, overlays, context, { context.findActivity() }, graph::send)
    }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted -> gate.answered(granted) }
    SideEffect { gate.request = { launcher.launch(StorageAccess.PERMISSION) } }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle, gate) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) { gate.recheck() }
    }
    CompositionLocalProvider(LocalDownloadGate provides gate, content = content)
}

/** "Storage access is off…" with Allow… or Open Settings, on Android 8 and 9 once storage was refused. */
@Composable
fun StorageOffNotice(modifier: Modifier = Modifier) {
    val gate = LocalDownloadGate.current ?: return
    if (!gate.off) return
    val label = stringResource(if (gate.grant == StorageGrant.OpenSettings) R.string.local_network_open_settings else R.string.local_network_allow)
    Notice(stringResource(R.string.storage_off_notice), modifier, warning = true, action = label to gate::allow)
}
