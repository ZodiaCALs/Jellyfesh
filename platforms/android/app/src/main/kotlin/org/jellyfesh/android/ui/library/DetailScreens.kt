package org.jellyfesh.android.ui.library

import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import kotlinx.coroutines.launch
import org.jellyfesh.android.QueryResult
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.actions.TrackTarget
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.AlbumCard
import org.jellyfesh.android.ui.components.ArtworkFill
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.BackBar
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LoadingRow
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SheetAction
import org.jellyfesh.android.ui.components.TrackRow
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberPagedSource
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.topEdgeFade
import org.jellyfesh.logic.core.CollectionRef
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.images.ArtworkSizes
import org.jellyfesh.logic.library.ArtistMatch
import org.jellyfesh.logic.playlist.PendingMove
import org.jellyfesh.logic.playlist.PlaylistDrag
import org.jellyfesh.logic.playlist.PlaylistReorder
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.android.ui.theme.rounded

/** The large artwork, title and facts at the top of a detail screen. */
@Composable
private fun DetailHeader(art: ArtKey?, title: String, subtitle: String?, facts: String?) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        ArtworkImage(art, title, 220.dp, rounded(Tokens.Artwork.radiusGrid.dp), Modifier.padding(bottom = 10.dp))
        JfText(title, style = JfType.title, textAlign = TextAlign.Center, modifier = Modifier.semantics { heading() })
        if (subtitle != null) JfText(subtitle, style = JfType.nowPlayingArtist, color = Jf.colors.ink70, textAlign = TextAlign.Center)
        if (facts != null) JfText(facts, style = JfType.caption, color = Jf.colors.ink50, textAlign = TextAlign.Center)
    }
}

/** How far the field fades in under a detail screen's back bar and the status bar. */
@Composable
fun barFade(): androidx.compose.ui.unit.Dp = WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + 64.dp

/**
 * An album's header after Apple Music (v0.0.4): large artwork (full width on phones, 240 dp from
 * 600 dp), what it is ([kind], on a source's album or playlist page), the title, the artist in
 * accent text (it opens the artist when [onArtist] is given), then genre, year and the song count,
 * and a [note] under them (a source's "Content from {Service}").
 */
@Composable
fun AlbumHeader(
    art: ArtKey?,
    title: String,
    artist: String?,
    details: String?,
    onArtist: (() -> Unit)?,
    kind: String? = null,
    note: String? = null,
) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val wide = maxWidth >= 600.dp
        val px = with(LocalDensity.current) { (if (wide) 240.dp else maxWidth).roundToPx() }
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp),
            horizontalAlignment = if (wide) Alignment.Start else Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (wide) {
                ArtworkImage(art, title, 240.dp, rounded(Tokens.Artwork.radiusGrid.dp), Modifier.padding(bottom = 12.dp))
            } else {
                ArtworkFill(art, title, rounded(Tokens.Artwork.radiusGrid.dp), px, Modifier.padding(bottom = 12.dp))
            }
            val align = if (wide) TextAlign.Start else TextAlign.Center
            if (kind != null) JfText(kind, style = JfType.caption, color = Jf.colors.ink70, textAlign = align)
            JfText(title, style = JfType.title, textAlign = align, modifier = Modifier.semantics { heading() })
            if (artist != null) {
                JfText(
                    artist,
                    style = JfType.nowPlayingArtist,
                    color = Jf.colors.accentText,
                    textAlign = align,
                    modifier = if (onArtist != null) Modifier.clickable(role = Role.Button, onClick = onArtist) else Modifier,
                )
            }
            if (details != null) JfText(details, style = JfType.caption, color = Jf.colors.ink50, textAlign = align)
            if (note != null) JfText(note, style = JfType.caption, color = Jf.colors.ink70, textAlign = align)
        }
    }
}

/** The line under a list of songs: "12 songs, 48 minutes" (an album's, a source's album or playlist). */
@Composable
fun SongsFooter(count: Int, totalMs: Long) {
    val (h, m) = TimeFormat.hoursMinutes(totalMs)
    val minutes = pluralStringResource(R.plurals.count_minutes, m.toInt(), m.toInt())
    val length = if (h > 0) stringResource(R.string.length_hours_minutes, pluralStringResource(R.plurals.count_hours, h.toInt(), h.toInt()), minutes) else minutes
    JfText(
        stringResource(R.string.album_footer, pluralStringResource(R.plurals.count_songs, count, count), length),
        style = JfType.caption,
        color = Jf.colors.ink50,
        modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp),
    )
}

/** "1 h 12 min" or "48 min" for a total length. */
@Composable
fun totalLength(ms: Long): String {
    val (h, m) = TimeFormat.hoursMinutes(ms)
    return if (h > 0) stringResource(R.string.duration_hours_minutes, h, m) else stringResource(R.string.duration_minutes, m)
}

/** An album: its songs in disc and track order, as the core lists them. */
@Composable
fun AlbumScreen(id: Long, title: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val tracks by rememberQuery(id, state.libraryRevision) { it.albumTracks(id) }
    val listState = rememberLazyListState()
    val headerGone by remember { derivedStateOf { listState.firstVisibleItemIndex > 1 } }
    val playing = state.nowPlaying?.track
    Box(Modifier.fillMaxSize()) {
        LazyColumn(state = listState, modifier = Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            when (val t = tracks) {
                QueryResult.Loading -> item(key = "state") { LoadingRow() }
                is QueryResult.NotAvailable -> item(key = "state") { Notice(stringResource(R.string.not_available_yet)) }
                is QueryResult.Failed -> item(key = "state") { Notice(stringResource(R.string.query_failed, t.message), warning = true) }
                is QueryResult.Ok -> {
                    val list = t.value
                    val first = list.firstOrNull()
                    val artist = first?.albumArtist ?: first?.artist
                    item(key = "header") {
                        val details = listOfNotNull(
                            list.firstNotNullOfOrNull { it.genre?.takeIf(String::isNotBlank) },
                            first?.year?.toString(),
                            pluralStringResource(R.plurals.count_songs, list.size, list.size),
                        ).joinToString(" · ")
                        val scope = rememberCoroutineScope()
                        // The album carries the artist's name only: the core's search finds the artist.
                        val openArtist = artist?.let { name ->
                            {
                                scope.launch {
                                    val found = (graph.query { it.searchLibrary(name, 20) } as? QueryResult.Ok)?.value?.artists.orEmpty()
                                    ArtistMatch.find(name, found)?.let { nav.openArtist(it.id, it.name) }
                                }
                                Unit
                            }
                        }
                        AlbumHeader(ArtKey.ofHash(first?.artworkHash), title, artist, details, openArtist)
                    }
                    item(key = "play") {
                        PlayButtons(
                            onPlay = { graph.send(Command.PlayCollection(CollectionRef.Album(id), 0, false)) },
                            onShuffle = { graph.send(Command.PlayCollection(CollectionRef.Album(id), 0, true)) },
                        ) {
                            JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), {
                                actions.showForCollection(title, artist, CollectionRef.Album(id), withPlay = false)
                            })
                        }
                    }
                    if (list.isEmpty()) {
                        item(key = "empty") { EmptyState(R.drawable.ic_album, stringResource(R.string.album_empty), null) }
                    }
                    val discs = list.mapNotNull { it.discNo }.distinct().size > 1
                    list.forEachIndexed { index, track ->
                        val disc = track.discNo
                        if (discs && disc != null && (index == 0 || list[index - 1].discNo != disc)) {
                            item(key = "disc-$disc") { SectionHeader(stringResource(R.string.album_disc, disc)) }
                        }
                        item(key = track.id) {
                            val ref = TrackRef.Library(track.id)
                            val artistLine = track.artist?.takeIf { it != artist }
                            TrackRow(
                                title = track.title,
                                subtitle = artistLine,
                                showArt = false,
                                number = track.trackNo ?: (index + 1),
                                durationMs = track.durationMs,
                                playing = ref == playing,
                                onClick = { graph.send(Command.PlayCollection(CollectionRef.Album(id), index, false)) },
                                onMore = { actions.showFor(TrackTarget(ref, track.title, Labels.artistAlbum(track.artist, track.album), id, title, artist = track.artist)) },
                            )
                        }
                    }
                    if (list.isNotEmpty()) {
                        // "12 songs, 48 minutes".
                        item(key = "footer") { SongsFooter(list.size, list.sumOf { it.durationMs ?: 0L }) }
                    }
                }
            }
        }
        BackBar(onBack = nav::back, title = title.takeIf { headerGone }, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

/** An artist: their albums, and their songs to play or shuffle. */
@Composable
fun ArtistScreen(id: Long, name: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val albums by rememberQuery(id, state.libraryRevision) { it.artistAlbums(id) }
    val cellPx = with(LocalDensity.current) { 180.dp.roundToPx() }
    Box(Modifier.fillMaxSize()) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(150.dp),
            modifier = Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()),
            contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = listPadding().calculateBottomPadding()),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item(key = "bar-space", span = { GridItemSpan(maxLineSpan) }) { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "header", span = { GridItemSpan(maxLineSpan) }) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    JfText(name, style = JfType.title, textAlign = TextAlign.Center, modifier = Modifier.padding(horizontal = 16.dp).semantics { heading() })
                    PlayButtons(
                        onPlay = { graph.send(Command.PlayCollection(CollectionRef.Artist(id), 0, false)) },
                        onShuffle = { graph.send(Command.PlayCollection(CollectionRef.Artist(id), 0, true)) },
                        modifier = Modifier.widthIn(max = 520.dp),
                    ) {
                        JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), { actions.showForCollection(name, null, CollectionRef.Artist(id), withPlay = false) })
                    }
                    SectionHeader(stringResource(R.string.section_albums), Modifier.padding(horizontal = 0.dp))
                }
            }
            when (val a = albums) {
                QueryResult.Loading -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { LoadingRow() }
                is QueryResult.NotAvailable -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { Notice(stringResource(R.string.not_available_yet)) }
                is QueryResult.Failed -> item(key = "state", span = { GridItemSpan(maxLineSpan) }) { Notice(stringResource(R.string.query_failed, a.message), warning = true) }
                is QueryResult.Ok -> items(a.value.size, key = { a.value[it].id }) { i ->
                    val album = a.value[i]
                    AlbumCard(
                        title = album.title,
                        subtitle = album.year?.toString(),
                        art = ArtKey.ofHash(album.artworkHash),
                        sizePx = ArtworkSizes.bucket(cellPx),
                        onClick = { nav.openAlbum(album.id, album.title) },
                    )
                }
            }
        }
        BackBar(onBack = nav::back, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

/**
 * A playlist: its entries in order, with the playlist's own actions. Rows reorder by dragging
 * their handle (docs/ui.md); the drop shows at once and becomes `MovePlaylistEntry`, and the list
 * read after the core's change confirms it ([PlaylistReorder]). TalkBack gets Move up and Move
 * down on each row instead.
 */
@Composable
fun PlaylistScreen(id: Long, name: String, state: CoreState) {
    val graph = LocalGraph.current
    val nav = LocalNav.current
    val actions = rememberTrackActions()
    val playlistActions = rememberPlaylistActions()
    val summary by rememberQuery(id, state.libraryRevision) { it.playlist(id) }
    val listState = rememberLazyListState()
    val entries = rememberPagedSource(listState, state.libraryRevision, id, rowOffset = 3) { core, offset, limit -> core.playlistEntries(id, offset, limit) }
    val headerGone by remember { derivedStateOf { listState.firstVisibleItemIndex > 1 } }
    val playing = state.nowPlaying?.track
    val current = (summary as? QueryResult.Ok)?.value
    val title = current?.name ?: name
    val removeLabel = stringResource(R.string.action_remove_from_playlist)
    val moreLabel = stringResource(R.string.action_more)
    val moveUp = stringResource(R.string.a11y_move_up)
    val moveDown = stringResource(R.string.a11y_move_down)
    val rowPx = with(LocalDensity.current) { PLAYLIST_ROW.toPx() }
    var drag by remember(id) { mutableStateOf<PlaylistDrag?>(null) }
    var pending by remember(id) { mutableStateOf<PendingMove?>(null) }
    // The local order holds until rows read after the core's change arrive.
    val shownPending = PlaylistReorder.stillPending(pending, entries.rowsRevision)
    Box(Modifier.fillMaxSize()) {
        LazyColumn(state = listState, modifier = Modifier.fillMaxSize().topEdgeFade(Jf.colors.sky, barFade()), contentPadding = listPadding()) {
            item(key = "bar-space") { Box(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) }
            item(key = "header") {
                val facts = current?.let {
                    listOfNotNull(
                        pluralStringResource(R.plurals.count_songs, it.entryCount, it.entryCount),
                        totalLength(it.durationMs).takeIf { _ -> it.durationMs > 0 },
                    ).joinToString(" · ")
                }
                DetailHeader(ArtKey.ofHash(current?.artworkHash), title, null, facts)
            }
            item(key = "play") {
                PlayButtons(
                    onPlay = { graph.send(Command.PlayCollection(CollectionRef.Playlist(id), 0, false)) },
                    onShuffle = { graph.send(Command.PlayCollection(CollectionRef.Playlist(id), 0, true)) },
                ) {
                    JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), {
                        actions.showForCollection(
                            title,
                            null,
                            CollectionRef.Playlist(id),
                            withPlay = false,
                            extra = playlistActions.items(id, title) { nav.back() },
                        )
                    })
                }
            }
            pagedStates(entries, R.drawable.ic_playlist, R.string.playlist_empty, R.string.playlist_empty_body)
            val count = entries.count
            items(count, key = { i -> entries[PlaylistReorder.rowAt(i, shownPending)]?.entryId ?: "pending-$i" }) { i ->
                val entry = entries[PlaylistReorder.rowAt(i, shownPending)]
                if (entry != null) {
                    val row = entry.row
                    val subtitle = Labels.artistAlbum(row.artist, row.album)
                    val dragging = drag?.entryId == entry.entryId
                    TrackRow(
                        title = row.title,
                        subtitle = subtitle,
                        art = ArtKey.ofHash(row.artworkHash) ?: ArtKey.of(row.artwork),
                        durationMs = row.durationMs,
                        playing = row.track == playing,
                        missing = row.missing,
                        favorite = row.favorite,
                        modifier = Modifier
                            .zIndex(if (dragging) 1f else 0f)
                            .graphicsLayer {
                                translationY = PlaylistReorder.shift(i, drag, rowPx, count)
                                if (dragging) shadowElevation = 12f
                            }
                            .semantics {
                                customActions = listOfNotNull(
                                    PlaylistReorder.step(id, entry.entryId, i, -1, count)?.let { command ->
                                        CustomAccessibilityAction(moveUp) { graph.send(command) != 0L }
                                    },
                                    PlaylistReorder.step(id, entry.entryId, i, 1, count)?.let { command ->
                                        CustomAccessibilityAction(moveDown) { graph.send(command) != 0L }
                                    },
                                )
                            },
                        onClick = { graph.send(Command.PlayCollection(CollectionRef.Playlist(id), i, false)) },
                        // More, then the handle at the row's end, as in the queue.
                        trailing = {
                            JfIconButton(R.drawable.ic_more, moreLabel, {
                                actions.showFor(
                                    TrackTarget(row.track, row.title, subtitle, favorite = row.favorite, artist = row.artist),
                                    extra = listOf(
                                        SheetAction(removeLabel, R.drawable.ic_close, destructive = true) {
                                            graph.send(Command.RemoveFromPlaylist(id, listOf(entry.entryId)))
                                        },
                                    ),
                                )
                            }, size = 40.dp, iconSize = 20.dp)
                            DragHandle(
                                key = entry.entryId,
                                onStart = { drag = PlaylistDrag(entry.entryId, i, 0f) },
                                onDrag = { dy -> drag = drag?.let { it.copy(offsetPx = it.offsetPx + dy) } },
                                onEnd = {
                                    val held = drag
                                    drag = null
                                    if (held != null) {
                                        PlaylistReorder.drop(id, held, rowPx, count, state.libraryRevision)?.let { (move, command) ->
                                            pending = move
                                            graph.send(command)
                                        }
                                    }
                                },
                            )
                        },
                    )
                }
            }
        }
        BackBar(onBack = nav::back, title = title.takeIf { headerGone }, modifier = Modifier.windowInsetsPadding(WindowInsets.statusBars))
    }
}

/** A playlist row's height (a [TrackRow]: the 52 dp row with its 8 dp of padding). */
private val PLAYLIST_ROW = (Tokens.Player.rowHeight + 8).dp

/**
 * The handle a row is dragged by: 48 dp, labelled for TalkBack (which uses Move up and Move down).
 * The gesture outlives recompositions, so it calls the latest callbacks (a row's index changes
 * after a move while its key stays).
 */
@Composable
internal fun DragHandle(key: Long, onStart: () -> Unit, onDrag: (Float) -> Unit, onEnd: () -> Unit) {
    val label = stringResource(R.string.a11y_drag)
    val start by rememberUpdatedState(onStart)
    val move by rememberUpdatedState(onDrag)
    val end by rememberUpdatedState(onEnd)
    Box(
        Modifier
            .size(48.dp)
            .semantics { contentDescription = label }
            .pointerInput(key) {
                detectVerticalDragGestures(
                    onDragStart = { start() },
                    onDragEnd = { end() },
                    onDragCancel = { end() },
                    onVerticalDrag = { change, dy ->
                        change.consume()
                        move(dy)
                    },
                )
            },
        contentAlignment = Alignment.Center,
    ) {
        JfIcon(R.drawable.ic_drag, Jf.colors.ink50, size = 20.dp)
    }
}
