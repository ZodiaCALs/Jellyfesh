package org.jellyfesh.android.ui.home

import androidx.compose.foundation.layout.padding
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.logic.theme.LookPlaces
import org.jellyfesh.logic.core.HomeSection
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.ScreenTitle
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.TrackRow
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.library.LibraryAccessButtons
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.statusBarFade
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.logic.core.AlbumSort
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.core.TrackRow as CoreTrackRow
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.images.ArtworkSizes
import org.jellyfesh.logic.library.HomeModel
import org.jellyfesh.logic.nav.Tab
import org.jellyfesh.logic.state.CoreState

private const val SHELF_ALBUMS = 12
private const val RECENT_ROWS = 10

/**
 * Home (docs/ui.md): the library's size ("1,204 songs · 96 albums · 41 artists"), then Recently
 * Added (albums), Recently Played and Most Played; with nothing in the library and no source
 * signed in, a welcome with Add Music Folder… and Add a Source instead (and Allow Access to Music
 * while Jellyfesh may not read the music on this phone).
 */
@Composable
fun HomeScreen(state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val revision = state.libraryRevision
    val stats by rememberQuery(revision) { it.stats() }
    val added by rememberQuery(revision) { it.albumsSorted(AlbumSort.Added, true, 0, SHELF_ALBUMS) }
    val played by rememberQuery(revision) { it.recentlyPlayed(RECENT_ROWS) }
    val most by rememberQuery(revision) { it.mostPlayed(RECENT_ROWS) }
    // Home's sections in the look's order (ADR-0020); Luminous's while not customized.
    val look = Jf.look
    val sections = look.home
    val summaryUnderTitle = LookPlaces.summaryUnderTitle(look)
    val favourites by rememberQuery(revision, HomeSection.Favourites in sections) {
        if (HomeSection.Favourites in sections) it.favorites(0, RECENT_ROWS) else emptyList()
    }
    val listState = rememberLazyListState()
    val cardPx = with(LocalDensity.current) { 148.dp.roundToPx() }
    val playingUid = state.nowPlaying?.track
    val chrome = LocalChromePadding.current
    val statusInset = WindowInsets.statusBars

    LazyColumn(
        state = listState,
        modifier = Modifier.fillMaxSize().statusBarFade(),
        contentPadding = PaddingValues(bottom = chrome.calculateBottomPadding() + 16.dp),
    ) {
        item(key = "title") {
            ScreenTitle(
                stringResource(R.string.tab_home),
                modifier = Modifier.windowInsetsPadding(statusInset),
                subtitle = if (summaryUnderTitle) librarySummary((stats as? QueryResult.Ok)?.value) else null,
            ) {
                JfIconButton(R.drawable.ic_settings, stringResource(R.string.action_settings), { nav.openSettings() }, tint = Jf.colors.ink90)
            }
        }
        if (HomeModel.showsWelcome((stats as? QueryResult.Ok)?.value, state.scanning, state.sources)) {
            item(key = "welcome") {
                EmptyState(
                    icon = R.drawable.ic_library,
                    title = stringResource(R.string.home_welcome_title),
                    body = stringResource(R.string.home_welcome_body),
                ) { LibraryAccessButtons(onAddSource = { nav.select(Tab.Sources) }) }
            }
        }
        if (stats is QueryResult.Failed) {
            item(key = "failed") { Notice(stringResource(R.string.query_failed, (stats as QueryResult.Failed).message), warning = true) }
        }
        sections.forEach { section ->
            when (section) {
                // The library's size, here when the look moves it from under the title.
                HomeSection.Summary -> if (!summaryUnderTitle) {
                    item(key = "summary") {
                        librarySummary((stats as? QueryResult.Ok)?.value)?.let {
                            JfText(it, style = JfType.small, color = Jf.colors.ink70, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
                        }
                    }
                }
                HomeSection.RecentlyAdded -> {
                    val albums = (added as? QueryResult.Ok)?.value.orEmpty()
                    if (albums.isNotEmpty()) {
                        item(key = "added-h") { SectionHeader(stringResource(R.string.home_recently_added)) }
                        item(key = "added") {
                            LazyRow(contentPadding = PaddingValues(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                items(albums, key = { it.id }) { album ->
                                    AlbumCard(
                                        title = album.title,
                                        subtitle = album.albumArtist,
                                        art = ArtKey.ofHash(album.artworkHash),
                                        sizePx = ArtworkSizes.bucket(cardPx),
                                        onClick = { nav.openAlbum(album.id, album.title) },
                                        modifier = Modifier.width(148.dp),
                                    )
                                }
                            }
                        }
                    }
                }
                HomeSection.RecentlyPlayed -> trackSection("played", played, R.string.home_recently_played, playingUid, graph::send, actions::showFor)
                HomeSection.MostPlayed -> trackSection("most", most, R.string.home_most_played, playingUid, graph::send, actions::showFor)
                HomeSection.Favourites -> trackSection("favourites", favourites, R.string.section_favourites, playingUid, graph::send, actions::showFor)
            }
        }
    }
}

/** "1,204 songs · 96 albums · 41 artists", or null for an empty library. */
@Composable
private fun librarySummary(stats: org.jellyfesh.logic.core.LibraryStats?): String? = stats?.takeIf { it.tracks > 0 }?.let {
    listOf(
        pluralStringResource(R.plurals.count_songs, it.tracks.toInt(), it.tracks.toInt()),
        pluralStringResource(R.plurals.count_albums, it.albums.toInt(), it.albums.toInt()),
        pluralStringResource(R.plurals.count_artists, it.artists.toInt(), it.artists.toInt()),
    ).joinToString(" · ")
}

/**
 * A titled list of rows on one grouped surface (Luminous: the Mist tint, edge and shadow-1; at
 * most [RECENT_ROWS] rows, so they are laid out at once); tapping one plays the list from there.
 */
private fun androidx.compose.foundation.lazy.LazyListScope.trackSection(
    key: String,
    result: QueryResult<List<CoreTrackRow>>,
    title: Int,
    playing: TrackRef?,
    send: (Command) -> Long,
    more: (TrackTarget) -> Unit,
) {
    val rows = (result as? QueryResult.Ok)?.value.orEmpty()
    if (rows.isEmpty()) return
    item(key = "$key-h") { SectionHeader(stringResource(title)) }
    val refs = rows.map { it.track }
    item(key = key) {
        Panel {
            rows.forEachIndexed { index, row ->
                if (index > 0) Divider()
                TrackRow(
                    title = row.title,
                    subtitle = Labels.artistAlbum(row.artist, row.album),
                    art = ArtKey.ofHash(row.artworkHash) ?: ArtKey.of(row.artwork),
                    durationMs = row.durationMs,
                    playing = row.track == playing,
                    missing = row.missing,
                    favorite = row.favorite,
                    onClick = { send(Command.PlayCollection(CollectionRef.Tracks(refs), index, false)) },
                    onMore = { more(TrackTarget(row.track, row.title, Labels.artistAlbum(row.artist, row.album), favorite = row.favorite, artist = row.artist)) },
                )
            }
        }
    }
}
