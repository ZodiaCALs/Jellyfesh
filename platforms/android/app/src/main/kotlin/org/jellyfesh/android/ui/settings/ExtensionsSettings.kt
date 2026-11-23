package org.jellyfesh.android.ui.settings

import android.text.format.Formatter
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
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.BuildConfig
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.sources.PluginIcon
import org.jellyfesh.android.ui.sources.PluginSettingsPanel
import org.jellyfesh.android.ui.sources.rememberDisclosures
import org.jellyfesh.android.ui.sources.rememberPluginInstaller
import org.jellyfesh.android.ui.sources.rememberSignIn
import org.jellyfesh.android.ui.sources.signInLabel
import org.jellyfesh.android.ui.theme.Badge
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SpeechModel
import org.jellyfesh.logic.core.SpeechModelStatus
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.settings.AboutInfo
import org.jellyfesh.logic.settings.Extensions
import org.jellyfesh.logic.settings.ProLine
import org.jellyfesh.logic.settings.ProModel
import org.jellyfesh.logic.sources.Disclosure
import org.jellyfesh.logic.state.CoreState

// ---- Settings › Extensions (docs/ui.md; ADR-0019, ADR-0020, ADR-0021) ---------------------------

/**
 * Jellyfesh Pro: where Pro comes from (Pro Preview, a purchase, a licence key with Remove
 * Licence…), or what it holds with the ways to get it; then the Licence Agreement, the Privacy
 * Policy and the Terms of Sale.
 */
internal fun LazyListScope.jellyfeshPro(state: CoreState) {
    item(key = "pro") {
        val graph = LocalGraph.current
        val sheets = rememberProSheets()
        val pro = state.pro
        SettingsCard(stringResource(R.string.pro_title)) {
            when (val line = ProModel.line(pro)) {
                ProLine.Preview -> SettingRow(stringResource(R.string.pro_preview))
                ProLine.Purchased -> SettingRow(stringResource(R.string.pro_purchased))
                is ProLine.Licence -> SettingRow(stringResource(R.string.pro_licence, line.suffix)) {
                    JfButton(stringResource(R.string.pro_remove_licence_ask), { sheets.removeLicence(graph::send) }, style = ButtonStyle.Danger)
                }
                ProLine.Offer -> Column {
                    SettingRow(stringResource(R.string.pro_offer))
                    ProOffers(pro, sheets, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp))
                }
            }
        }
    }
    item(key = "pro-links") {
        val context = LocalContext.current
        val links = remember { AboutInfo.links(BuildConfig.WEBSITE) } ?: return@item
        SettingsCard(null) {
            LinkRow(stringResource(R.string.about_licence_agreement)) { openLink(context, links.licenceAgreement) }
            Divider()
            LinkRow(stringResource(R.string.about_privacy_policy)) { openLink(context, links.privacyPolicy) }
            Divider()
            LinkRow(stringResource(R.string.about_terms_of_sale)) { openLink(context, links.termsOfSale) }
        }
    }
}

/**
 * The installed extensions, each opening its page, with Install from File… in the heading where
 * the build installs plugins. An extension that needs Pro carries the Pro badge while Pro is
 * inactive.
 */
internal fun LazyListScope.installedExtensions(state: CoreState) {
    item(key = "ext") {
        val nav = LocalNav.current
        val extensions = Extensions.list(state.sources.plugins)
        val install = if (state.sources.canInstallPlugins) rememberPluginInstaller() else null
        SettingsCard(
            stringResource(R.string.sources_installed),
            action = install?.let { { JfButton(stringResource(R.string.action_install_plugin), it, style = ButtonStyle.Quiet, icon = R.drawable.ic_add) } },
        ) {
            if (extensions.isEmpty()) SettingRow(stringResource(R.string.extensions_empty))
            extensions.forEachIndexed { i, plugin ->
                if (i > 0) Divider()
                NavRow(
                    title = plugin.name,
                    subtitle = extensionStatus(plugin),
                    onClick = { nav.push(Screen.Extension(plugin.id)) },
                    leading = { PluginIcon(plugin.name, plugin.icon) },
                    badge = stringResource(R.string.pro_badge).takeIf { Extensions.showsProBadge(plugin, state.pro) },
                )
            }
        }
    }
}

/** An extension's line: its badge, and Off while it is off. */
@Composable
private fun extensionStatus(plugin: PluginSummary): String {
    val badge = stringResource(if (plugin.badge == PluginBadge.Official) R.string.badge_official else R.string.badge_community)
    return if (plugin.enabled) badge else listOf(badge, stringResource(R.string.source_disabled)).joinToString(" · ")
}

/** Appearance › Customize…: opens Extensions › Customize, with the Pro badge while Pro is inactive. */
internal fun LazyListScope.customize(state: CoreState) {
    val plugin = Extensions.customize(state.sources.plugins, state.look) ?: return
    item(key = "customize") {
        val nav = LocalNav.current
        SettingsCard(null) {
            NavRow(
                title = stringResource(R.string.customize_row),
                subtitle = stringResource(R.string.customize_row_desc),
                onClick = { nav.push(Screen.Extension(plugin.id)) },
                badge = stringResource(R.string.pro_badge).takeIf { Extensions.showsProBadge(plugin, state.pro) },
            )
        }
    }
}

/**
 * An extension's page: its name and badge; "Part of Jellyfesh Pro." with Learn More… while Pro
 * is inactive; Enabled; its account (AI Lyrics' online mode: Sign In, or "API Key Added" and Sign
 * Out); its settings by group, with Reset to Defaults for an appearance plugin; for AI Lyrics the
 * On-Device Models and Sending Audio; and "Version 1.2.0 · Author".
 */
@Composable
fun ExtensionScreen(pluginId: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val disclosures = rememberDisclosures()
    val sheets = rememberProSheets()
    val plugin = state.sources.plugins.firstOrNull { it.id == pluginId }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            if (plugin == null) {
                item(key = "gone") { EmptyState(R.drawable.ic_sparkle, stringResource(R.string.source_gone), null) }
                return@LazyColumn
            }
            val needsPro = Extensions.showsProBadge(plugin, state.pro)
            item(key = "header") {
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    JfText(plugin.name, style = JfType.title)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Badge(
                            stringResource(if (plugin.badge == PluginBadge.Official) R.string.badge_official else R.string.badge_community),
                            warning = plugin.badge == PluginBadge.Community,
                        )
                        if (needsPro) Badge(stringResource(R.string.pro_badge), warning = false)
                    }
                }
            }
            if (needsPro) {
                item(key = "pro") { Notice(stringResource(R.string.pro_part_of), action = stringResource(R.string.pro_learn_more) to sheets::open) }
            }
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
            if (plugin.authMethods.isNotEmpty()) {
                item(key = "account-h") { SectionHeader(stringResource(R.string.source_account)) }
                item(key = "account") { ExtensionAccount(plugin) }
            }
            item(key = "settings") { PluginSettingsPanel(plugin, resetToDefaults = Extensions.changesTheLook(plugin)) }
            if (Extensions.makesLyrics(plugin) && Extensions.aiLyrics(state.sources.plugins, state.aiLyrics)?.id == plugin.id) {
                val models = state.aiLyrics.models
                if (models.isNotEmpty()) {
                    item(key = "models-h") { SectionHeader(stringResource(R.string.ai_models)) }
                    item(key = "models") {
                        Panel {
                            models.forEachIndexed { i, model ->
                                if (i > 0) Divider()
                                SpeechModelRow(model)
                            }
                        }
                    }
                }
                val consents = Extensions.consents(state.aiLyrics, plugin.id)
                if (consents.isNotEmpty()) {
                    item(key = "sending-h") { SectionHeader(stringResource(R.string.ai_sending_audio)) }
                    item(key = "sending") {
                        Panel {
                            consents.forEachIndexed { i, consent ->
                                if (i > 0) Divider()
                                SettingRow(consent.origin) {
                                    JfButton(stringResource(R.string.ai_stop_allowing), {
                                        graph.send(Command.RevokeAudioUpload(consent.plugin, consent.origin))
                                    }, style = ButtonStyle.Quiet)
                                }
                            }
                        }
                    }
                }
            }
            item(key = "version") {
                JfText(
                    stringResource(R.string.source_version, plugin.version, plugin.author),
                    style = JfType.caption,
                    color = Jf.colors.ink50,
                    modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp),
                )
            }
        }
        BackBar(onBack = nav::back, title = plugin?.name, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

/**
 * An extension's sign-in. A key-only sign-in (AI Lyrics' online mode) is Sign In, then "API Key
 * Added" with Sign Out; others say who is signed in, as sources do.
 */
@Composable
private fun ExtensionAccount(plugin: PluginSummary) {
    val graph = LocalGraph.current
    val signIn = rememberSignIn()
    val keyOnly = plugin.authMethods == listOf(AuthMethod.ApiKey)
    Panel {
        val account = plugin.account
        when {
            account != null -> SettingRow(if (keyOnly) stringResource(R.string.ai_api_key_added) else stringResource(R.string.source_signed_in, account)) {
                JfButton(stringResource(R.string.action_sign_out), { graph.send(Command.Logout(plugin.id)) }, style = ButtonStyle.Quiet, icon = R.drawable.ic_logout)
            }
            keyOnly -> SettingRow(stringResource(R.string.sign_in_submit), onClick = { signIn.start(plugin, AuthMethod.ApiKey) })
            else -> plugin.authMethods.forEachIndexed { i, method ->
                if (i > 0) Divider()
                SettingRow(stringResource(signInLabel(method)), onClick = { signIn.start(plugin, method) })
            }
        }
    }
}

/** An on-device model: "Name · Size" with Download, "Downloading… 35%" with Cancel, or "Ready" with Delete…. */
@Composable
private fun SpeechModelRow(model: SpeechModel) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val title = "${model.name} · ${Formatter.formatShortFileSize(context, model.bytes)}"
    val deleteTitle = stringResource(R.string.ai_model_delete_title, model.name)
    val deleteBody = stringResource(R.string.ai_model_delete_body)
    val deleteLabel = stringResource(R.string.action_delete)
    when (val status = model.state) {
        SpeechModelStatus.NotDownloaded -> SettingRow(title) {
            JfButton(stringResource(R.string.ai_model_download), { graph.send(Command.DownloadSpeechModel(model.id)) })
        }
        is SpeechModelStatus.Downloading -> SettingRow(title, description = stringResource(R.string.ai_model_downloading, (status.fraction.coerceIn(0f, 1f) * 100).toInt())) {
            JfButton(stringResource(R.string.action_cancel), { graph.send(Command.CancelSpeechModelDownload(model.id)) }, style = ButtonStyle.Quiet)
        }
        SpeechModelStatus.Ready -> SettingRow(title, description = stringResource(R.string.ai_model_ready)) {
            JfButton(stringResource(R.string.ai_model_delete_ask), {
                overlays.confirm(deleteTitle, deleteBody, deleteLabel, destructive = true) { graph.send(Command.DeleteSpeechModel(model.id)) }
            }, style = ButtonStyle.Danger)
        }
        is SpeechModelStatus.Failed -> SettingRow(title, description = stringResource(R.string.ai_model_failed, status.message)) {
            JfButton(stringResource(R.string.ai_model_download), { graph.send(Command.DownloadSpeechModel(model.id)) })
        }
    }
}
