package org.jellyfesh.android.ui.player

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.images.rememberArtwork
import org.jellyfesh.android.ui.actions.TrackTarget
import org.jellyfesh.android.ui.actions.rememberTrackActions
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.SheetAction
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.theme.GlassBackdrop
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.JfToggleIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.PlayPauseButton
import org.jellyfesh.android.ui.theme.SeekBar
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.android.ui.theme.TouchTarget
import org.jellyfesh.android.ui.theme.VolumeBar
import org.jellyfesh.android.ui.theme.jfShadow
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.android.ui.time.rememberPositionSeconds
import org.jellyfesh.logic.core.BackdropStyle
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.player.NowPlayingMotion
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.player.PlayerTrack
import org.jellyfesh.logic.player.RemoteModel
import org.jellyfesh.logic.player.RemoteSession
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.seek.SeekModel
import org.jellyfesh.logic.seek.SeekState
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.theme.Luminous
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.tokens.Tokens

/** The panels next to (or, on phones, instead of) the artwork. */
enum class NowPlayingPanel { Lyrics, Queue, Info }

/**
 * Now Playing as a stage (Luminous, v0.0.6): the core's baked, contrast-safe artwork backdrop
 * behind everything in the Artwork style (crossfading over dur-4 on track change), or the Misty
 * field in the Misty style (ADR-0013); large artwork with shadow-3; the title and, for a remote
 * session, "Playing on {Device}" with its error in a quieter line; and every control in one
 * floating Veil dock: the scrubber with its times, shuffle, previous, play, next, repeat, the
 * volume, the device button and More (a rounded card on phones, an 80 dp bar on wide windows).
 * Wide windows show a panel beside the artwork (lyrics by default); phones show the artwork, or one
 * panel with the dock under it, chosen with the round buttons at the top. The dock is the screen's
 * one live glass layer; the top controls take the tint alone. While a remote session runs, the
 * views show its song (PlayerModel.track) and its controls reach the service's player (the core
 * routes the usual commands). [requestedPanel] is the panel the player bar's Lyrics or Queue
 * button asked for.
 */
@Composable
fun NowPlayingScreen(state: CoreState, onClose: () -> Unit, requestedPanel: NowPlayingPanel? = null) {
    var panel by rememberSaveable { mutableStateOf<NowPlayingPanel?>(null) }
    LaunchedEffect(requestedPanel) {
        if (requestedPanel != null) panel = requestedPanel
    }
    val track = PlayerModel.track(state)
    val dark = Jf.colors.dark
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val width = maxWidth
        val height = maxHeight
        val wide = width >= 720.dp && width > height
        GlassBackdrop(Modifier.fillMaxSize(), invalidateKey = state.theme) {
            // The core bakes no backdrop for a remote session's song: the field stands in rather than another song's.
            Backdrop(state.settings.style, state.theme?.backdrop(dark)?.takeIf { !PlayerModel.isRemote(state) })
            Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars)) {
                TopControls(
                    panel = if (wide) panel ?: NowPlayingPanel.Lyrics else panel,
                    compact = width < 600.dp,
                    onPanel = { chosen -> panel = if (!wide && panel == chosen) null else chosen },
                    onClose = onClose,
                )
                Box(Modifier.weight(1f).fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars)) {
                    when {
                        track == null -> EmptyState(R.drawable.ic_song, stringResource(R.string.np_nothing_title), stringResource(R.string.np_nothing_body), Modifier.align(Alignment.Center))
                        wide -> Column(Modifier.fillMaxSize().padding(start = 32.dp, end = 32.dp, top = 8.dp, bottom = 16.dp)) {
                            Row(Modifier.weight(1f).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(48.dp), verticalAlignment = Alignment.CenterVertically) {
                                Column(Modifier.weight(1f).widthIn(max = 420.dp).fillMaxHeight(), verticalArrangement = Arrangement.Center) {
                                    HeroArea(state, track, artMax = 380.dp)
                                    Spacer(Modifier.size(20.dp))
                                    TitleBlock(state, track, width.value)
                                }
                                Box(Modifier.weight(1.3f).fillMaxHeight()) { PanelContent(panel ?: NowPlayingPanel.Lyrics, state, track) }
                            }
                            Spacer(Modifier.size(12.dp))
                            Dock(state, track, wide = true)
                        }
                        panel == null -> Column(Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 8.dp)) {
                            HeroArea(state, track, artMax = 560.dp, Modifier.padding(horizontal = 8.dp))
                            Spacer(Modifier.size(18.dp))
                            TitleBlock(state, track, width.value, Modifier.padding(horizontal = 8.dp))
                            Spacer(Modifier.size(14.dp))
                            Dock(state, track, wide = false)
                        }
                        else -> Column(Modifier.fillMaxSize()) {
                            Box(Modifier.weight(1f).fillMaxWidth()) { PanelContent(panel!!, state, track) }
                            Dock(state, track, wide = false, Modifier.padding(horizontal = 16.dp, vertical = 8.dp), withTitle = true)
                        }
                    }
                }
            }
        }
    }
}

/** The baked artwork backdrop (Artwork style) or the Misty field (Misty style, or no backdrop yet). */
@Composable
private fun Backdrop(style: BackdropStyle, path: String?) {
    val reduce = Jf.effects.reduceMotion
    val graph = LocalGraph.current
    val target = if (style == BackdropStyle.Artwork) path else null
    Crossfade(target, animationSpec = tween(if (reduce) 0 else Tokens.Motion.dur4Ms, easing = JfMotion.Fluid), label = "backdrop") { p ->
        if (p == null) {
            MistyField(Modifier.fillMaxSize())
        } else {
            // The bake is small (long edge 256 px) and already blurred: draw it scaled up, smoothly.
            val image = rememberArtwork(graph.images, ArtKey.File(p), 256)
            if (image == null) {
                MistyField(Modifier.fillMaxSize())
            } else {
                Image(image, contentDescription = null, contentScale = ContentScale.Crop, filterQuality = FilterQuality.Low, modifier = Modifier.fillMaxSize())
            }
        }
    }
}

/**
 * The top controls, floating over the backdrop: close (a round glass button) and the panel switch:
 * Lyrics, Queue and Info as round glyph buttons on phones, a segmented control on wider windows.
 * They take the Veil tint alone: nothing scrolls under them, so a live blur would cost a layer for
 * nothing.
 */
@Composable
private fun TopControls(panel: NowPlayingPanel?, compact: Boolean, onPanel: (NowPlayingPanel) -> Unit, onClose: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 60.dp)
            .padding(horizontal = 16.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        GlassSurface(grade = GlassGrade.Veil, shape = CircleShape, blur = false) {
            JfIconButton(R.drawable.ic_chevron_down, stringResource(R.string.np_close), onClose, tint = Jf.colors.ink90)
        }
        if (compact) {
            PanelButtons(panel, onPanel)
        } else {
            GlassSurface(grade = GlassGrade.Veil, shape = RoundedCornerShape(14.dp), blur = false) {
                SegmentedControl(
                    options = NowPlayingPanel.entries,
                    selected = panel,
                    label = { stringResource(panelLabel(it)) },
                    onSelect = onPanel,
                    description = stringResource(R.string.np_views),
                )
            }
        }
    }
}

/** Lyrics, Queue and Info as round glyph buttons in one glass capsule; the chosen one on a soft fill. */
@Composable
private fun PanelButtons(selected: NowPlayingPanel?, onSelect: (NowPlayingPanel) -> Unit) {
    val colors = Jf.colors
    val views = stringResource(R.string.np_views)
    GlassSurface(grade = GlassGrade.Veil, shape = CircleShape, blur = false) {
        Row(Modifier.padding(3.dp).selectableGroup().semantics { contentDescription = views }, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            NowPlayingPanel.entries.forEach { option ->
                val on = option == selected
                val label = stringResource(panelLabel(option))
                Box(
                    Modifier
                        .size(width = TouchTarget, height = 44.dp)
                        .clip(CircleShape)
                        .background(if (on) colors[Tokens.Line.fillHover] else Color.Transparent)
                        .selectable(on, role = Role.Tab) { onSelect(option) }
                        .semantics { contentDescription = label },
                    contentAlignment = Alignment.Center,
                ) {
                    JfIcon(panelIcon(option), if (on) colors.ink90 else colors.ink70, size = 20.dp)
                }
            }
        }
    }
}

private fun panelLabel(panel: NowPlayingPanel): Int = when (panel) {
    NowPlayingPanel.Lyrics -> R.string.np_lyrics
    NowPlayingPanel.Queue -> R.string.np_queue
    NowPlayingPanel.Info -> R.string.np_info
}

private fun panelIcon(panel: NowPlayingPanel): Int = when (panel) {
    NowPlayingPanel.Lyrics -> R.drawable.ic_lyrics
    NowPlayingPanel.Queue -> R.drawable.ic_queue
    NowPlayingPanel.Info -> R.drawable.ic_info
}

/**
 * A panel's content. While a remote session runs, Lyrics and Queue say where they are instead
 * ("Lyrics aren't shown while {Service} plays.", "Up Next is in {Service}.").
 */
@Composable
private fun PanelContent(panel: NowPlayingPanel, state: CoreState, track: PlayerTrack) {
    val remote = RemoteModel.session(state)
    when (panel) {
        NowPlayingPanel.Lyrics -> if (remote != null) {
            RemoteNote(R.drawable.ic_lyrics, stringResource(R.string.remote_lyrics, remote.service))
        } else {
            LyricsView(
                state.currentLyrics,
                state.settings.showTranslation,
                state.settings.showRomanization,
                playing = PlayerModel.isPlaying(PlayerModel.status(state)),
                size = state.settings.lyricsSize,
                align = state.settings.lyricsAlign,
            )
        }
        NowPlayingPanel.Queue -> if (remote != null) {
            RemoteNote(R.drawable.ic_queue, stringResource(R.string.remote_queue, remote.service))
        } else {
            QueueView(state.queue)
        }
        NowPlayingPanel.Info -> InfoView(track, state.currentLyrics.takeIf { remote == null }, PlayerModel.durationUs(state))
    }
}

/** What a pane says while the service plays: a glyph and one quiet sentence, centred. */
@Composable
private fun RemoteNote(icon: Int, text: String) {
    val colors = Jf.colors
    Column(
        Modifier.fillMaxSize().padding(horizontal = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
    ) {
        Box(Modifier.size(56.dp).clip(CircleShape).background(colors.accentSoft), contentAlignment = Alignment.Center) {
            JfIcon(icon, colors.accentText, size = 26.dp)
        }
        JfText(text, style = JfType.body, color = colors.ink70, textAlign = TextAlign.Center)
    }
}

/** The artwork, as large as the space the title and the dock leave (up to [artMax]). */
@Composable
private fun androidx.compose.foundation.layout.ColumnScope.HeroArea(state: CoreState, track: PlayerTrack, artMax: Dp, modifier: Modifier = Modifier) {
    Box(modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
        BoxWithConstraints {
            val artSize = minOf(maxWidth, maxHeight, artMax).coerceAtLeast(96.dp)
            HeroArtwork(ArtKey.of(track.artwork), track.title, artSize, state.settings.style, PlayerModel.isPlaying(PlayerModel.status(state)))
        }
    }
}

/**
 * Hero artwork: shadow-3 in the Artwork style; on a Clear frame in the Misty style. It rests at
 * 88 % while paused and springs back to full size when playing (dur-3 Spring); with reduced
 * motion it stays at full size.
 */
@Composable
private fun HeroArtwork(key: ArtKey?, title: String, size: Dp, style: BackdropStyle, playing: Boolean) {
    val colors = Jf.colors
    val reduce = Jf.effects.reduceMotion
    val scale by animateFloatAsState(NowPlayingMotion.artworkScale(playing, reduce), JfMotion.spring(Tokens.Motion.dur3Ms, reduce), label = "artwork")
    Box(Modifier.graphicsLayer { scaleX = scale; scaleY = scale }) {
        HeroArtworkImage(key, title, size, style, colors)
    }
}

@Composable
private fun HeroArtworkImage(key: ArtKey?, title: String, size: Dp, style: BackdropStyle, colors: org.jellyfesh.android.ui.theme.JfColors) {
    if (style == BackdropStyle.Misty) {
        val frame = RoundedCornerShape(36.dp)
        // A Clear surface drawn with its tint and edge only: the field behind is a smooth gradient,
        // so a live blur would cost a glass layer for no visible change.
        Box(
            Modifier
                .jfShadow(1, frame)
                .background(colors[Tokens.Glass.tintClear], frame)
                .border(1.dp, colors.glassEdgeSoft, frame)
                .padding(12.dp),
        ) {
            ArtworkImage(key, title, size - 24.dp, RoundedCornerShape(Tokens.Artwork.radiusHero.dp))
        }
    } else {
        val shape = RoundedCornerShape(Tokens.Artwork.radiusHero.dp)
        ArtworkImage(key, title, size, shape, Modifier.jfShadow(3, shape))
    }
}

/**
 * The title (type-roles.now-playing-title, 32, scaled for phones), `Artist · Album`, and for a
 * remote session "Playing on {Device}" with its error in a quieter line; the heart for library
 * songs.
 */
@Composable
private fun TitleBlock(state: CoreState, track: PlayerTrack, widthDp: Float, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    val remote = RemoteModel.session(state)
    val titleSize = Luminous.nowPlayingTitleSp(widthDp, Tokens.TypeRoles.nowPlayingTitle.size)
    val titleStyle = JfType.nowPlayingTitle.copy(fontSize = titleSize.sp, lineHeight = (titleSize * Tokens.TypeRoles.nowPlayingTitle.lineHeight).sp)
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            JfText(track.title, style = titleStyle, maxLines = 2, modifier = Modifier.semantics { heading() })
            track.subtitle?.let { JfText(it, style = JfType.nowPlayingArtist, color = colors.ink70, maxLines = 2) }
            RemoteLines(remote)
            if (remote == null && state.isBuffering) JfText(stringResource(R.string.np_buffering), style = JfType.caption, color = colors.ink50)
        }
        Favourite(state, track)
    }
}

/** "Playing on {Device}", and under it, quieter, what went wrong last talking to the service. */
@Composable
private fun RemoteLines(remote: RemoteSession?) {
    if (remote == null) return
    val colors = Jf.colors
    remote.device?.let { JfText(stringResource(R.string.remote_playing_on, it.name), style = JfType.label, color = colors.ink90, maxLines = 1) }
    // Quieter, yet never below ink-70 on glass and the backdrop (Luminous contrast).
    remote.error?.let { JfText(it, style = JfType.small, color = colors.ink70, maxLines = 2) }
}

/** The heart for library songs (the core keeps favourites; sources keep their own). */
@Composable
private fun Favourite(state: CoreState, track: PlayerTrack) {
    val now = track.local ?: return
    val ref = now.track
    if (ref !is TrackRef.Library && ref !is TrackRef.File) return
    val graph = LocalGraph.current
    val favourite by rememberQuery(now.uid, state.libraryRevision) { it.isFavorite(ref) }
    val on = (favourite as? org.jellyfesh.android.QueryResult.Ok)?.value ?: false
    JfToggleIconButton(
        icon = if (on) R.drawable.ic_heart_filled else R.drawable.ic_heart,
        description = stringResource(R.string.a11y_favourite),
        stateText = stringResource(if (on) R.string.state_on else R.string.state_off),
        checked = on,
        onToggle = { graph.send(Command.SetFavorite(ref, it)) },
    )
}

/**
 * The dock (Luminous): every control in one floating Veil surface. On phones a rounded card: (with
 * a panel open, a small title row), the scrubber with its times, the transport, then the volume,
 * the device button and More. On wide windows one 80 dp bar with a 40 dp radius.
 */
@Composable
private fun Dock(state: CoreState, track: PlayerTrack, wide: Boolean, modifier: Modifier = Modifier, withTitle: Boolean = false) {
    val shape = RoundedCornerShape(if (wide) 40.dp else 32.dp)
    GlassSurface(modifier.fillMaxWidth(), grade = GlassGrade.Veil, shape = shape, floating = true) {
        if (wide) {
            Row(
                Modifier.fillMaxWidth().height(80.dp).padding(start = 20.dp, end = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Transport(state, playSize = 54.dp, spaced = false)
                SeekWithTimes(state, Modifier.weight(1f))
                DockVolume(state, Modifier.width(180.dp))
                DeviceButton(state)
                MoreButton(state, track)
            }
        } else {
            Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 12.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                if (withTitle) CompactTitle(state, track)
                SeekWithTimes(state)
                Transport(state, playSize = 64.dp, spaced = true)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    DockVolume(state, Modifier.weight(1f))
                    DeviceButton(state)
                    MoreButton(state, track)
                }
            }
        }
    }
}

/** The small title row of the dock while a panel is open on a phone. */
@Composable
private fun CompactTitle(state: CoreState, track: PlayerTrack) {
    val remote = RemoteModel.session(state)
    Row(Modifier.padding(bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        ArtworkImage(ArtKey.of(track.artwork), track.title, 48.dp, RoundedCornerShape(Tokens.Radius.rMd.dp))
        Column(Modifier.weight(1f)) {
            JfText(track.title, style = JfType.listTitle, maxLines = 1)
            val second = remote?.device?.let { stringResource(R.string.remote_playing_on, it.name) } ?: track.subtitle
            second?.let { JfText(it, style = JfType.listSecondary, color = Jf.colors.ink70, maxLines = 1) }
            remote?.error?.let { JfText(it, style = JfType.listSecondary.copy(fontSize = 11.sp), color = Jf.colors.ink70, maxLines = 1) }
        }
        Favourite(state, track)
    }
}

/**
 * More: the song's menu (Open in {Service} among it, when the song has a link); while a remote
 * session runs, Open in {Service} and Stop Controlling (`Remote { End }`: the service keeps playing).
 */
@Composable
private fun MoreButton(state: CoreState, track: PlayerTrack) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val actions = rememberTrackActions()
    val remote = RemoteModel.session(state)
    val stop = stringResource(R.string.remote_stop)
    val playingOn = remote?.device?.let { stringResource(R.string.remote_playing_on, it.name) }
    val openIn = remote?.let { stringResource(R.string.action_open_in, it.service) }
    JfIconButton(R.drawable.ic_more, stringResource(R.string.action_more), {
        val local = track.local
        val link = track.link
        when {
            remote != null -> overlays.showActions(
                track.title,
                playingOn ?: track.subtitle,
                listOfNotNull(
                    if (link != null && openIn != null) SheetAction(openIn, R.drawable.ic_external) { actions.openLink(link) } else null,
                    SheetAction(stop, R.drawable.ic_close) { graph.send(RemoteModel.end) },
                ),
            )
            local != null -> actions.showFor(
                TrackTarget(local.track, track.title, track.subtitle, artist = track.artist, link = track.link, source = track.source),
            )
        }
    }, tint = Jf.colors.ink90)
}

/** The volume in the dock: the engine's, or the playing remote device's (hidden when it has none). */
@Composable
private fun DockVolume(state: CoreState, modifier: Modifier) {
    if (!PlayerModel.isRemote(state)) {
        PlayerVolume(state, modifier)
        return
    }
    val volume = RemoteModel.session(state)?.volume
    if (volume != null) RemoteVolume(volume, modifier) else Spacer(modifier)
}

/**
 * The playing remote device's volume (`Remote { SetVolume }`). The service answers each change over
 * the network, so the line follows the finger and the volume goes when it lifts; the chosen value
 * shows until the service reports it (or 3 s pass).
 */
@Composable
private fun RemoteVolume(volume: Float, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    var dragging by remember { mutableStateOf<Float?>(null) }
    var sent by remember { mutableStateOf<Float?>(null) }
    LaunchedEffect(volume) { sent = null }
    LaunchedEffect(sent) {
        if (sent != null) {
            delay(3_000)
            sent = null
        }
    }
    val shown = dragging ?: sent ?: volume
    VolumeBar(
        volume = shown,
        muted = false,
        description = stringResource(R.string.a11y_volume),
        valueText = stringResource(R.string.a11y_volume_value, (shown * 100).toInt()),
        muteDescription = null,
        onVolume = { dragging = it },
        onVolumeEnd = {
            dragging?.let {
                sent = it
                graph.send(RemoteModel.volume(it))
            }
            dragging = null
        },
        onToggleMute = null,
        modifier = modifier,
    )
}

/**
 * The seek bar with its times (elapsed, and remaining as −1:23), following the core's anchor
 * through the ticker: it redraws when the fill moves by a device pixel and the times each second.
 * While dragging, the times follow the finger. Also the player bar's.
 */
@Composable
fun SeekWithTimes(state: CoreState, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    val time = LocalPlaybackTime.current
    val durationMs = PlayerModel.durationUs(state)?.div(1_000)
    val generation = PlayerModel.anchor(state).trackGeneration
    var seek by remember { mutableStateOf(SeekState()) }
    // A sent seek holds the thumb until the core shows it (a new generation) or 1.5 s pass.
    LaunchedEffect(seek.pendingMs, generation) {
        if (seek.pendingMs == null) return@LaunchedEffect
        seek = SeekModel.settle(seek, generation, System.currentTimeMillis())
        if (seek.pendingMs != null) {
            delay(SeekModel.SETTLE_MS)
            seek = SeekModel.settle(seek, generation, System.currentTimeMillis())
        }
    }
    val fraction = {
        SeekModel.displayFraction(seek, time.positionUs / 1_000, durationMs, generation, System.currentTimeMillis())
    }
    var barSteps by remember { mutableStateOf(300) }
    TimeDemandEffect(TimeDemand.Steps(barSteps))
    TimeDemandEffect(TimeDemand.Seconds)
    val seconds by rememberPositionSeconds()
    val elapsedMs = if (seek.dragging && durationMs != null) (seek.dragFraction * durationMs).toLong() else seconds * 1_000
    val elapsed = TimeFormat.clock(elapsedMs)
    val remaining = durationMs?.let { TimeFormat.remaining(elapsedMs, it) } ?: "−:--"
    val total = durationMs?.let { TimeFormat.clock(it) } ?: "--:--"
    SeekBar(
        fraction = fraction,
        enabled = durationMs != null && durationMs > 0,
        dragging = seek.dragging,
        valueText = stringResource(R.string.a11y_position_value, elapsed, total),
        description = stringResource(R.string.a11y_position),
        onDragStart = { seek = SeekModel.start(it) },
        onDrag = { seek = SeekModel.move(seek, it) },
        onDragEnd = {
            val (next, target) = SeekModel.end(seek, durationMs, generation, System.currentTimeMillis())
            seek = next
            target?.let { graph.send(Command.Seek(it)) }
        },
        onDragCancel = { seek = SeekModel.cancel() },
        onSeekTo = { f ->
            val d = durationMs
            if (d != null && d > 0) {
                val target = (f * d).toLong()
                seek = SeekState(pendingMs = target, pendingSinceMs = System.currentTimeMillis(), pendingGeneration = generation)
                graph.send(Command.Seek(target))
            }
        },
        modifier = modifier,
        onStep = { key ->
            val d = durationMs
            if (d != null && d > 0) {
                // From the drawn position, so quick presses add up while a seek is in flight.
                val target = SeekGeometry.keyTarget(key, (fraction() * d).toLong(), d)
                seek = SeekState(pendingMs = target, pendingSinceMs = System.currentTimeMillis(), pendingGeneration = generation)
                graph.send(Command.Seek(target))
            }
        },
        onWidthPx = { barSteps = it.coerceAtLeast(1) },
        leading = { JfText(elapsed, style = JfType.time, color = Jf.colors.ink70, maxLines = 1) },
        trailing = { JfText(remaining, style = JfType.time, color = Jf.colors.ink70, maxLines = 1) },
    )
}

/**
 * Shuffle, previous, play, next and repeat, spread over the dock's width on phones ([spaced]). They
 * send the usual commands; during a remote session the core routes them to the service.
 */
@Composable
private fun Transport(state: CoreState, playSize: Dp, spaced: Boolean) {
    val graph = LocalGraph.current
    val status = PlayerModel.status(state)
    val playing = PlayerModel.isPlaying(status)
    val shuffle = PlayerModel.shuffle(state)
    val repeat = PlayerModel.repeat(state)
    Row(
        if (spaced) Modifier.fillMaxWidth() else Modifier,
        horizontalArrangement = if (spaced) Arrangement.SpaceBetween else Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        JfToggleIconButton(
            R.drawable.ic_shuffle,
            stringResource(R.string.a11y_shuffle),
            stringResource(if (shuffle) R.string.state_on else R.string.state_off),
            shuffle,
            { graph.send(Command.SetShuffle(it)) },
        )
        JfIconButton(R.drawable.ic_previous, stringResource(R.string.a11y_previous), { graph.send(Command.Previous) }, tint = Jf.colors.ink90, size = 52.dp, iconSize = 28.dp)
        PlayPauseButton(playing, stringResource(if (playing) R.string.a11y_pause else R.string.a11y_play), { graph.send(PlayerModel.playPause(status)) }, size = playSize)
        JfIconButton(R.drawable.ic_next, stringResource(R.string.a11y_next), { graph.send(Command.Next) }, tint = Jf.colors.ink90, size = 52.dp, iconSize = 28.dp)
        JfToggleIconButton(
            if (repeat == RepeatMode.One) R.drawable.ic_repeat_one else R.drawable.ic_repeat,
            stringResource(R.string.a11y_repeat),
            stringResource(
                when (repeat) {
                    RepeatMode.Off -> R.string.state_off
                    RepeatMode.All -> R.string.repeat_all
                    RepeatMode.One -> R.string.repeat_one
                },
            ),
            repeat != RepeatMode.Off,
            { graph.send(Command.SetRepeat(PlayerModel.nextRepeat(repeat))) },
        )
    }
}

/** Volume, sent at most every 50 ms while dragging and once more when the finger lifts. Also the player bar's. */
@Composable
fun PlayerVolume(state: CoreState, modifier: Modifier = Modifier) {
    val graph = LocalGraph.current
    var local by remember { mutableStateOf<Float?>(null) }
    var lastSent by remember { mutableStateOf(0L) }
    val shown = local ?: state.playback.volume
    VolumeBar(
        volume = shown,
        muted = state.muted,
        description = stringResource(R.string.a11y_volume),
        valueText = stringResource(R.string.a11y_volume_value, (shown * 100).toInt()),
        muteDescription = stringResource(if (state.muted) R.string.a11y_unmute else R.string.a11y_mute),
        onVolume = { v ->
            local = v
            val nowMs = System.currentTimeMillis()
            if (nowMs - lastSent >= 50) {
                lastSent = nowMs
                graph.send(Command.SetVolume(v))
                if (state.muted) graph.send(Command.SetMuted(false))
            }
        },
        onVolumeEnd = {
            local?.let { graph.send(Command.SetVolume(it)) }
            local = null
        },
        onToggleMute = { graph.send(Command.SetMuted(!state.muted)) },
        modifier = modifier,
    )
}
