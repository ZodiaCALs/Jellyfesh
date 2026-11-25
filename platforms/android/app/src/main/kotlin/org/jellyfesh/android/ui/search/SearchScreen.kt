package org.jellyfesh.android.ui.search

import org.jellyfesh.logic.library.DetailsModel
import org.jellyfesh.android.ui.player.KnownInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import org.jellyfesh.logic.search.SourceSection
import org.jellyfesh.logic.search.SearchTab
import org.jellyfesh.logic.core.SearchResults
import org.jellyfesh.android.ui.theme.PillTabs
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.actions.TrackActions
import org.jellyfesh.android.ui.actions.TrackTarget
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.AlbumCard
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.InitialTile
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.TrackRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.library.listPadding
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.statusBarFade
import org.jellyfesh.logic.core.CollectionRef
import androidx.compose.ui.platform.LocalContext
import org.jellyfesh.android.ui.signals.describeFailure
import org.jellyfesh.android.ui.sources.PluginIcon
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.logic.search.SearchGroupKey
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.CoreEvent
import org.jellyfesh.logic.core.ItemKind
import org.jellyfesh.logic.core.SearchScope
import org.jellyfesh.logic.core.SourceItem
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.images.ArtworkSizes
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.search.SearchModel
import org.jellyfesh.logic.search.SearchState
import org.jellyfesh.logic.sources.ItemAction
import org.jellyfesh.logic.sources.Links
import org.jellyfesh.logic.sources.SourceItems
import org.jellyfesh.logic.sources.YourSources
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.android.ui.theme.rounded

/**
 * Unified search (SearchModel): the library's fast query as the person types, and every enabled
 * source's results from the core's merged search, each source shown as its answer arrives. The
 * shell never ranks or merges: it shows what the core sends.
 */
@Composable
fun SearchScreen(state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    var query by rememberSaveable { mutableStateOf("") }
    var search by remember { mutableStateOf(SearchState()) }
    val keyboard = LocalSoftwareKeyboardController.current
    val focus = remember { FocusRequester() }
    val context = LocalContext.current

    // Typing: debounce, then the library's fast query (again whenever the library changes) and,
    // once per query, the sources through the core's merged search.
    val anySource = YourSources.anyOn(state.sources.plugins)
    LaunchedEffect(query) {
        search = SearchModel.queryChanged(search, query)
        if (query.isBlank() || !anySource) return@LaunchedEffect
        delay(SearchModel.DEBOUNCE_MS)
        val id = graph.send(Command.Search(query, SearchScope.All))
        search = SearchModel.requestSent(search, query, id)
    }
    LaunchedEffect(query, state.libraryRevision) {
        if (query.isBlank()) return@LaunchedEffect
        delay(SearchModel.DEBOUNCE_MS)
        val q = query
        val results = graph.query { it.searchLibrary(q, SearchModel.LIBRARY_LIMIT) }
        if (results is QueryResult.Ok) search = SearchModel.libraryResults(search, q, results.value)
    }
    LaunchedEffect(Unit) {
        graph.store.signals.collect { event ->
            search = when (event) {
                is CoreEvent.SearchResults -> SearchModel.onUpdate(search, event.update)
                is CoreEvent.SearchPage -> SearchModel.onPage(search, event.page)
                is CoreEvent.CommandFailed -> SearchModel.onCommandFailed(search, event.id, describeFailure(context, event.error))
                else -> search
            }
        }
    }

    val playing = state.nowPlaying?.track
    val cardPx = with(LocalDensity.current) { 148.dp.roundToPx() }
    LazyColumn(Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "title") { ScreenTitle(stringResource(R.string.tab_search), Modifier.windowInsetsPadding(WindowInsets.statusBars)) }
        item(key = "field") {
            SearchField(
                query = query,
                onQuery = { query = it },
                onSubmit = { keyboard?.hide() },
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).focusRequester(focus),
            )
        }
        if (search.isBlank) {
            item(key = "hint") {
                EmptyState(R.drawable.ic_search, stringResource(R.string.search_empty_title), stringResource(R.string.search_empty_body))
            }
            return@LazyColumn
        }
        // The tabs: All, Library, then each source that answered (docs/ui.md, "Search").
        val tab = SearchModel.shownTab(search)
        item(key = "tabs") {
            PillTabs(
                options = SearchModel.tabs(search),
                selected = tab,
                label = { searchTabLabel(it, state) },
                onSelect = { search = SearchModel.selectTab(search, it) },
                description = stringResource(R.string.tab_search),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }
        val library = search.library
        val sourcesUnavailable = search.sourcesUnavailable && YourSources.anyOn(state.sources.plugins)
        when (tab) {
            SearchTab.All -> {
                if (search.libraryQuery == search.query && !search.hasResults && search.done) {
                    item(key = "none") { EmptyState(R.drawable.ic_search, stringResource(R.string.search_no_results), stringResource(R.string.search_no_results_body)) }
                }
                libraryResults(library, playing, cardPx, graph::send, nav, actions)
                // Then From Your Sources: one group per source that allows merging, as each answers,
                // then a link to the tab of each source whose terms keep its results apart.
                val merged = SearchModel.mergedSections(search)
                val separate = SearchModel.separateSections(search)
                if (sourcesUnavailable || merged.isNotEmpty() || separate.isNotEmpty()) {
                    item(key = "src-h") { SectionHeader(stringResource(R.string.search_from_sources)) }
                }
                if (sourcesUnavailable) {
                    item(key = "src-unavailable") { Notice(stringResource(R.string.search_sources_unavailable)) }
                }
                merged.forEach { section -> sourceGroup(section, state, playing, graph::send, nav, actions, withName = true) }
                separate.forEach { section ->
                    item(key = "src-${section.source}-apart") {
                        val plugin = state.sources.plugins.firstOrNull { it.id == section.source }
                        val name = plugin?.name ?: section.source
                        NavRow(
                            stringResource(R.string.search_results_from, name),
                            Links.attributedService(state.sources, section.source)?.let { stringResource(R.string.source_attribution, it) },
                            onClick = { search = SearchModel.selectTab(search, SearchTab.Source(section.source)) },
                            leading = { PluginIcon(name, plugin?.icon) },
                        )
                    }
                }
            }
            SearchTab.Library -> {
                if (search.libraryQuery == search.query && library.isEmpty) {
                    item(key = "none") { EmptyState(R.drawable.ic_search, stringResource(R.string.search_no_results), stringResource(R.string.search_no_results_body)) }
                }
                libraryResults(library, playing, cardPx, graph::send, nav, actions)
            }
            is SearchTab.Source -> SearchModel.sourceTab(search, tab.source)?.let { section ->
                if (section.tabGroups.isEmpty() && section.error == null) {
                    item(key = "none") { EmptyState(R.drawable.ic_search, stringResource(R.string.search_no_results), null) }
                }
                sourceTabGroups(
                    section,
                    search,
                    state,
                    playing,
                    graph::send,
                    nav,
                    actions,
                    onShowMore = { kind ->
                        SearchModel.showMore(search, section.source, kind)?.let { command ->
                            search = SearchModel.moreSent(search, section.source, kind, graph.send(command))
                        }
                    },
                )
            }
        }
        if (!search.done && tab !is SearchTab.Library) {
            item(key = "searching") {
                JfText(stringResource(R.string.search_searching), style = JfType.small, color = Jf.colors.ink50, modifier = Modifier.padding(16.dp))
            }
        }
    }
}

/** A tab's name: All, Library, or the source's own name. */
@Composable
private fun searchTabLabel(tab: SearchTab, state: CoreState): String = when (tab) {
    SearchTab.All -> stringResource(R.string.search_tab_all)
    SearchTab.Library -> stringResource(R.string.tab_library)
    is SearchTab.Source -> state.sources.plugins.firstOrNull { it.id == tab.source }?.name ?: tab.source
}

/** The library's results: Songs, Albums and Artists. */
private fun LazyListScope.libraryResults(
    library: SearchResults,
    playing: TrackRef?,
    cardPx: Int,
    send: (Command) -> Long,
    nav: org.jellyfesh.android.ui.nav.NavController,
    actions: TrackActions,
) {
    if (library.tracks.isNotEmpty()) {
        item(key = "lib-tracks-h") { SectionHeader(stringResource(R.string.kind_tracks)) }
        val refs = library.tracks.map { TrackRef.Library(it.id) }
        itemsIndexed(library.tracks, key = { _, t -> "lt-${t.id}" }) { index, track ->
            val ref = TrackRef.Library(track.id)
            val subtitle = Labels.artistAlbum(track.artist, track.album)
            TrackRow(
                title = track.title,
                subtitle = subtitle,
                art = ArtKey.ofHash(track.artworkHash),
                durationMs = track.durationMs,
                playing = ref == playing,
                onClick = { send(Command.PlayCollection(CollectionRef.Tracks(refs), index, false)) },
                onMore = { actions.showFor(TrackTarget(ref, track.title, subtitle, track.albumId, track.album, artist = track.artist)) },
            )
        }
    }
    if (library.albums.isNotEmpty()) {
        item(key = "lib-albums-h") { SectionHeader(stringResource(R.string.kind_albums)) }
        item(key = "lib-albums") {
            LazyRow(contentPadding = PaddingValues(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                items(library.albums, key = { it.id }) { album ->
                    AlbumCard(
                        album.title,
                        album.albumArtist,
                        ArtKey.ofHash(album.artworkHash),
                        ArtworkSizes.bucket(cardPx),
                        onClick = { nav.openAlbum(album.id, album.title) },
                        modifier = Modifier.width(148.dp),
                    )
                }
            }
        }
    }
    if (library.artists.isNotEmpty()) {
        item(key = "lib-artists-h") { SectionHeader(stringResource(R.string.kind_artists)) }
        items(library.artists, key = { "la-${it.id}" }) { artist ->
            NavRow(
                artist.name,
                pluralStringResource(R.plurals.count_songs, artist.trackCount, artist.trackCount),
                onClick = { nav.openArtist(artist.id, artist.name) },
                leading = { InitialTile(artist.name) },
            )
        }
    }
}

/**
 * One source's results: its name ([withName], on the All tab) with "Content from {Service}" when
 * its terms ask for the credit, its error, then its results by kind.
 */
private fun LazyListScope.sourceGroup(
    section: SourceSection,
    state: CoreState,
    playing: TrackRef?,
    send: (Command) -> Long,
    nav: org.jellyfesh.android.ui.nav.NavController,
    actions: TrackActions,
    withName: Boolean,
) {
    val plugin = state.sources.plugins.firstOrNull { it.id == section.source }
    val name = plugin?.name ?: section.source
    val credit = Links.attributedService(state.sources, section.source)
    if (withName || credit != null) {
        item(key = "src-${section.source}-h") {
            Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp)) {
                if (withName) JfText(name, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize), modifier = Modifier.semantics { heading() })
                credit?.let { JfText(stringResource(R.string.source_attribution, it), style = JfType.listSecondary, color = Jf.colors.ink70) }
            }
        }
    }
    section.error?.let { error ->
        item(key = "src-${section.source}-e") { Notice(stringResource(R.string.search_source_failed, name, error), warning = true) }
    }
    SearchModel.byKind(section.items).forEach { (kind, items) ->
        item(key = "src-${section.source}-$kind") {
            JfText(stringResource(kindLabel(kind)), style = JfType.label, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
        itemsIndexed(items, key = { i, it -> "src-${section.source}-$kind-$i-${it.id}" }) { _, item ->
            val action = SourceItems.mainAction(item, items)
            SourceItemRow(item, playing, action, onMore = sourceItemMenu(item, actions)) {
                runItemAction(action, send, nav::push, actions::openLink)
            }
        }
    }
}

/**
 * A source's tab (ADR-0024 §4): "Content from {Service}" when its terms ask for the credit, its
 * error, then its Songs, Albums, Artists and Playlists, each with Show More while the core has a
 * cursor for it; Show More's failure shows under its group.
 */
private fun LazyListScope.sourceTabGroups(
    section: SourceSection,
    search: SearchState,
    state: CoreState,
    playing: TrackRef?,
    send: (Command) -> Long,
    nav: org.jellyfesh.android.ui.nav.NavController,
    actions: TrackActions,
    onShowMore: (ItemKind) -> Unit,
) {
    val name = state.sources.plugins.firstOrNull { it.id == section.source }?.name ?: section.source
    Links.attributedService(state.sources, section.source)?.let { credit ->
        item(key = "tab-${section.source}-credit") {
            JfText(stringResource(R.string.source_attribution, credit), style = JfType.listSecondary, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
        }
    }
    section.error?.let { error ->
        item(key = "tab-${section.source}-e") { Notice(stringResource(R.string.search_source_failed, name, error), warning = true) }
    }
    section.tabGroups.forEach { group ->
        val kind = group.kind
        item(key = "tab-${section.source}-$kind") { SectionHeader(stringResource(kindLabel(kind))) }
        itemsIndexed(group.items, key = { i, it -> "tab-${section.source}-$kind-$i-${it.id}" }) { _, item ->
            val action = SourceItems.mainAction(item, group.items)
            SourceItemRow(item, playing, action, onMore = sourceItemMenu(item, actions)) {
                runItemAction(action, send, nav::push, actions::openLink)
            }
        }
        search.moreErrors[SearchGroupKey(section.source, kind)]?.let { error ->
            item(key = "tab-${section.source}-$kind-error") { Notice(error, warning = true) }
        }
        if (group.next != null) {
            item(key = "tab-${section.source}-$kind-more") {
                Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    JfButton(
                        stringResource(R.string.search_show_more),
                        { onShowMore(kind) },
                        style = ButtonStyle.Quiet,
                        enabled = !SearchModel.loadingMore(search, section.source, kind),
                    )
                }
            }
        }
    }
}

/**
 * Does what choosing a source row does ([SourceItems.mainAction]): plays a song with the other
 * playable songs of its list, opens a container, or opens the link of an item that cannot play
 * here (Open in {Service}).
 */
fun runItemAction(action: ItemAction, send: (Command) -> Long, push: (Screen) -> Unit, openLink: (String) -> Unit) {
    when (action) {
        is ItemAction.Play -> send(Command.PlayTracks(action.tracks, action.start))
        is ItemAction.Browse -> push(Screen.SourceBrowse(action.plugin, action.node, action.title, action.link, action.kind, action.subtitle, action.artwork))
        is ItemAction.OpenLink -> openLink(action.url)
        ItemAction.None -> Unit
    }
}

/**
 * The More menu of a source row, or null when it would be empty: the song menu for a song that
 * plays here, the More menu of an album, artist or playlist that does, and for an item that
 * cannot play here only Open in {Service} (when it has a link).
 */
fun sourceItemMenu(item: SourceItem, actions: TrackActions): (() -> Unit)? {
    val track = item.track
    val subtitle = Labels.artistAlbum(item.subtitle, item.album)
    return when {
        item.playable && track != null -> {
            {
                actions.showFor(
                    TrackTarget(
                        track, item.title, subtitle,
                        albumTitle = item.album,
                        artist = item.subtitle,
                        link = item.link,
                        source = item.source,
                        sourceAlbumId = item.albumId,
                        sourceArtistId = item.artistId,
                        durationMs = item.durationMs,
                    ),
                )
            }
        }
        item.playable && (item.kind == ItemKind.Album || item.kind == ItemKind.Playlist || item.kind == ItemKind.Artist) -> {
            {
                val info = if (item.kind == ItemKind.Playlist) {
                    emptyList()
                } else {
                    listOf(actions.getInfoAction(DetailsModel.itemFor(item), KnownInfo(item.title, item.subtitle, source = item.source)))
                }
                actions.showForCollection(item.title, subtitle, CollectionRef.Source(item.source, item.id, item.kind), link = item.link, extra = info)
            }
        }
        else -> actions.openLinkAction(item.link, item.source)?.let { open -> { actions.showSheetActions(item.title, item.unplayableReason ?: subtitle, listOf(open)) } }
    }
}

fun kindLabel(kind: ItemKind): Int = when (kind) {
    ItemKind.Track -> R.string.kind_tracks
    ItemKind.Album -> R.string.kind_albums
    ItemKind.Artist -> R.string.kind_artists
    ItemKind.Playlist -> R.string.kind_playlists
    ItemKind.Folder -> R.string.kind_folders
}

/**
 * A result from an online source: playable rows play and containers open ([action]); a row that
 * cannot play here says why, dimmed, and opens its link in its service when it has one (with the
 * external glyph at its end). A song with a [number] (on an album's page) shows it in place of
 * its artwork.
 */
@Composable
fun SourceItemRow(item: SourceItem, playing: TrackRef?, action: ItemAction, onMore: (() -> Unit)?, number: Int? = null, onClick: () -> Unit) {
    val subtitle = when {
        !item.playable && item.unplayableReason != null -> item.unplayableReason
        item.alsoIn.isNotEmpty() -> listOfNotNull(item.subtitle, stringResource(R.string.search_also_in, item.alsoIn.joinToString())).joinToString(" · ")
        else -> Labels.artistAlbum(item.subtitle, item.album)
    }
    val opensLink = action is ItemAction.OpenLink
    if (item.kind == ItemKind.Track) {
        val openIn = if (opensLink) rememberTrackActions().openInLabel(item.source) else null
        TrackRow(
            title = item.title,
            subtitle = subtitle,
            art = ArtKey.of(item.artwork),
            showArt = number == null,
            number = number,
            durationMs = item.durationMs,
            playing = item.track != null && item.track == playing,
            dimmed = !item.playable,
            enabled = action != ItemAction.None,
            onClick = onClick,
            onMore = onMore,
            trailing = openIn?.let { label -> { JfIcon(R.drawable.ic_external, Jf.colors.ink50, size = 18.dp, description = label) } },
        )
    } else {
        val art = ArtKey.of(item.artwork)
        NavRow(
            item.title,
            subtitle,
            onClick = onClick,
            leading = {
                // Its cover (an artist's picture round), else its kind's glyph.
                if (art != null) {
                    ArtworkImage(art, item.title, 44.dp, if (item.kind == ItemKind.Artist) CircleShape else rounded(Tokens.Artwork.radiusList.dp))
                } else {
                    InitialTile(item.title, icon = kindIcon(item.kind))
                }
            },
            onMore = onMore,
            trailingIcon = if (opensLink) R.drawable.ic_external else R.drawable.ic_chevron_right,
        )
    }
}

fun kindIcon(kind: ItemKind): Int = when (kind) {
    ItemKind.Track -> R.drawable.ic_song
    ItemKind.Album -> R.drawable.ic_album
    ItemKind.Artist -> R.drawable.ic_artist
    ItemKind.Playlist -> R.drawable.ic_playlist
    ItemKind.Folder -> R.drawable.ic_folder
}

@Composable
private fun SearchField(query: String, onQuery: (String) -> Unit, onSubmit: () -> Unit, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    val shape = rounded(Tokens.Radius.rLg.dp)
    val hint = stringResource(R.string.search_hint)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .background(colors.fill, shape)
            .border(1.dp, colors.line, shape)
            .padding(start = 14.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        JfIcon(R.drawable.ic_search, colors.ink50, size = 20.dp)
        Box(Modifier.weight(1f)) {
            if (query.isEmpty()) JfText(hint, style = JfType.body, color = colors.ink50, maxLines = 1)
            BasicTextField(
                value = query,
                onValueChange = onQuery,
                singleLine = true,
                textStyle = JfType.body.copy(color = colors.ink90),
                cursorBrush = SolidColor(colors.accentText),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
                modifier = Modifier.fillMaxWidth().semantics { contentDescription = hint },
            )
        }
        if (query.isNotEmpty()) {
            JfIconButton(R.drawable.ic_close, stringResource(R.string.a11y_clear_search), { onQuery("") }, size = 40.dp, iconSize = 18.dp)
        }
    }
}
