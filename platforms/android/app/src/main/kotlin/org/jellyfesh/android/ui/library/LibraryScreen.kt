package org.jellyfesh.android.ui.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.LocalChromePadding
import org.jellyfesh.android.ui.actions.TrackTarget
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.AlbumCard
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.ChoiceRow
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.InitialTile
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.NavRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.components.TrackRow
import org.jellyfesh.android.ui.components.bleed
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.PagedSource
import org.jellyfesh.android.ui.data.rememberPagedGridSource
import org.jellyfesh.android.ui.data.rememberPagedSource
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.ChipButton
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.android.ui.theme.statusBarFade
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackSort
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.images.ArtworkSizes
import org.jellyfesh.logic.library.SortMenus
import org.jellyfesh.logic.library.SortOrder
import org.jellyfesh.logic.nav.LibrarySection
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens

/** The Library tab: Albums, Artists, Songs, Playlists and Favourites under one segmented control. */
@Composable
fun LibraryScreen(section: LibrarySection, state: CoreState) {
    val nav = LocalNav.current
    val header: @Composable () -> Unit = {
        Column(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
            ScreenTitle(stringResource(R.string.tab_library)) {
                JfIconButton(R.drawable.ic_settings, stringResource(R.string.action_settings), { nav.openSettings() }, tint = Jf.colors.ink90)
            }
            SegmentedControl(
                options = LibrarySection.entries,
                selected = section,
                label = { stringResource(sectionLabel(it)) },
                onSelect = { nav.replaceTop(Screen.Library(it)) },
                description = stringResource(R.string.a11y_library_sections),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
    when (section) {
        LibrarySection.Albums -> AlbumsGrid(state, header)
        LibrarySection.Artists -> ArtistsList(state, header)
        LibrarySection.Songs -> SongsList(state, header)
        LibrarySection.Playlists -> PlaylistsList(state, header)
        LibrarySection.Favorites -> FavouritesList(state, header)
    }
}

fun sectionLabel(section: LibrarySection): Int = when (section) {
    LibrarySection.Albums -> R.string.section_albums
    LibrarySection.Artists -> R.string.section_artists
    LibrarySection.Songs -> R.string.section_songs
    LibrarySection.Playlists -> R.string.section_playlists
    LibrarySection.Favorites -> R.string.section_favourites
}

/** Shared bottom space: the floating chrome plus a margin. */
@Composable
fun listPadding(): PaddingValues = PaddingValues(bottom = LocalChromePadding.current.calculateBottomPadding() + 16.dp)

@Composable
private fun AlbumsGrid(state: CoreState, header: @Composable () -> Unit) {
    val nav = LocalNav.current
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    var sortField by rememberSaveable { mutableStateOf(SortMenus.DEFAULT_ALBUMS.field) }
    var descending by rememberSaveable { mutableStateOf(SortMenus.DEFAULT_ALBUMS.descending) }
    val order = SortOrder(sortField, descending)
    val gridState = rememberLazyGridState()
    val albums = rememberPagedGridSource(gridState, state.libraryRevision, order, rowOffset = 1) { core, offset, limit ->
        core.albumsSorted(order.field, order.descending, offset, limit)
    }
    val sortTitle = stringResource(R.string.sort_albums)
    val cellPx = with(LocalDensity.current) { 180.dp.roundToPx() }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp),
        state = gridState,
        modifier = Modifier.fillMaxSize().statusBarFade(),
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = listPadding().calculateBottomPadding()),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item(key = "header", span = { GridItemSpan(maxLineSpan) }) {
            Column(Modifier.bleed(12.dp)) {
                header()
                SortChip(sortTitle, stringResource(albumSortLabel(order.field)), order.descending) {
                    showSortMenu(
                        overlays,
                        sortTitle,
                        SortMenus.ALBUM_FIELDS,
                        { stringResource(albumSortLabel(it)) },
                        { SortOrder(sortField, descending) },
                        SortMenus::album,
                    ) { chosen ->
                        sortField = chosen.field
                        descending = chosen.descending
                    }
                }
            }
        }
        pagedStates(albums, emptyTitle = R.string.albums_empty_title, emptyBody = R.string.albums_empty_body)
        items(albums.count, key = { i -> albums[i]?.id ?: "pending-$i" }) { i ->
            val album = albums[i]
            if (album != null) {
                AlbumCard(
                    title = album.title,
                    subtitle = album.albumArtist ?: stringResource(R.string.unknown_artist),
                    art = ArtKey.ofHash(album.artworkHash),
                    sizePx = ArtworkSizes.bucket(cellPx),
                    onClick = { nav.openAlbum(album.id, album.title) },
                    onPlay = { graph.send(Command.PlayCollection(CollectionRef.Album(album.id), 0, false)) },
                )
            }
        }
    }
}

fun albumSortLabel(sort: AlbumSort): Int = when (sort) {
    AlbumSort.Title -> R.string.sort_title
    AlbumSort.Artist -> R.string.sort_artist
    AlbumSort.Year -> R.string.sort_year
    AlbumSort.Added -> R.string.sort_added
}

fun songSortLabel(sort: TrackSort): Int = when (sort) {
    TrackSort.Title -> R.string.sort_title
    TrackSort.Artist -> R.string.sort_artist
    TrackSort.Album -> R.string.sort_album
    TrackSort.Year -> R.string.sort_year
    TrackSort.Added -> R.string.sort_added
    // Not in the Songs menu (SortMenus.SONG_FIELDS).
    TrackSort.Duration -> R.string.info_length
    TrackSort.Path -> R.string.info_file
}

/** The sort menu's button: the order in use; it opens the menu. */
@Composable
private fun SortChip(title: String, field: String, descending: Boolean, onClick: () -> Unit) {
    val spoken = listOfNotNull(title, field, stringResource(R.string.sort_descending).takeIf { descending }).joinToString(", ")
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.End) {
        ChipButton(R.drawable.ic_sort, field, onClick, description = spoken)
    }
}

/**
 * A sort menu as a sheet (docs/ui.md): the fields, each starting in its own direction
 * ([SortMenus]), then Descending. [current] is read while the sheet shows, so the switch follows.
 */
private fun <F> showSortMenu(
    overlays: Overlays,
    title: String,
    fields: List<F>,
    label: @Composable (F) -> String,
    current: () -> SortOrder<F>,
    start: (F) -> SortOrder<F>,
    onChange: (SortOrder<F>) -> Unit,
) {
    overlays.showSheet(title) { dismiss ->
        val order = current()
        Column(Modifier.selectableGroup().padding(top = 4.dp)) {
            fields.forEach { field ->
                ChoiceRow(label(field), field == order.field, {
                    onChange(start(field))
                    dismiss()
                })
            }
        }
        Divider()
        SettingRow(stringResource(R.string.sort_descending), Modifier.padding(horizontal = 4.dp)) {
            JfSwitch(order.descending, { onChange(SortMenus.descending(order, it)) })
        }
    }
}

/** Empty, loading and failure rows of a paged grid. */
private fun androidx.compose.foundation.lazy.grid.LazyGridScope.pagedStates(source: PagedSource<*>, emptyTitle: Int, emptyBody: Int) {
    val failure = source.failure
    when {
        failure is QueryResult.NotAvailable -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { Notice(stringResource(R.string.not_available_yet)) }
        failure is QueryResult.Failed -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { Notice(stringResource(R.string.query_failed, failure.message), warning = true) }
        source.isLoading -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { LoadingRow() }
        source.complete && source.count == 0 -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) {
            EmptyState(R.drawable.ic_album, stringResource(emptyTitle), stringResource(emptyBody)) { LibraryAccessButtons() }
        }
    }
}

/** Empty, loading and failure rows of a paged list. */
fun LazyListScope.pagedStates(source: PagedSource<*>, icon: Int, emptyTitle: Int, emptyBody: Int?, emptyAction: (@Composable () -> Unit)? = null) {
    val failure = source.failure
    when {
        failure is QueryResult.NotAvailable -> item(key = "state") { Notice(stringResource(R.string.not_available_yet)) }
        failure is QueryResult.Failed -> item(key = "state") { Notice(stringResource(R.string.query_failed, failure.message), warning = true) }
        source.isLoading -> item(key = "state") { LoadingRow() }
        source.complete && source.count == 0 -> item(key = "state") {
            EmptyState(icon, stringResource(emptyTitle), emptyBody?.let { stringResource(it) }, action = emptyAction)
        }
    }
}

@Composable
private fun ArtistsList(state: CoreState, header: @Composable () -> Unit) {
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val listState = rememberLazyListState()
    val artists = rememberPagedSource(listState, state.libraryRevision, rowOffset = 1) { core, offset, limit -> core.artists(offset, limit) }
    LazyColumn(state = listState, modifier = Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "header") { header() }
        pagedStates(artists, R.drawable.ic_artist, R.string.artists_empty_title, R.string.artists_empty_body) { LibraryAccessButtons() }
        items(artists.count, key = { i -> artists[i]?.id ?: "pending-$i" }) { i ->
            val artist = artists[i]
            if (artist != null) {
                NavRow(
                    title = artist.name,
                    subtitle = pluralStringResource(R.plurals.count_songs, artist.trackCount, artist.trackCount),
                    onClick = { nav.openArtist(artist.id, artist.name) },
                    leading = { InitialTile(artist.name) },
                    onMore = { actions.showForCollection(artist.name, null, CollectionRef.Artist(artist.id)) },
                )
            }
        }
    }
}

@Composable
private fun SongsList(state: CoreState, header: @Composable () -> Unit) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val actions = rememberTrackActions()
    val listState = rememberLazyListState()
    var sortField by rememberSaveable { mutableStateOf(SortMenus.DEFAULT_SONGS.field) }
    var descending by rememberSaveable { mutableStateOf(SortMenus.DEFAULT_SONGS.descending) }
    val order = SortOrder(sortField, descending)
    val songs = rememberPagedSource(listState, state.libraryRevision, order, rowOffset = 2) { core, offset, limit ->
        core.tracks(SortMenus.songsQuery(order, offset, limit))
    }
    val sortTitle = stringResource(R.string.sort_songs)
    val playing = state.nowPlaying?.track
    LazyColumn(state = listState, modifier = Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "header") { header() }
        item(key = "play") {
            Column {
                // Shuffles the songs loaded so far: the core has no "whole library" collection yet.
                PlayButtons(
                    onPlay = { refs(songs).takeIf { it.isNotEmpty() }?.let { graph.send(Command.PlayCollection(CollectionRef.Tracks(it), 0, false)) } },
                    onShuffle = { refs(songs).takeIf { it.isNotEmpty() }?.let { graph.send(Command.PlayCollection(CollectionRef.Tracks(it), 0, true)) } },
                )
                SortChip(sortTitle, stringResource(songSortLabel(order.field)), order.descending) {
                    showSortMenu(
                        overlays,
                        sortTitle,
                        SortMenus.SONG_FIELDS,
                        { stringResource(songSortLabel(it)) },
                        { SortOrder(sortField, descending) },
                        SortMenus::song,
                    ) { chosen ->
                        sortField = chosen.field
                        descending = chosen.descending
                    }
                }
            }
        }
        pagedStates(songs, R.drawable.ic_song, R.string.songs_empty_title, R.string.songs_empty_body) { LibraryAccessButtons() }
        items(songs.count, key = { i -> songs[i]?.id ?: "pending-$i" }) { i ->
            val track = songs[i]
            if (track != null) {
                val ref = TrackRef.Library(track.id)
                val subtitle = Labels.artistAlbum(track.artist, track.album)
                TrackRow(
                    title = track.title,
                    subtitle = subtitle,
                    art = ArtKey.ofHash(track.artworkHash),
                    durationMs = track.durationMs,
                    playing = ref == playing,
                    onClick = { graph.send(Command.PlayCollection(CollectionRef.Tracks(refs(songs)), i, false)) },
                    onMore = { actions.showFor(TrackTarget(ref, track.title, subtitle, track.albumId, track.album, artist = track.artist)) },
                )
            }
        }
    }
}

private fun refs(songs: PagedSource<org.jellyfesh.logic.core.Track>): List<TrackRef> = songs.loaded().map { TrackRef.Library(it.id) }

/** Play and Shuffle, side by side. */
@Composable
fun PlayButtons(onPlay: () -> Unit, onShuffle: () -> Unit, modifier: Modifier = Modifier, extra: (@Composable () -> Unit)? = null) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        JfButton(stringResource(R.string.action_play), onPlay, style = ButtonStyle.Accent, icon = R.drawable.ic_play, modifier = Modifier.weight(1f))
        JfButton(stringResource(R.string.action_shuffle), onShuffle, icon = R.drawable.ic_shuffle, modifier = Modifier.weight(1f))
        extra?.invoke()
    }
}

/**
 * Playlists (docs/ui.md): New Playlist… (asks for the name first) and Import Playlist…, then the
 * playlists; each one's More menu adds Rename…, Export Playlist… and Delete Playlist….
 */
@Composable
private fun PlaylistsList(state: CoreState, header: @Composable () -> Unit) {
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val transfer = rememberPlaylistTransfer()
    val playlistActions = rememberPlaylistActions(transfer)
    val playlists by rememberQuery(state.libraryRevision) { it.playlists() }
    val newLabel = stringResource(R.string.action_new_playlist)
    LazyColumn(modifier = Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "header") { header() }
        item(key = "new") {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                JfButton(newLabel, playlistActions::create, icon = R.drawable.ic_add)
                JfButton(stringResource(R.string.action_import_playlist), transfer.import, style = ButtonStyle.Quiet, icon = R.drawable.ic_import)
            }
        }
        when (val p = playlists) {
            QueryResult.Loading -> item(key = "state") { LoadingRow() }
            is QueryResult.NotAvailable -> item(key = "state") { Notice(stringResource(R.string.not_available_yet)) }
            is QueryResult.Failed -> item(key = "state") { Notice(stringResource(R.string.query_failed, p.message), warning = true) }
            is QueryResult.Ok -> {
                if (p.value.isEmpty()) {
                    item(key = "state") {
                        EmptyState(R.drawable.ic_playlist, stringResource(R.string.playlists_empty_title), stringResource(R.string.playlists_empty_body)) {
                            JfButton(newLabel, playlistActions::create, style = ButtonStyle.Accent, icon = R.drawable.ic_add)
                        }
                    }
                }
                items(p.value.size, key = { p.value[it].id }) { i ->
                    val playlist = p.value[i]
                    val subtitle = pluralStringResource(R.plurals.count_songs, playlist.entryCount, playlist.entryCount)
                    NavRow(
                        title = playlist.name,
                        subtitle = subtitle,
                        onClick = { nav.openPlaylist(playlist.id, playlist.name) },
                        leading = {
                            if (playlist.artworkHash != null) {
                                ArtworkImage(ArtKey.ofHash(playlist.artworkHash), playlist.name, 44.dp, RoundedCornerShape(Tokens.Artwork.radiusList.dp))
                            } else {
                                InitialTile(playlist.name, icon = R.drawable.ic_playlist)
                            }
                        },
                        onMore = {
                            actions.showForCollection(
                                playlist.name,
                                subtitle,
                                CollectionRef.Playlist(playlist.id),
                                extra = playlistActions.items(playlist.id, playlist.name),
                            )
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun FavouritesList(state: CoreState, header: @Composable () -> Unit) {
    val graph = LocalGraph.current
    val actions = rememberTrackActions()
    val listState = rememberLazyListState()
    val favourites = rememberPagedSource(listState, state.libraryRevision, rowOffset = 2) { core, offset, limit -> core.favorites(offset, limit) }
    val playing = state.nowPlaying?.track
    LazyColumn(state = listState, modifier = Modifier.fillMaxSize().statusBarFade(), contentPadding = listPadding()) {
        item(key = "header") { header() }
        item(key = "play") {
            PlayButtons(
                onPlay = { graph.send(Command.PlayCollection(CollectionRef.Favorites, 0, false)) },
                onShuffle = { graph.send(Command.PlayCollection(CollectionRef.Favorites, 0, true)) },
            )
        }
        pagedStates(favourites, R.drawable.ic_heart, R.string.favourites_empty_title, R.string.favourites_empty_body)
        items(favourites.count, key = { i -> favourites[i]?.let { "f-$i-${it.track}" } ?: "pending-$i" }) { i ->
            val row = favourites[i]
            if (row != null) {
                val subtitle = Labels.artistAlbum(row.artist, row.album)
                TrackRow(
                    title = row.title,
                    subtitle = subtitle,
                    art = ArtKey.ofHash(row.artworkHash) ?: ArtKey.of(row.artwork),
                    durationMs = row.durationMs,
                    playing = row.track == playing,
                    missing = row.missing,
                    onClick = { graph.send(Command.PlayCollection(CollectionRef.Favorites, i, false)) },
                    onMore = { actions.showFor(TrackTarget(row.track, row.title, subtitle, favorite = true, artist = row.artist)) },
                )
            }
        }
    }
}
