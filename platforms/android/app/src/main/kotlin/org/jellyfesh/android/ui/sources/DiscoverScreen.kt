package org.jellyfesh.android.ui.sources

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.AlbumCard
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.library.barFade
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.search.SourceItemRow
import org.jellyfesh.android.ui.search.runItemAction
import org.jellyfesh.android.ui.search.sourceItemMenu
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.rounded
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.core.Shelf
import org.jellyfesh.logic.core.ShelfStyle
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.images.ArtworkSizes
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.sources.DiscoverModel
import org.jellyfesh.logic.sources.DiscoverState
import org.jellyfesh.logic.sources.KnownShelf
import org.jellyfesh.logic.sources.Links
import org.jellyfesh.logic.sources.ShelfTitle
import org.jellyfesh.logic.sources.SourceItems
import org.jellyfesh.logic.sources.YourSources
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens

/**
 * A source's Discover page (docs/ui.md, "Discover"; ADR-0022): its name and status, with Browse
 * (its own folders) and its settings in the header, then the core's shelves (`Discover`): rows
 * that scroll sideways, grids that wrap and numbered charts, each with See All when it has more.
 * While loading, placeholders the shape of the shelves; on failure, the error and Try Again.
 */
@Composable
fun DiscoverScreen(pluginId: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val context = LocalContext.current
    val plugin = state.sources.plugins.firstOrNull { it.id == pluginId }
    var discover by remember(pluginId) { mutableStateOf(DiscoverState()) }
    LaunchedEffect(pluginId) {
        // Listening first, so the answer cannot slip past.
        launch(start = CoroutineStart.UNDISPATCHED) {
            graph.store.signals.collect { event ->
                discover = when (event) {
                    is CoreEvent.Discover -> DiscoverModel.onPage(discover, event.page)
                    is CoreEvent.CommandFailed -> DiscoverModel.onFailed(discover, event.id, describeFailure(context, event.error))
                    else -> discover
                }
            }
        }
        if (discover.page == null && !discover.loading) discover = DiscoverModel.load(discover, pluginId, graph::send)
    }
    val credit = Links.attributedService(state.sources, pluginId)?.let { stringResource(R.string.source_attribution, it) }
    val shelves = DiscoverModel.shelves(discover)
    val error = discover.error
    Box(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "title") {
                ScreenTitle(plugin?.name ?: pluginId, subtitle = listOfNotNull(plugin?.let { pluginStatus(it) }, credit).joinToString("\n").ifEmpty { null }) {
                    if (plugin != null) {
                        YourSources.browse(plugin)?.let { browse ->
                            JfButton(stringResource(R.string.source_browse), { nav.push(browse) }, style = ButtonStyle.Tonal, icon = R.drawable.ic_folder)
                        }
                    }
                }
            }
            // Android 17: a source on the home network needs Local Network access.
            item(key = "local-network") { LocalNetworkNotice(listOfNotNull(plugin)) }
            when {
                error != null && shelves.isEmpty() -> item(key = "error") {
                    Notice(
                        error,
                        warning = true,
                        title = stringResource(R.string.browse_failed_title),
                        action = stringResource(R.string.action_retry) to { discover = DiscoverModel.load(discover, pluginId, graph::send) },
                    )
                }
                discover.page == null -> item(key = "placeholders") { ShelfPlaceholders() }
                shelves.isEmpty() -> item(key = "empty") { EmptyState(R.drawable.ic_sources, stringResource(R.string.browse_empty), null) }
            }
            shelves.forEach { shelf ->
                item(key = "h:${shelf.id}") {
                    val title = shelf.titleText(plugin)
                    SectionHeader(title) {
                        shelf.more?.let { node ->
                            JfButton(stringResource(R.string.discover_see_all), { nav.push(Screen.SourceBrowse(pluginId, node, title)) }, style = ButtonStyle.Quiet)
                        }
                    }
                }
                item(key = "s:${shelf.id}") { ShelfContent(shelf, state) }
            }
        }
        BackBar(onBack = nav::back, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
            JfIconButton(R.drawable.ic_settings, stringResource(R.string.action_settings), { nav.push(Screen.Source(pluginId)) }, tint = Jf.colors.ink90)
        }
    }
}

/** A shelf's title: the plugin's words, or the app's for the core's own shelves. */
@Composable
private fun Shelf.titleText(plugin: PluginSummary?): String = when (val t = DiscoverModel.title(this, plugin)) {
    is ShelfTitle.Given -> t.title
    is ShelfTitle.SourceName -> t.name
    is ShelfTitle.Known -> stringResource(
        when (t.shelf) {
            KnownShelf.ForYou -> R.string.discover_for_you
            KnownShelf.RecentlyAdded -> R.string.home_recently_added
            KnownShelf.MostPlayed -> R.string.home_most_played
            KnownShelf.Rediscover -> R.string.discover_rediscover
            KnownShelf.Browse -> R.string.source_browse
        },
    )
}

/** A shelf as its style asks: a row of covers, a grid that wraps, or a numbered chart. */
@Composable
private fun ShelfContent(shelf: Shelf, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val cardPx = with(LocalDensity.current) { 148.dp.roundToPx() }
    fun open(item: SourceItem) = runItemAction(SourceItems.mainAction(item, shelf.items), graph::send, nav::push, actions::openLink)
    when (shelf.style) {
        ShelfStyle.Row -> LazyRow(contentPadding = PaddingValues(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            items(shelf.items, key = { "${it.kind}:${it.id}" }) { item ->
                AlbumCard(
                    title = item.title,
                    subtitle = item.subtitle,
                    art = ArtKey.of(item.artwork),
                    sizePx = ArtworkSizes.bucket(cardPx),
                    onClick = { open(item) },
                    modifier = Modifier.width(148.dp),
                )
            }
        }
        ShelfStyle.Grid -> BoxWithConstraints(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
            val columns = (maxWidth / 160.dp).toInt().coerceIn(2, 6)
            val px = with(LocalDensity.current) { (maxWidth / columns).roundToPx() }
            Column {
                shelf.items.chunked(columns).forEach { row ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        row.forEach { item ->
                            AlbumCard(
                                title = item.title,
                                subtitle = item.subtitle,
                                art = ArtKey.of(item.artwork),
                                sizePx = ArtworkSizes.bucket(px),
                                onClick = { open(item) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                        repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }
        ShelfStyle.Chart -> Column {
            val playing = state.nowPlaying?.track
            shelf.items.forEachIndexed { i, item ->
                val action = SourceItems.mainAction(item, shelf.items)
                SourceItemRow(item, playing, action, onMore = sourceItemMenu(item, actions), number = i + 1) { open(item) }
            }
        }
    }
}

/** While the page loads: shelves' shapes, quiet. */
@Composable
private fun ShelfPlaceholders() {
    val fill = Jf.colors.ink06
    val label = stringResource(R.string.loading)
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { contentDescription = label }, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        repeat(3) {
            Box(Modifier.width(140.dp).height(18.dp).background(fill, rounded(Tokens.Radius.rMd.dp)))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(3) { Box(Modifier.weight(1f).aspectRatio(1f).background(fill, rounded(Tokens.Artwork.radiusGrid.dp))) }
            }
        }
    }
}
