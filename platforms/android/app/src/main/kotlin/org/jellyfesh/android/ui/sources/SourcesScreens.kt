package org.jellyfesh.android.ui.sources

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.InitialTile
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.search.SourceItemRow
import org.jellyfesh.android.ui.search.openSourceItem
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.Badge
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.statusBarFade
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.BrowseUpdate
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.nav.SettingsSection
import org.jellyfesh.logic.sources.Disclosure
import org.jellyfesh.logic.state.CoreState

/** The installed sources: Official and Community plugins, their state, and installing a new one. */
@Composable
fun SourcesScreen(state: CoreState) {
    val nav = LocalNav.current
    val install = rememberPluginInstaller()
    val plugins = state.sources.plugins
    LazyColumn(Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "title") {
            ScreenTitle(
                stringResource(R.string.tab_sources),
                Modifier.windowInsetsPadding(WindowInsets.statusBars),
                subtitle = stringResource(R.string.sources_subtitle),
            )
        }
        item(key = "actions") {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                JfButton(stringResource(R.string.action_install_plugin), install, icon = R.drawable.ic_add)
                JfButton(stringResource(R.string.settings_title), { nav.openSettings(SettingsSection.Sources) }, style = ButtonStyle.Quiet, icon = R.drawable.ic_settings)
            }
        }
        if (plugins.isEmpty()) {
            item(key = "empty") { EmptyState(R.drawable.ic_sources, stringResource(R.string.sources_empty_title), stringResource(R.string.sources_empty_body)) }
        }
        items(plugins, key = { it.id }) { plugin ->
            NavRow(
                title = plugin.name,
                subtitle = pluginStatus(plugin),
                onClick = { nav.push(Screen.Source(plugin.id)) },
                leading = { InitialTile(plugin.name, icon = R.drawable.ic_sources) },
            )
        }
    }
}

@Composable
private fun pluginStatus(plugin: PluginSummary): String {
    val badge = stringResource(if (plugin.badge == PluginBadge.Official) R.string.badge_official else R.string.badge_community)
    val account = plugin.account
    val state = when {
        !plugin.enabled -> stringResource(R.string.source_disabled)
        account != null -> stringResource(R.string.source_signed_in, account)
        plugin.authMethods.isNotEmpty() -> stringResource(R.string.source_not_signed_in)
        else -> stringResource(R.string.source_enabled)
    }
    return "$badge · $state"
}

/**
 * Picks a `.jfpkg` with the system's document picker and hands it to the core (copied to the
 * cache first: the core installs from a path). Returns the action that opens the picker.
 */
@Composable
fun rememberPluginInstaller(): () -> Unit {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val ok = graph.library.installPlugin(uri)
            overlays.toast(context.getString(if (ok) R.string.source_installing else R.string.source_install_failed))
        }
    }
    return remember(picker) { { picker.launch(arrayOf("application/octet-stream", "application/zip", "application/x-jfpkg")) } }
}

/** Asks before enabling a community plugin the first time, or allowing community plugins (ADR-0012). */
@Stable
class Disclosures internal constructor(private val overlays: Overlays, private val context: android.content.Context) {
    fun show(title: String, onAccept: () -> Unit) {
        overlays.showSheet(title) { dismiss ->
            Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                listOf(R.string.disclosure_1, R.string.disclosure_2, R.string.disclosure_3, R.string.disclosure_4).forEach {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        JfText("•", style = JfType.small, color = Jf.colors.ink70)
                        JfText(stringResource(it), style = JfType.small, color = Jf.colors.ink90)
                    }
                }
                Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    JfButton(stringResource(R.string.action_cancel), dismiss, style = ButtonStyle.Quiet)
                    JfButton(stringResource(R.string.disclosure_accept), {
                        dismiss()
                        onAccept()
                    }, style = ButtonStyle.Accent)
                }
            }
        }
    }

    fun allowCommunity(onAccept: () -> Unit) = show(context.getString(R.string.disclosure_allow_title), onAccept)

    fun enable(plugin: PluginSummary, onAccept: () -> Unit) = show(context.getString(R.string.disclosure_enable_title, plugin.name), onAccept)
}

@Composable
fun rememberDisclosures(): Disclosures {
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    return remember(overlays, context) { Disclosures(overlays, context) }
}

/** One plugin: its badge, switch, account and sign-in, settings, browsing and removal. */
@Composable
fun SourceScreen(pluginId: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val overlays = LocalOverlays.current
    val disclosures = rememberDisclosures()
    val plugin = state.sources.plugins.firstOrNull { it.id == pluginId }
    val uninstallTitle = plugin?.let { stringResource(R.string.source_uninstall_title, it.name) }
    val uninstallBody = stringResource(R.string.source_uninstall_body)
    val uninstallLabel = stringResource(R.string.action_uninstall)
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            if (plugin == null) {
                item(key = "gone") { EmptyState(R.drawable.ic_sources, stringResource(R.string.source_gone), null) }
                return@LazyColumn
            }
            item(key = "header") {
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    JfText(plugin.name, style = JfType.title)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Badge(
                            stringResource(if (plugin.badge == PluginBadge.Official) R.string.badge_official else R.string.badge_community),
                            warning = plugin.badge == PluginBadge.Community,
                        )
                        JfText(stringResource(R.string.source_version, plugin.version, plugin.author), style = JfType.caption, color = Jf.colors.ink50, maxLines = 1)
                    }
                }
            }
            if (plugin.vendorPlayback) item(key = "vendor") { Notice(stringResource(R.string.source_vendor_playback)) }
            item(key = "enable") {
                Panel {
                    SettingRow(stringResource(R.string.source_enabled)) {
                        JfSwitch(plugin.enabled, { on ->
                            if (Disclosure.needsDisclosureToEnable(plugin, on)) {
                                disclosures.enable(plugin) { Disclosure.acceptedFor(plugin).forEach(graph::send) }
                            } else {
                                graph.send(Command.SetPluginEnabled(plugin.id, on))
                            }
                        })
                    }
                }
            }
            item(key = "account-h") { SectionHeader(stringResource(R.string.source_account)) }
            item(key = "account") { AccountPanel(plugin) }
            item(key = "settings") { PluginSettingsPanel(plugin) }
            if ("browse" in plugin.capabilities && plugin.enabled) {
                item(key = "browse") {
                    Panel {
                        SettingRow(stringResource(R.string.source_browse), onClick = { nav.push(Screen.SourceBrowse(plugin.id, null, plugin.name)) })
                    }
                }
            }
            item(key = "uninstall") {
                Row(Modifier.padding(16.dp)) {
                    JfButton(uninstallLabel, {
                        overlays.confirm(uninstallTitle ?: uninstallLabel, uninstallBody, uninstallLabel, destructive = true) {
                            graph.send(Command.UninstallPlugin(plugin.id))
                            nav.back()
                        }
                    }, style = ButtonStyle.Danger, icon = R.drawable.ic_trash)
                }
            }
        }
        BackBar(onBack = nav::back, title = plugin?.name, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

@Composable
private fun AccountPanel(plugin: PluginSummary) {
    val graph = LocalGraph.current
    val signIn = rememberSignIn()
    Panel {
        val account = plugin.account
        when {
            account != null -> SettingRow(stringResource(R.string.source_signed_in, account)) {
                JfButton(stringResource(R.string.action_sign_out), { graph.send(Command.Logout(plugin.id)) }, style = ButtonStyle.Quiet, icon = R.drawable.ic_logout)
            }
            plugin.authMethods.isEmpty() -> SettingRow(stringResource(R.string.source_no_sign_in))
            else -> plugin.authMethods.forEachIndexed { i, method ->
                if (i > 0) Divider()
                SettingRow(stringResource(signInLabel(method)), onClick = { signIn.start(plugin, method) }) {
                    org.jellyfesh.android.ui.components.JfIcon(signInIcon(method), Jf.colors.ink50, size = 20.dp)
                }
            }
        }
    }
}

fun signInLabel(method: AuthMethod): Int = when (method) {
    AuthMethod.Qr -> R.string.sign_in_qr
    AuthMethod.Oauth -> R.string.sign_in_browser
    AuthMethod.CookieImport -> R.string.sign_in_cookies
    AuthMethod.PasswordExchange -> R.string.sign_in_password
    AuthMethod.ApiKey -> R.string.sign_in_api_key
}

private fun signInIcon(method: AuthMethod): Int = when (method) {
    AuthMethod.Qr -> R.drawable.ic_qr
    AuthMethod.Oauth -> R.drawable.ic_external
    else -> R.drawable.ic_shield
}

/**
 * A source's own folders and lists (`Command.Browse`), page by page: the core answers with a
 * BrowsePage event for the request id `send` returned.
 */
@Composable
fun SourceBrowseScreen(pluginId: String, node: String?, title: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val context = LocalContext.current
    var items by remember(pluginId, node) { mutableStateOf<List<SourceItem>>(emptyList()) }
    var nextPage by remember(pluginId, node) { mutableStateOf<Int?>(0) }
    var request by remember(pluginId, node) { mutableStateOf(0L) }
    var error by remember(pluginId, node) { mutableStateOf<String?>(null) }
    var loading by remember(pluginId, node) { mutableStateOf(false) }
    fun load(page: Int) {
        loading = true
        request = graph.send(Command.Browse(pluginId, node, page))
        if (request == 0L) {
            loading = false
            error = context.getString(R.string.not_available_yet)
        }
    }
    LaunchedEffect(pluginId, node) {
        graph.store.signals.collect { event ->
            when {
                event is CoreEvent.BrowsePage && event.update.request == request -> {
                    val update: BrowseUpdate = event.update
                    items = items + update.items
                    nextPage = update.nextPage
                    error = update.error
                    loading = false
                }
                event is CoreEvent.CommandFailed && event.id == request -> {
                    error = describeFailure(context, event.error)
                    loading = false
                }
            }
        }
    }
    LaunchedEffect(pluginId, node) { if (items.isEmpty()) load(0) }
    val playing = state.nowPlaying?.track
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "title") { ScreenTitle(title) }
            val playable = items.mapNotNull { it.track }
            itemsIndexed(items, key = { i, it -> "$i-${it.id}" }) { _, item ->
                SourceItemRow(item, playing) { openSourceItem(item, playable, graph::send) { nav.push(it) } }
            }
            val e = error
            if (e != null) item(key = "error") { Notice(e, warning = true) }
            when {
                loading -> item(key = "loading") { LoadingRow() }
                nextPage != null && items.isNotEmpty() -> item(key = "more") {
                    Row(Modifier.padding(16.dp)) { JfButton(stringResource(R.string.action_load_more), { nextPage?.let(::load) }) }
                }
                items.isEmpty() && e == null -> item(key = "empty") { EmptyState(R.drawable.ic_folder, stringResource(R.string.browse_empty), null) }
            }
        }
        BackBar(onBack = nav::back, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}
