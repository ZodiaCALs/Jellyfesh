package org.jellyfesh.android.ui.sources

import org.jellyfesh.logic.core.ItemRef
import org.jellyfesh.android.ui.player.rememberDetails
import org.jellyfesh.android.ui.player.KnownInfo
import org.jellyfesh.android.ui.player.Blurb
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.images.rememberArtwork
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
import org.jellyfesh.android.ui.library.AlbumHeader
import org.jellyfesh.android.ui.library.PlayButtons
import org.jellyfesh.android.ui.library.SongsFooter
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.search.SourceItemRow
import org.jellyfesh.android.ui.search.runItemAction
import org.jellyfesh.android.ui.search.sourceItemMenu
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.Badge
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.statusBarFade
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.AuthMethod
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.PluginBadge
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.SourcePreset
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.nav.SettingsSection
import org.jellyfesh.logic.sources.AuthFlow
import org.jellyfesh.logic.sources.BrowseModel
import org.jellyfesh.logic.sources.BrowseState
import org.jellyfesh.logic.sources.Disclosure
import org.jellyfesh.logic.sources.Links
import org.jellyfesh.logic.sources.SourceItems
import org.jellyfesh.logic.sources.SourceState
import org.jellyfesh.logic.sources.YourSources
import org.jellyfesh.logic.state.CoreState

/**
 * The Sources tab (docs/ui.md, "Sources"): what sources are, Add a Source (the core's services),
 * Your Sources (each installed source with its icon and status; choosing one browses it, or opens
 * its page while it is off or has nothing to browse), then Sources Settings…, which opens
 * Settings › Sources (where Install from File… is, as in every app).
 */
@Composable
fun SourcesScreen(state: CoreState) {
    val nav = LocalNav.current
    val connect = rememberConnectSheet()
    // Extensions (AI Lyrics, Customize) are in Settings › Extensions, not among the sources.
    val plugins = YourSources.sources(state.sources.plugins)
    val presets = state.sources.presets
    LazyColumn(Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "title") {
            ScreenTitle(
                stringResource(R.string.tab_sources),
                Modifier.windowInsetsPadding(WindowInsets.statusBars),
                subtitle = stringResource(R.string.sources_subtitle),
            ) {
                JfIconButton(R.drawable.ic_settings, stringResource(R.string.action_settings), { nav.openSettings() }, tint = Jf.colors.ink90)
            }
        }
        // The services Jellyfesh offers, each while its official plugin is installed.
        if (presets.isNotEmpty()) {
            item(key = "add-h") { SectionHeader(stringResource(R.string.sources_add)) }
            items(presets, key = { "preset:${it.id}" }) { preset ->
                PresetRow(preset, state.sources.plugins) { connect.open(preset) }
            }
        }
        item(key = "yours-h") { SectionHeader(stringResource(R.string.sources_your)) }
        if (plugins.isEmpty()) {
            item(key = "empty") { EmptyState(R.drawable.ic_sources, stringResource(R.string.sources_empty_title), stringResource(R.string.sources_empty_body)) }
        }
        items(plugins, key = { it.id }) { plugin ->
            NavRow(
                title = plugin.name,
                subtitle = pluginStatus(plugin),
                onClick = { nav.push(YourSources.opens(plugin)) },
                leading = { PluginIcon(plugin.name, plugin.icon) },
            )
        }
        item(key = "actions") {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                JfButton(stringResource(R.string.sources_settings), { nav.openSettings(SettingsSection.Sources) }, icon = R.drawable.ic_settings)
            }
        }
    }
}

/**
 * A source's line in a list: its badge, then "Off", "Signed in as …" or "Not signed in"; a source
 * that needs no sign-in shows its badge alone ([YourSources.state]).
 */
@Composable
fun pluginStatus(plugin: PluginSummary): String {
    val badge = stringResource(if (plugin.badge == PluginBadge.Official) R.string.badge_official else R.string.badge_community)
    val state = when (YourSources.state(plugin)) {
        SourceState.Off -> stringResource(R.string.source_disabled)
        SourceState.SignedIn -> stringResource(R.string.source_signed_in, plugin.account.orEmpty())
        SourceState.NotSignedIn -> stringResource(R.string.source_not_signed_in)
        SourceState.Ready -> null
    }
    return listOfNotNull(badge, state).joinToString(" · ")
}

/** A service of Add a Source (the Sources tab and Settings › Sources): its picture, name and line. */
@Composable
fun PresetRow(preset: SourcePreset, plugins: List<PluginSummary>, onClick: () -> Unit) {
    NavRow(
        title = preset.name,
        subtitle = presetDescription(preset),
        onClick = onClick,
        leading = { PluginIcon(preset.name, plugins.firstOrNull { it.id == preset.plugin }?.icon) },
    )
}

/** The line under a service in Add a Source: the account once signed in, else what the service is. */
@Composable
private fun presetDescription(preset: SourcePreset): String? {
    val account = preset.account
    val about = presetAbout(preset.id)
    return when {
        account != null -> stringResource(R.string.source_signed_in, account)
        about != null -> stringResource(about)
        else -> null
    }
}

/** What a service is, by its stable id; null for one this version of the app does not know yet. */
private fun presetAbout(id: String): Int? = when (id) {
    "jellyfin" -> R.string.sources_add_jellyfin
    "navidrome" -> R.string.sources_add_navidrome
    "subsonic" -> R.string.sources_add_subsonic
    "qqmusic" -> R.string.sources_add_qqmusic
    "spotify" -> R.string.sources_add_spotify
    else -> null
}

/**
 * A source's or a service's picture: its plugin's icon (a file in the core's cache), drawn as the
 * package made it, since it comes shaped already; the sources glyph while it loads or when the
 * package has none. Add a Source, Your Sources and Settings › Sources › Installed show it.
 */
@Composable
fun PluginIcon(name: String, icon: String?) {
    val size = 44.dp
    val px = with(LocalDensity.current) { size.roundToPx() }
    val image = rememberArtwork(LocalGraph.current.images, icon?.let { ArtKey.File(it) }, px)
    if (image != null) {
        Image(image, contentDescription = null, modifier = Modifier.size(size))
    } else {
        InitialTile(name, size, icon = R.drawable.ic_sources)
    }
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

/**
 * "Connect to …" for a service in Add a Source: its server's address (for a self-hosted server)
 * and one button per way to sign in. A button closes the sheet and starts the sign-in, whose
 * steps the sign-in sheet then shows.
 */
@Stable
class ConnectSheet internal constructor(
    private val overlays: Overlays,
    private val context: android.content.Context,
    private val signIn: SignIn,
) {
    fun open(preset: SourcePreset) {
        overlays.showSheet(context.getString(R.string.sources_connect_title, preset.name)) { dismiss ->
            ConnectContent(preset, dismiss) { address, method -> signIn.connect(preset, address, method) }
        }
    }
}

@Composable
fun rememberConnectSheet(): ConnectSheet {
    val overlays = LocalOverlays.current
    val context = LocalContext.current
    val signIn = rememberSignIn()
    return remember(overlays, context, signIn) { ConnectSheet(overlays, context, signIn) }
}

@Composable
private fun ConnectContent(preset: SourcePreset, dismiss: () -> Unit, connect: (address: String, method: AuthMethod) -> Unit) {
    // Starts from the address the plugin has now, so connecting again only needs a sign-in method.
    var address by remember { mutableStateOf(preset.address.orEmpty()) }
    val ready = AuthFlow.canConnect(preset, address)
    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (preset.needsAddress) {
            JfTextField(
                value = address,
                onValueChange = { address = it },
                label = stringResource(R.string.sources_connect_address),
                placeholder = preset.addressExample,
                keyboard = KeyboardType.Uri,
            )
        }
        preset.authMethods.forEach { method ->
            JfButton(
                stringResource(signInLabel(method)),
                {
                    dismiss()
                    connect(address, method)
                },
                modifier = Modifier.fillMaxWidth(),
                style = if (method == AuthMethod.PasswordExchange) ButtonStyle.Accent else ButtonStyle.Tonal,
                icon = signInIcon(method),
                enabled = ready,
            )
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            JfButton(stringResource(R.string.action_cancel), dismiss, style = ButtonStyle.Quiet)
        }
    }
}

/**
 * One source (docs/ui.md, Settings › Sources › Installed): its name, badge and whether it is
 * signed in; then Enabled, the sign-in buttons (or Sign Out), its own settings, browsing it,
 * "Version 1.2.0 · Author" and Uninstall…, which asks first.
 */
@Composable
fun SourceScreen(pluginId: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val overlays = LocalOverlays.current
    val disclosures = rememberDisclosures()
    val plugin = state.sources.plugins.firstOrNull { it.id == pluginId }
    val uninstallTitle = plugin?.let { stringResource(R.string.source_uninstall_title, it.name) }
    val uninstallBody = stringResource(R.string.source_uninstall_body)
    val uninstallAsk = stringResource(R.string.action_uninstall_ask)
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
                        val account = plugin.account
                        val signedIn = when {
                            account != null -> stringResource(R.string.source_signed_in, account)
                            plugin.authMethods.isNotEmpty() -> stringResource(R.string.source_not_signed_in)
                            else -> null
                        }
                        if (signedIn != null) JfText(signedIn, style = JfType.caption, color = Jf.colors.ink70, maxLines = 1)
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
            if (YourSources.canBrowse(plugin)) {
                item(key = "browse") {
                    Panel {
                        SettingRow(stringResource(R.string.source_browse), onClick = { nav.push(Screen.SourceBrowse(plugin.id, null, plugin.name)) })
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
            item(key = "uninstall") {
                Row(Modifier.padding(16.dp)) {
                    JfButton(uninstallAsk, {
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
 * A level of a source's catalogue (`Command.Browse`, page by page; [BrowseModel]): its root, or an
 * album, artist, playlist or folder. An album or playlist gets the full page, as Jellyfesh Web and
 * the library's album page show it: the artwork, what it is, the title, the artist in accent text,
 * the song count, Play, Shuffle and More (Play Next, Play Last, Open in {Service}), the songs
 * (numbered on an album) and "12 songs, 48 minutes"; when none of its songs plays here, Open in
 * {Service} is its main action. Other levels show their title and rows, with the More menu (Play,
 * Shuffle, Play Next, Play Last, Open in {Service}) in the top bar. A source that asks for
 * attribution says "Content from {Service}" under the title. A page the source did not answer
 * says so with its error; an empty one says "Nothing Here".
 */
@Composable
fun SourceBrowseScreen(screen: Screen.SourceBrowse, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val context = LocalContext.current
    val pluginId = screen.pluginId
    val node = screen.node
    var browse by remember(pluginId, node) { mutableStateOf(BrowseState()) }
    LaunchedEffect(pluginId, node) {
        // Listening first, so the answer to the first page cannot slip past.
        launch(start = CoroutineStart.UNDISPATCHED) {
            graph.store.signals.collect { event ->
                browse = when (event) {
                    is CoreEvent.BrowsePage -> BrowseModel.onPage(browse, event.update)
                    is CoreEvent.CommandFailed -> BrowseModel.onFailed(browse, event.id, describeFailure(context, event.error))
                    else -> browse
                }
            }
        }
        if (browse.items.isEmpty() && !browse.loading) browse = BrowseModel.loadMore(browse, pluginId, node, graph::send)
    }
    val collection = BrowseModel.isCollection(screen.kind) && node != null
    val container = node?.let { CollectionRef.Source(pluginId, it) }
    // An album's or artist's page (Go to Album, Go to Artist): its picture and notes or biography
    // from the source's details (ADR-0022).
    val kind = screen.kind
    val describes = state.sources.plugins.firstOrNull { it.id == pluginId }?.details == true
    val detailsItem = if (node != null && describes && (kind == ItemKind.Album || kind == ItemKind.Artist)) ItemRef.Source(pluginId, kind, node) else null
    val details = rememberDetails(detailsItem)
    val picture = ArtKey.of(screen.artwork) ?: ArtKey.of(details.page?.image)
    val infoAction = detailsItem?.let { actions.getInfoAction(it, KnownInfo(screen.title, screen.subtitle, source = pluginId)) }
    val link = Links.openable(screen.link)
    val credit = Links.attributedService(state.sources, pluginId)?.let { stringResource(R.string.source_attribution, it) }
    val numbers = remember(browse.items, screen.kind) { BrowseModel.numbers(browse, screen.kind) }
    val listState = rememberLazyListState()
    val headerGone by remember { derivedStateOf { listState.firstVisibleItemIndex > 1 } }
    val playing = state.nowPlaying?.track
    val moreLabel = stringResource(R.string.action_more)
    Box(Modifier.fillMaxSize()) {
        LazyColumn(state = listState, modifier = Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            if (collection && container != null) {
                item(key = "header") {
                    val songs = browse.tracks.size.takeIf { browse.complete && it > 0 }
                    AlbumHeader(
                        art = picture,
                        title = screen.title,
                        artist = screen.subtitle?.takeIf { it.isNotBlank() },
                        details = songs?.let { pluralStringResource(R.plurals.count_songs, it, it) },
                        onArtist = null,
                        kind = stringResource(if (screen.kind == ItemKind.Album) R.string.kind_album else R.string.kind_playlist),
                        note = credit,
                    )
                }
                item(key = "actions") {
                    when {
                        browse.playable -> PlayButtons(
                            onPlay = { graph.send(Command.PlayCollection(container, 0, false)) },
                            onShuffle = { graph.send(Command.PlayCollection(container, 0, true)) },
                        ) {
                            JfIconButton(R.drawable.ic_more, moreLabel, {
                                actions.showForCollection(screen.title, screen.subtitle, container, withPlay = false, link = screen.link, extra = listOfNotNull(infoAction))
                            })
                        }
                        // None of its songs plays here: its service is where they play.
                        BrowseModel.opensLink(browse, screen.link) && link != null -> Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                            JfButton(actions.openInLabel(pluginId), { actions.openLink(link) }, style = ButtonStyle.Accent, icon = R.drawable.ic_external)
                        }
                        link != null -> Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
                            JfIconButton(R.drawable.ic_more, moreLabel, {
                                actions.openLinkAction(screen.link, pluginId)?.let { open -> actions.showSheetActions(screen.title, screen.subtitle, listOf(open)) }
                            })
                        }
                    }
                }
            } else {
                // An artist's picture above the name.
                if (kind == ItemKind.Artist && picture != null) {
                    item(key = "picture") {
                        Box(Modifier.fillMaxWidth().padding(top = 8.dp), contentAlignment = Alignment.Center) {
                            org.jellyfesh.android.ui.components.ArtworkImage(picture, screen.title, 160.dp, androidx.compose.foundation.shape.CircleShape)
                        }
                    }
                }
                // "Content from {Service}" for a source whose terms ask for the credit.
                item(key = "title") { ScreenTitle(screen.title, subtitle = credit) }
            }
            details.page?.description?.takeIf { it.isNotBlank() }?.let { text ->
                item(key = "blurb") { Blurb(text, Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) }
            }
            itemsIndexed(browse.items, key = { i, it -> "$i-${it.id}" }) { i, item ->
                val action = SourceItems.mainAction(item, browse.items)
                SourceItemRow(item, playing, action, onMore = sourceItemMenu(item, actions), number = numbers.getOrNull(i)) {
                    runItemAction(action, graph::send, nav::push, actions::openLink)
                }
            }
            val error = browse.error
            when {
                browse.unavailable -> item(key = "state") { Notice(stringResource(R.string.not_available_yet)) }
                error != null -> item(key = "state") { Notice(error, warning = true, title = stringResource(R.string.browse_failed_title)) }
                // Asking, or about to ask, for the first page.
                browse.loading || (browse.items.isEmpty() && browse.nextPage == 0) -> item(key = "state") { LoadingRow() }
                browse.nextPage != null && browse.items.isNotEmpty() -> item(key = "state") {
                    Row(Modifier.padding(16.dp)) { JfButton(stringResource(R.string.action_load_more), { browse = BrowseModel.loadMore(browse, pluginId, node, graph::send) }) }
                }
                browse.items.isEmpty() -> item(key = "state") { EmptyState(R.drawable.ic_folder, stringResource(R.string.browse_empty), null) }
            }
            // "12 songs, 48 minutes" once every page has arrived.
            if (collection && browse.complete && error == null && browse.tracks.isNotEmpty()) {
                item(key = "footer") { SongsFooter(browse.tracks.size, browse.totalMs) }
            }
        }
        BackBar(onBack = nav::back, title = screen.title.takeIf { collection && headerGone }, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
            if (!collection && container != null) {
                JfIconButton(R.drawable.ic_more, moreLabel, {
                    actions.showForCollection(screen.title, null, container, link = screen.link, extra = listOfNotNull(infoAction))
                }, tint = Jf.colors.ink90)
            }
        }
    }
}
