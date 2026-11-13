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
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.images.rememberArtwork
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.theme.ChipButton
import org.jellyfesh.android.ui.theme.GlassBackdrop
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.JfToggleIconButton
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.SeekBar
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.android.ui.theme.TransportCapsule
import org.jellyfesh.android.ui.theme.VolumeBar
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.android.ui.time.rememberPositionSeconds
import org.jellyfesh.logic.core.BackdropStyle
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.core.TrackRef
import org.jellyfesh.logic.format.Labels
import org.jellyfesh.logic.format.TimeFormat
import org.jellyfesh.logic.player.NowPlayingMotion
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.seek.SeekModel
import org.jellyfesh.logic.seek.SeekState
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.tokens.Tokens

/** The panels next to (or, on phones, instead of) the artwork. */
enum class NowPlayingPanel { Lyrics, Queue, Info }

/**
 * Now Playing. Wide windows show the player beside a panel (lyrics by default); phones show the
 * player, or one panel with compact controls under it, toggled from the segmented control. The
 * backdrop is the core's baked, contrast-safe artwork image in the Artwork style (crossfading
 * 600 ms on track change) or the Misty field at the artwork hue in the Misty style (ADR-0013).
 * After Apple Music (v0.0.4): the artwork rests at 88 % while paused and springs back when
 * playing, play and pause morph, the seek bar is a straight line. [requestedPanel] is the panel
 * the player bar's Lyrics or Queue button asked for.
 */
@Composable
fun NowPlayingScreen(state: CoreState, onClose: () -> Unit, requestedPanel: NowPlayingPanel? = null) {
    var panel by rememberSaveable { mutableStateOf<NowPlayingPanel?>(null) }
    LaunchedEffect(requestedPanel) {
        if (requestedPanel != null) panel = requestedPanel
    }
    val now = state.nowPlaying
    val dark = Jf.colors.dark
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val width = maxWidth
        val height = maxHeight
        val wide = width >= 720.dp && width > height
        GlassBackdrop(Modifier.fillMaxSize(), invalidateKey = state.theme) {
            Backdrop(state.settings.style, state.theme?.backdrop(dark))
            Column(Modifier.fillMaxSize()) {
                TopBar(
                    state = state,
                    panel = if (wide) panel ?: NowPlayingPanel.Lyrics else panel,
                    compact = width < 600.dp,
                    onPanel = { chosen -> panel = if (!wide && panel == chosen) null else chosen },
                    onClose = onClose,
                )
                Box(Modifier.weight(1f).fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars)) {
                    when {
                        now == null -> EmptyState(R.drawable.ic_song, stringResource(R.string.np_nothing_title), stringResource(R.string.np_nothing_body), Modifier.align(Alignment.Center))
                        wide -> Row(Modifier.fillMaxSize().padding(horizontal = 32.dp, vertical = 16.dp), horizontalArrangement = Arrangement.spacedBy(48.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f).widthIn(max = 420.dp).fillMaxHeight(), verticalArrangement = Arrangement.Center) {
                                PlayerColumn(state, artMax = 400.dp)
                            }
                            Box(Modifier.weight(1.3f).fillMaxHeight()) { PanelContent(panel ?: NowPlayingPanel.Lyrics, state) }
                        }
                        panel == null -> Column(Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 12.dp)) {
                            PlayerColumn(state, artMax = 560.dp)
                        }
                        else -> Column(Modifier.fillMaxSize()) {
                            Box(Modifier.weight(1f).fillMaxWidth()) { PanelContent(panel!!, state) }
                            CompactPlayer(state, Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
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

/** The Veil bar: close, the panel switch and the output chip (Controls D top bar). */
@Composable
private fun TopBar(state: CoreState, panel: NowPlayingPanel?, compact: Boolean, onPanel: (NowPlayingPanel) -> Unit, onClose: () -> Unit) {
    val overlays = LocalOverlays.current
    val devicesTitle = stringResource(R.string.np_output_device)
    GlassSurface(Modifier.fillMaxWidth(), grade = GlassGrade.Veil, showEdge = false) {
        Column {
            Row(
                Modifier
                    .fillMaxWidth()
                    .windowInsetsPadding(WindowInsets.statusBars)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                JfIconButton(R.drawable.ic_chevron_down, stringResource(R.string.np_close), onClose, tint = Jf.colors.ink90)
                Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                    SegmentedControl(
                        options = NowPlayingPanel.entries,
                        selected = panel,
                        label = { stringResource(panelLabel(it)) },
                        onSelect = onPanel,
                        description = stringResource(R.string.np_views),
                    )
                }
                if (compact) {
                    JfIconButton(R.drawable.ic_speaker, outputName(state), { showDevices(overlays, devicesTitle) }, tint = Jf.colors.ink90)
                } else {
                    ChipButton(R.drawable.ic_speaker, outputName(state), { showDevices(overlays, devicesTitle) }, Modifier.padding(end = 8.dp))
                }
            }
            Box(Modifier.fillMaxWidth().size(width = Dp.Unspecified, height = 1.dp).background(Jf.colors.glassEdge))
        }
    }
}

private fun panelLabel(panel: NowPlayingPanel): Int = when (panel) {
    NowPlayingPanel.Lyrics -> R.string.np_lyrics
    NowPlayingPanel.Queue -> R.string.np_queue
    NowPlayingPanel.Info -> R.string.np_info
}

@Composable
private fun PanelContent(panel: NowPlayingPanel, state: CoreState) {
    val now = state.nowPlaying ?: return
    when (panel) {
        NowPlayingPanel.Lyrics -> LyricsView(
            state.currentLyrics,
            state.settings.showTranslation,
            state.settings.showRomanization,
            playing = PlayerModel.isPlaying(state.playback.status),
            size = state.settings.lyricsSize,
            align = state.settings.lyricsAlign,
        )
        NowPlayingPanel.Queue -> QueueView(state.queue)
        NowPlayingPanel.Info -> InfoView(now, state.currentLyrics, PlayerModel.durationUs(state))
    }
}

/**
 * Artwork, title, seek bar, transport and volume. The artwork takes the height the controls leave
 * (up to [artMax]), so short screens and large fonts shrink it instead of pushing controls away.
 */
@Composable
private fun ColumnScope.PlayerColumn(state: CoreState, artMax: Dp) {
    val now = state.nowPlaying ?: return
    val style = state.settings.style
    Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
        BoxWithConstraints {
            val artSize = minOf(maxWidth, maxHeight, artMax).coerceAtLeast(96.dp)
            HeroArtwork(now.artwork?.let { ArtKey.of(it) }, now.title, artSize, style, PlayerModel.isPlaying(state.playback.status))
        }
    }
    Spacer(Modifier.size(20.dp))
    TitleBlock(state)
    Spacer(Modifier.size(12.dp))
    SeekWithTimes(state)
    Spacer(Modifier.size(8.dp))
    Transport(state)
    Spacer(Modifier.size(8.dp))
    PlayerVolume(state)
}

/** The controls under a panel on phones: a small title row, the seek bar and the transport. */
@Composable
private fun CompactPlayer(state: CoreState, modifier: Modifier = Modifier) {
    val now = state.nowPlaying ?: return
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ArtworkImage(ArtKey.of(now.artwork), now.title, 48.dp, RoundedCornerShape(Tokens.Radius.rMd.dp))
            Column(Modifier.weight(1f)) {
                JfText(now.title, style = JfType.listTitle, maxLines = 1)
                Labels.nowPlayingSubtitle(now)?.let { JfText(it, style = JfType.listSecondary, color = Jf.colors.ink70, maxLines = 1) }
            }
            Favourite(state)
        }
        SeekWithTimes(state)
        Transport(state)
    }
}

/**
 * Hero artwork: shadowed in the Artwork style; on a Clear frame in the Misty style. It rests at
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
        // so a live blur would cost a third glass layer for no visible change.
        Box(
            Modifier
                .shadow(6.dp, frame, ambientColor = colors[Tokens.Elevation.shadow1Layer0], spotColor = colors[Tokens.Elevation.shadow1Layer1])
                .background(colors[Tokens.Glass.tintClear], frame)
                .border(1.dp, colors.glassEdgeSoft, frame)
                .padding(12.dp),
        ) {
            ArtworkImage(key, title, size - 24.dp, RoundedCornerShape(Tokens.Artwork.radiusHero.dp))
        }
    } else {
        ArtworkImage(
            key,
            title,
            size,
            RoundedCornerShape(Tokens.Artwork.radiusHero.dp),
            Modifier.shadow(16.dp, RoundedCornerShape(Tokens.Artwork.radiusHero.dp), ambientColor = colors[Tokens.Elevation.shadow2Layer0], spotColor = colors[Tokens.Elevation.shadow2Layer1]),
        )
    }
}

@Composable
private fun TitleBlock(state: CoreState) {
    val now = state.nowPlaying ?: return
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            JfText(now.title, style = JfType.nowPlayingTitle, maxLines = 2, modifier = Modifier.semantics { heading() })
            Labels.nowPlayingSubtitle(now)?.let { JfText(it, style = JfType.nowPlayingArtist, color = Jf.colors.ink70, maxLines = 2) }
            if (state.isBuffering) JfText(stringResource(R.string.np_buffering), style = JfType.caption, color = Jf.colors.ink50)
        }
        Favourite(state)
    }
}

/** The heart for library songs (the core keeps favourites; sources keep their own). */
@Composable
private fun Favourite(state: CoreState) {
    val now = state.nowPlaying ?: return
    val track = now.track
    if (track !is TrackRef.Library && track !is TrackRef.File) return
    val graph = LocalGraph.current
    val favourite by rememberQuery(now.uid, state.libraryRevision) { it.isFavorite(track) }
    val on = (favourite as? org.jellyfesh.android.QueryResult.Ok)?.value ?: false
    JfToggleIconButton(
        icon = if (on) R.drawable.ic_heart_filled else R.drawable.ic_heart,
        description = stringResource(R.string.a11y_favourite),
        stateText = stringResource(if (on) R.string.state_on else R.string.state_off),
        checked = on,
        onToggle = { graph.send(Command.SetFavorite(track, it)) },
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
    val generation = state.playback.anchor.trackGeneration
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

/** Shuffle, the transport capsule and repeat. */
@Composable
private fun Transport(state: CoreState) {
    val graph = LocalGraph.current
    val playing = PlayerModel.isPlaying(state.playback.status)
    val queue = state.queue
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        JfToggleIconButton(
            R.drawable.ic_shuffle,
            stringResource(R.string.a11y_shuffle),
            stringResource(if (queue.shuffle) R.string.state_on else R.string.state_off),
            queue.shuffle,
            { graph.send(Command.SetShuffle(it)) },
        )
        TransportCapsule(
            playing = playing,
            playDescription = stringResource(if (playing) R.string.a11y_pause else R.string.a11y_play),
            previousDescription = stringResource(R.string.a11y_previous),
            nextDescription = stringResource(R.string.a11y_next),
            onPrevious = { graph.send(Command.Previous) },
            onPlayPause = { graph.send(PlayerModel.playPause(state.playback.status)) },
            onNext = { graph.send(Command.Next) },
        )
        JfToggleIconButton(
            if (queue.repeat == RepeatMode.One) R.drawable.ic_repeat_one else R.drawable.ic_repeat,
            stringResource(R.string.a11y_repeat),
            stringResource(
                when (queue.repeat) {
                    RepeatMode.Off -> R.string.state_off
                    RepeatMode.All -> R.string.repeat_all
                    RepeatMode.One -> R.string.repeat_one
                },
            ),
            queue.repeat != RepeatMode.Off,
            { graph.send(Command.SetRepeat(PlayerModel.nextRepeat(queue.repeat))) },
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
