package org.jellyfesh.android.ui.sources

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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.SideEffect
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
import org.jellyfesh.android.system.LocalNetworkPermission
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
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.sources.LocalNetwork
import org.jellyfesh.logic.sources.LocalNetworkAccess
import org.jellyfesh.logic.sources.LocalNetworkReason
import org.jellyfesh.logic.sources.LocalNetworkStep

/**
 * Asks for Local Network access (Android 17) before Jellyfesh connects to a server the core calls
 * local, and again only when the person chooses to after a denial, with the fuller reason
 * ([LocalNetwork]). One per window, from [ProvideLocalNetwork].
 *
 * The answer is advice: what was about to happen (signing in, turning a source on) goes ahead
 * whatever it is. An action waiting for Android's dialog is lost if the window is recreated
 * meanwhile; the person starts it again.
 */
@Stable
class LocalNetworkGate internal constructor(
    private val permission: LocalNetworkPermission,
    private val overlays: Overlays,
    private val context: android.content.Context,
    private val activity: () -> Activity?,
) {
    /** What Android said when last read: after an answer, and whenever the window comes back to the front. */
    private var facts by mutableStateOf(read())

    /** Shows Android's dialog; set by [ProvideLocalNetwork]. */
    internal var request: () -> Unit = {}

    /** What runs once Android's dialog answers. */
    private var pending: Pair<List<String>, () -> Unit>? = null

    private data class Facts(val granted: Boolean, val asked: Boolean, val rationale: Boolean)

    private fun read() = Facts(permission.granted(), permission.asked, permission.rationale(activity()))

    /** Where the permission stands while [anyLocal] sources need it (snapshot state: views follow it). */
    fun access(anyLocal: Boolean): LocalNetworkAccess =
        LocalNetwork.access(Build.VERSION.SDK_INT, anyLocal, facts.granted, facts.asked, facts.rationale)

    /**
     * Before connecting to [name] at an address the core calls local ([local]): the sheet and
     * Android's dialog when Android 17 needs them, then [connect] whatever the answer.
     */
    fun beforeConnecting(name: String, local: Boolean, connect: () -> Unit) {
        if (!LocalNetwork.needsPermission(local, Build.VERSION.SDK_INT, permission.granted())) {
            connect()
            return
        }
        show(listOf(name), connect)
    }

    /** A notice's button, or Jellyfesh opening with sources on the home network: the sheet, then Android. */
    fun ask(names: List<String>) = show(names) {}

    /** Android's dialog answered. */
    internal fun answered(granted: Boolean) {
        permission.asked = true
        facts = read()
        val (names, then) = pending ?: return
        pending = null
        if (!granted && names.size == 1) overlays.toast(context.getString(R.string.local_network_denied, names.single()))
        then()
    }

    /** The window came back to the front: the person may have changed it in Android's settings. */
    internal fun recheck() {
        facts = read()
    }

    private fun show(names: List<String>, then: () -> Unit) {
        facts = read()
        val access = access(anyLocal = true)
        val step = LocalNetwork.step(access)
        if (step == null) {
            then()
            return
        }
        overlays.showSheet(context.getString(R.string.local_network_ask_title)) { dismiss ->
            LocalNetworkSheet(
                names = names,
                reason = LocalNetwork.reason(access),
                step = step,
                onNotNow = {
                    dismiss()
                    then()
                },
                onContinue = {
                    dismiss()
                    when (step) {
                        LocalNetworkStep.Ask -> {
                            pending = names to then
                            request()
                        }
                        LocalNetworkStep.OpenSettings -> {
                            permission.openSettings(activity())
                            then()
                        }
                    }
                },
            )
        }
    }
}

/** The window's gate, or null outside the main UI (previews, the start-up screen). */
val LocalLocalNetwork = staticCompositionLocalOf<LocalNetworkGate?> { null }

/**
 * Provides the window's [LocalNetworkGate] to [content]: Android's permission dialog, a check
 * whenever the window comes back to the front, and the one time Jellyfesh asks by itself, when it
 * opens with enabled sources the core calls local and has never asked ([LocalNetwork.asksAtStart]).
 */
@Composable
fun ProvideLocalNetwork(plugins: List<PluginSummary>, content: @Composable () -> Unit) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val gate = remember(graph, overlays, context) {
        LocalNetworkGate(graph.localNetwork, overlays, context) { context.findActivity() }
    }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted -> gate.answered(granted) }
    SideEffect { gate.request = { launcher.launch(LocalNetwork.PERMISSION) } }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle, gate) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) { gate.recheck() }
    }
    val names = LocalNetwork.localSources(plugins).map { it.name }
    val access = gate.access(anyLocal = names.isNotEmpty())
    val sheetOpen = overlays.isOpen
    LaunchedEffect(names, access, sheetOpen) {
        if (names.isEmpty() || sheetOpen) return@LaunchedEffect
        if (LocalNetwork.asksAtStart(access, graph.localNetwork.introduced)) {
            graph.localNetwork.introduced = true
            gate.ask(names)
        }
    }
    androidx.compose.runtime.CompositionLocalProvider(LocalLocalNetwork provides gate, content = content)
}

/**
 * "Local Network Access Is Off" while enabled sources the core calls local cannot be reached:
 * which ones, and Allow… (the sheet, then Android's dialog or its settings page). Nothing once
 * access is granted, before Android 17, or when no source needs it.
 */
@Composable
fun LocalNetworkNotice(plugins: List<PluginSummary>, modifier: Modifier = Modifier) {
    val gate = LocalLocalNetwork.current ?: return
    val local = LocalNetwork.localSources(plugins)
    if (local.isEmpty()) return
    val notice = LocalNetwork.notice(gate.access(anyLocal = true), local) ?: return
    val names = notice.names
    val text = when {
        notice.step == LocalNetworkStep.OpenSettings -> stringResource(R.string.local_network_blocked)
        names.size == 1 -> stringResource(R.string.local_network_off_one, names.single())
        else -> stringResource(R.string.local_network_off_many)
    }
    val label = stringResource(if (notice.step == LocalNetworkStep.OpenSettings) R.string.local_network_open_settings else R.string.local_network_allow)
    Notice(
        text,
        modifier,
        warning = true,
        title = stringResource(R.string.local_network_off_title),
        action = label to { gate.ask(names) },
    )
}

/** The sheet before Android's dialog: why, then Not Now and Continue (or Open Settings once Android stops asking). */
@Composable
private fun LocalNetworkSheet(
    names: List<String>,
    reason: LocalNetworkReason,
    step: LocalNetworkStep,
    onNotNow: () -> Unit,
    onContinue: () -> Unit,
) {
    val one = names.singleOrNull()
    val why = when (reason) {
        LocalNetworkReason.Short -> if (one != null) stringResource(R.string.local_network_reason_one, one) else stringResource(R.string.local_network_reason_many)
        LocalNetworkReason.Full -> if (one != null) stringResource(R.string.local_network_reason_full_one, one) else stringResource(R.string.local_network_reason_full_many)
    }
    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        JfText(why, style = JfType.small, color = Jf.colors.ink90)
        if (step == LocalNetworkStep.OpenSettings) JfText(stringResource(R.string.local_network_blocked), style = JfType.small, color = Jf.colors.ink70)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            JfButton(stringResource(R.string.pro_not_now), onNotNow, style = ButtonStyle.Quiet)
            JfButton(
                stringResource(if (step == LocalNetworkStep.OpenSettings) R.string.local_network_open_settings else R.string.local_network_continue),
                onContinue,
                style = ButtonStyle.Accent,
            )
        }
    }
}
