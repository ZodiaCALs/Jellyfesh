package org.jellyfesh.android.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
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
import org.jellyfesh.android.ui.actions.TrackTarget
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.AlbumCard
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
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens

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

    // Typing: debounce, then the library's fast query (again whenever the library changes) and,
    // once per query, the sources through the core's merged search.
    val anySource = state.sources.plugins.any { it.enabled }
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
                is CoreEvent.CommandFailed -> SearchModel.onCommandFailed(search, event.id)
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
        val library = search.library
        if (search.libraryQuery == search.query && !search.hasResults && search.done) {
            item(key = "none") { EmptyState(R.drawable.ic_search, stringResource(R.string.search_no_results), stringResource(R.string.search_no_results_body)) }
        }
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
                    onClick = { graph.send(Command.PlayCollection(CollectionRef.Tracks(refs), index, false)) },
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
        // Then From Your Sources, one group per source, as each answers (docs/ui.md, "Search").
        val sourcesUnavailable = search.sourcesUnavailable && state.sources.plugins.any { it.enabled }
        if (sourcesUnavailable || search.sources.isNotEmpty()) {
            item(key = "src-h") { SectionHeader(stringResource(R.string.search_from_sources)) }
        }
        if (sourcesUnavailable) {
            item(key = "src-unavailable") { Notice(stringResource(R.string.search_sources_unavailable)) }
        }
        search.sources.forEach { section ->
            val plugin = state.sources.plugins.firstOrNull { it.id == section.source }
            val name = plugin?.name ?: section.source
            item(key = "src-${section.source}-h") {
                JfText(name, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize), modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp).semantics { heading() })
            }
            section.error?.let { error ->
                item(key = "src-${section.source}-e") { Notice(stringResource(R.string.search_source_failed, name, error), warning = true) }
            }
            SearchModel.byKind(section.items).forEach { (kind, items) ->
                item(key = "src-${section.source}-$kind") {
                    JfText(stringResource(kindLabel(kind)), style = JfType.label, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                }
                val playable = items.mapNotNull { it.track }
                itemsIndexed(items, key = { i, it -> "src-${section.source}-$kind-$i-${it.id}" }) { _, item ->
                    SourceItemRow(item, playing) {
                        openSourceItem(item, playable, graph::send) { nav.push(it) }
                    }
                }
            }
        }
        if (!search.done) {
            item(key = "searching") {
                JfText(stringResource(R.string.search_searching), style = JfType.small, color = Jf.colors.ink50, modifier = Modifier.padding(16.dp))
            }
        }
    }
}

/** Plays a source track (with the other playable rows of its list), or opens a container. */
fun openSourceItem(item: SourceItem, playable: List<TrackRef>, send: (Command) -> Long, push: (Screen) -> Unit) {
    val track = item.track
    when {
        track != null && item.playable -> send(Command.PlayTracks(playable, playable.indexOf(track).coerceAtLeast(0)))
        item.kind == ItemKind.Album || item.kind == ItemKind.Playlist || item.kind == ItemKind.Folder || item.kind == ItemKind.Artist ->
            push(Screen.SourceBrowse(item.source, item.id, item.title))
    }
}

fun kindLabel(kind: ItemKind): Int = when (kind) {
    ItemKind.Track -> R.string.kind_tracks
    ItemKind.Album -> R.string.kind_albums
    ItemKind.Artist -> R.string.kind_artists
    ItemKind.Playlist -> R.string.kind_playlists
    ItemKind.Folder -> R.string.kind_folders
}

/** A result from an online source: playable rows play, containers open; unplayable rows say why. */
@Composable
fun SourceItemRow(item: SourceItem, playing: TrackRef?, onClick: () -> Unit) {
    val subtitle = when {
        !item.playable && item.unplayableReason != null -> item.unplayableReason
        item.alsoIn.isNotEmpty() -> listOfNotNull(item.subtitle, stringResource(R.string.search_also_in, item.alsoIn.joinToString())).joinToString(" · ")
        else -> Labels.artistAlbum(item.subtitle, item.album)
    }
    if (item.kind == ItemKind.Track) {
        TrackRow(
            title = item.title,
            subtitle = subtitle,
            art = ArtKey.of(item.artwork),
            durationMs = item.durationMs,
            playing = item.track != null && item.track == playing,
            missing = !item.playable,
            onClick = onClick,
        )
    } else {
        NavRow(item.title, subtitle, onClick = onClick, leading = { InitialTile(item.title, icon = kindIcon(item.kind)) })
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
    val shape = RoundedCornerShape(Tokens.Radius.rLg.dp)
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
