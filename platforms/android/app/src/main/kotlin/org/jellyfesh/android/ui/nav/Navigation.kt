package org.jellyfesh.android.ui.nav

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import org.jellyfesh.android.R
import org.jellyfesh.android.images.ArtKey
import org.jellyfesh.android.ui.components.ArtworkImage
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.GlassGrade
import org.jellyfesh.android.ui.theme.GlassSurface
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.JfIconButton
import org.jellyfesh.android.ui.theme.JfToggleIconButton
import org.jellyfesh.android.ui.theme.MiniProgressLine
import org.jellyfesh.android.ui.theme.PlayPauseButton
import org.jellyfesh.android.ui.theme.PlayPauseGlyph
import org.jellyfesh.android.ui.theme.TouchTarget
import org.jellyfesh.android.ui.player.DeviceButton
import org.jellyfesh.android.ui.player.LyricsBarStrip
import org.jellyfesh.android.ui.player.PlayerVolume
import org.jellyfesh.android.ui.player.SeekWithTimes
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.TimeDemandEffect
import org.jellyfesh.logic.player.MiniPlayerKind
import org.jellyfesh.logic.player.NowPlayingPanel
import org.jellyfesh.logic.nav.LibrarySection
import org.jellyfesh.logic.nav.NavState
import org.jellyfesh.logic.nav.Navigator
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.nav.SettingsSection
import org.jellyfesh.logic.nav.Tab
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.RepeatMode
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.player.PlayerTrack
import org.jellyfesh.logic.player.RemoteModel
import org.jellyfesh.logic.player.RemoteSession
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.time.TimeDemand
import org.jellyfesh.tokens.Tokens
import org.jellyfesh.android.ui.theme.rounded

/** Moves through :logic's [Navigator] over the app's navigation state. Main thread. */
@Stable
class NavController(private val state: MutableStateFlow<NavState>) {
    val current: NavState get() = state.value

    fun push(screen: Screen) = state.update { Navigator.push(it, screen) }

    fun replaceTop(screen: Screen) = state.update { Navigator.replaceTop(it, screen) }

    fun select(tab: Tab) = state.update { Navigator.select(it, tab) }

    /** Back, unless there is nowhere left (the system then leaves the app). */
    fun back() = state.update { Navigator.back(it) ?: it }

    fun openNowPlaying() = state.update(Navigator::openNowPlaying)

    fun closeNowPlaying() = state.update(Navigator::closeNowPlaying)

    fun openAlbum(id: Long, title: String) = push(Screen.Album(id, title))

    fun openArtist(id: Long, name: String) = push(Screen.Artist(id, name))

    fun openPlaylist(id: Long, name: String) = push(Screen.Playlist(id, name))

    fun openSettings(section: SettingsSection? = null) = push(Screen.Settings(section))

    fun openLibrary(section: LibrarySection) {
        select(Tab.Library)
        replaceTop(Screen.Library(section))
    }
}

val LocalNav = staticCompositionLocalOf<NavController> { error("NavController is missing") }

/** The mini-player pill's height (Luminous phone screens: 64, half of it the pill's radius). */
val MiniPlayerHeight = 64.dp

/** The floating tab bar capsule's height. */
val TabBarHeight = 68.dp

/** The player bar's height (player.bar-height), for windows 900 dp and wider. */
val PlayerBarHeight = Tokens.Player.barHeight.dp

/** The look's Floating mini-player: a smaller capsule at the trailing side (ADR-0020). */
val FloatingPlayerHeight = 52.dp
private val FloatingPlayerWidth = 280.dp

/** The player's height for its kind. */
fun playerHeight(kind: MiniPlayerKind): Dp = when (kind) {
    MiniPlayerKind.Bar -> PlayerBarHeight
    MiniPlayerKind.Mini -> MiniPlayerHeight
    MiniPlayerKind.Floating -> FloatingPlayerHeight
}

/** Between the floating capsules and the screen's sides, and between the two capsules. */
private val ChromeSide = 14.dp
private val ChromeGap = 8.dp

/** Below the tab bar capsule, above the system navigation bar. */
private val ChromeBottom = 10.dp

/**
 * How much of the screen's bottom the floating chrome covers (the system navigation bar not
 * included): the tab bar, the mini-player or the player bar when something plays, and the lyrics
 * bar above it when it shows ([lyricsBar], its height). Screens leave this free under their last
 * rows, and their content fades out just above it.
 */
fun chromeHeight(hasPlayer: Boolean, kind: MiniPlayerKind, lyricsBar: Dp? = null): Dp =
    ChromeBottom + TabBarHeight +
        (if (!hasPlayer) 0.dp else ChromeGap + playerHeight(kind)) +
        (if (hasPlayer && lyricsBar != null) ChromeGap + lyricsBar else 0.dp)

/**
 * The floating chrome at the bottom of the library screens (Luminous): the mini-player pill (when
 * something plays; the player bar from 900 dp, tablets and desktop mode) above the tab bar
 * capsule, each a floating Veil glass layer: the two live blur layers Android's budget allows
 * (Tokens.GlassBudget.liveLayersMobile).
 */
@Composable
fun BottomChrome(
    state: CoreState,
    tab: Tab,
    kind: MiniPlayerKind,
    onSelect: (Tab) -> Unit,
    onOpenNowPlaying: (NowPlayingPanel?) -> Unit,
    modifier: Modifier = Modifier,
    lyricsBar: Boolean = false,
) {
    Column(
        modifier
            .fillMaxWidth()
            .windowInsetsPadding(WindowInsets.navigationBars)
            .padding(start = ChromeSide, end = ChromeSide, bottom = ChromeBottom),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(ChromeGap),
    ) {
        if (PlayerModel.track(state) != null) {
            // The line being sung, above the player (Show Lyrics Bar).
            if (lyricsBar) LyricsBarStrip(state, { onOpenNowPlaying(NowPlayingPanel.Lyrics) }, Modifier.widthIn(max = 600.dp))
            // The look's mini-player style: the bar (wide windows), the pill, or a floating capsule.
            when (kind) {
                MiniPlayerKind.Bar -> PlayerBar(state, onOpenNowPlaying, Modifier.widthIn(max = 1240.dp))
                MiniPlayerKind.Mini -> MiniPlayer(state, { onOpenNowPlaying(null) }, Modifier.widthIn(max = 600.dp))
                MiniPlayerKind.Floating -> Box(Modifier.fillMaxWidth().widthIn(max = 600.dp), contentAlignment = Alignment.CenterEnd) {
                    MiniPlayer(state, { onOpenNowPlaying(null) }, Modifier.widthIn(max = FloatingPlayerWidth), floating = true)
                }
            }
        }
        TabBar(tab, onSelect, Modifier.widthIn(max = 600.dp))
    }
}

/** The tab bar: a floating Veil capsule, each tab's glyph in a pill that takes the accent's soft tint when chosen. */
@Composable
private fun TabBar(selected: Tab, onSelect: (Tab) -> Unit, modifier: Modifier = Modifier) {
    val colors = Jf.colors
    GlassSurface(
        modifier.fillMaxWidth().height(TabBarHeight),
        grade = GlassGrade.Veil,
        shape = rounded(TabBarHeight / 2),
        floating = true,
    ) {
        Row(
            Modifier
                .fillMaxSize()
                .padding(horizontal = 6.dp)
                .selectableGroup(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Tab.entries.forEach { tab ->
                val isSelected = tab == selected
                val label = stringResource(tabLabel(tab))
                Column(
                    Modifier
                        .weight(1f)
                        .selectable(isSelected, remember { MutableInteractionSource() }, indication = null, role = Role.Tab) { onSelect(tab) }
                        .padding(vertical = 4.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Box(
                        Modifier
                            .size(width = 54.dp, height = 30.dp)
                            .background(if (isSelected) colors.accentSoft else Color.Transparent, rounded(15.dp)),
                        contentAlignment = Alignment.Center,
                    ) {
                        JfIcon(tabIcon(tab), if (isSelected) colors.accentText else colors.ink70, size = 21.dp)
                    }
                    JfText(label, style = JfType.micro, color = if (isSelected) colors.accentText else colors.ink70, maxLines = 1)
                }
            }
        }
    }
}

fun tabLabel(tab: Tab): Int = when (tab) {
    Tab.Home -> R.string.tab_home
    Tab.Library -> R.string.tab_library
    Tab.Search -> R.string.tab_search
    Tab.Sources -> R.string.tab_sources
}

private fun tabIcon(tab: Tab): Int = when (tab) {
    Tab.Home -> R.drawable.ic_home
    Tab.Library -> R.drawable.ic_library
    Tab.Search -> R.drawable.ic_search
    Tab.Sources -> R.drawable.ic_sources
}

/**
 * The line under the title in the mini-player and the player bar: "Playing on {Device}" while a
 * remote session names its device, else `Artist · Album`.
 */
@Composable
fun playerSecondLine(track: PlayerTrack, remote: RemoteSession?): String? =
    remote?.device?.let { stringResource(R.string.remote_playing_on, it.name) } ?: track.subtitle

/**
 * The mini-player under 900 dp (Luminous): a floating Veil pill with round artwork, two lines of
 * type ("Playing on {Device}" for a remote session, its error in a quieter third line) and one
 * control, play or pause (Misty CMP-02, never accent-filled, COL-04), and a straight 2 dp progress
 * line along its bottom edge, inset by its round ends, redrawn only when it grows by a pixel.
 * Tapping or swiping it up opens Now Playing.
 */
@Composable
fun MiniPlayer(state: CoreState, onOpen: () -> Unit, modifier: Modifier = Modifier, floating: Boolean = false) {
    val track = PlayerModel.track(state) ?: return
    val remote = RemoteModel.session(state)
    val graph = LocalGraph.current
    val colors = Jf.colors
    val time = LocalPlaybackTime.current
    val status = PlayerModel.status(state)
    val playing = PlayerModel.isPlaying(status)
    val durationUs = PlayerModel.durationUs(state)
    val density = LocalDensity.current
    var steps by remember { androidx.compose.runtime.mutableIntStateOf(300) }
    TimeDemandEffect(if (playing) TimeDemand.Steps(steps) else null)
    val fraction = {
        val d = durationUs
        if (d == null || d <= 0) 0f else (time.positionUs.toFloat() / d).coerceIn(0f, 1f)
    }
    val openLabel = stringResource(R.string.a11y_open_now_playing)
    val subtitle = playerSecondLine(track, remote)
    val percent by remember(durationUs) {
        derivedStateOf {
            val d = durationUs
            if (d == null || d <= 0) 0 else (time.positionUs * 100 / d).toInt().coerceIn(0, 100)
        }
    }
    val buttonLabel = stringResource(
        R.string.mini_play_progress,
        stringResource(if (playing) R.string.a11y_pause else R.string.a11y_play),
        percent,
    )
    val height = if (floating) FloatingPlayerHeight else MiniPlayerHeight
    val shape = rounded(height / 2)
    GlassSurface(
        modifier
            .fillMaxWidth()
            .height(height)
            .onSizeChanged { steps = SeekGeometry.miniSteps(it.width.toFloat(), density.density) }
            .pointerInput(Unit) {
                detectVerticalDragGestures { _, dragAmount -> if (dragAmount < -12f) onOpen() }
            },
        grade = GlassGrade.Veil,
        shape = shape,
        floating = true,
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .height(height)
                .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button, onClick = onOpen)
                .semantics(mergeDescendants = false) {
                    contentDescription = openLabel
                    onClick(openLabel) {
                        onOpen()
                        true
                    }
                }
                .padding(start = 10.dp, end = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            ArtworkImage(ArtKey.of(track.artwork), track.title, if (floating) 36.dp else 44.dp, CircleShape)
            Column(Modifier.weight(1f)) {
                JfText(track.title, style = JfType.listTitle.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), maxLines = 1)
                if (subtitle != null && !floating) JfText(subtitle, style = JfType.listSecondary, color = colors.ink70, maxLines = 1)
                remote?.error?.let { JfText(it, style = JfType.listSecondary.copy(fontSize = 11.sp), color = colors.ink70, maxLines = 1) }
            }
            MiniPlayButton(playing, buttonLabel) { graph.send(PlayerModel.playPause(status)) }
        }
        MiniProgressLine(fraction, Modifier.align(Alignment.BottomCenter).padding(bottom = 1.dp), inset = height / 2 + 2.dp)
    }
}

/** The mini-player's one control: play or pause, morphing, on a quiet round fill (never the accent, COL-04). */
@Composable
private fun MiniPlayButton(playing: Boolean, description: String, onClick: () -> Unit) {
    val colors = Jf.colors
    Box(
        Modifier
            .size(TouchTarget)
            .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(44.dp).background(colors[Tokens.Line.fillHover], CircleShape), contentAlignment = Alignment.Center) {
            PlayPauseGlyph(playing, colors.ink90, 20.dp)
        }
    }
}

/**
 * The player bar from 900 dp (tablets, desktop mode; docs/design-controls.md, "v0.0.4"), after
 * Media Player and Apple Music: a floating Veil bar 80 dp high with r-xl corners. Start: the
 * artwork and two lines (they open Now Playing); middle: the transport capsule (shuffle,
 * previous, play, next, repeat; play is the only accent fill) above the seek bar with its times;
 * end: Lyrics, Queue and the volume. It bends CMP-02 on purpose; phones keep the capsule.
 */
@Composable
fun PlayerBar(state: CoreState, onOpen: (NowPlayingPanel?) -> Unit, modifier: Modifier = Modifier) {
    val track = PlayerModel.track(state) ?: return
    val remote = RemoteModel.session(state)
    val graph = LocalGraph.current
    val colors = Jf.colors
    val status = PlayerModel.status(state)
    val playing = PlayerModel.isPlaying(status)
    val shuffle = PlayerModel.shuffle(state)
    val repeat = PlayerModel.repeat(state)
    val openLabel = stringResource(R.string.a11y_open_now_playing)
    GlassSurface(
        modifier.fillMaxWidth().height(PlayerBarHeight),
        grade = GlassGrade.Veil,
        shape = rounded(Tokens.Radius.r2xl.dp),
        floating = true,
    ) {
        Row(
            Modifier.fillMaxWidth().height(PlayerBarHeight).padding(start = 10.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(
                Modifier
                    .weight(1f)
                    .clickable(remember { MutableInteractionSource() }, indication = null, role = Role.Button) { onOpen(null) }
                    .semantics(mergeDescendants = true) { contentDescription = openLabel },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                ArtworkImage(ArtKey.of(track.artwork), track.title, 48.dp, rounded(Tokens.Radius.rMd.dp))
                Column(Modifier.weight(1f)) {
                    JfText(track.title, style = JfType.listTitle.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), maxLines = 1)
                    playerSecondLine(track, remote)?.let { JfText(it, style = JfType.listSecondary, color = colors.ink70, maxLines = 1) }
                    remote?.error?.let { JfText(it, style = JfType.listSecondary.copy(fontSize = 11.sp), color = colors.ink70, maxLines = 1) }
                }
            }
            Column(Modifier.weight(2f).widthIn(max = 560.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    JfToggleIconButton(
                        R.drawable.ic_shuffle,
                        stringResource(R.string.a11y_shuffle),
                        stringResource(if (shuffle) R.string.state_on else R.string.state_off),
                        shuffle,
                        { graph.send(Command.SetShuffle(it)) },
                        size = 36.dp,
                    )
                    JfIconButton(R.drawable.ic_previous, stringResource(R.string.a11y_previous), { graph.send(Command.Previous) }, tint = colors.ink90, size = 36.dp, iconSize = 20.dp)
                    PlayPauseButton(playing, stringResource(if (playing) R.string.a11y_pause else R.string.a11y_play), { graph.send(PlayerModel.playPause(status)) }, size = 40.dp)
                    JfIconButton(R.drawable.ic_next, stringResource(R.string.a11y_next), { graph.send(Command.Next) }, tint = colors.ink90, size = 36.dp, iconSize = 20.dp)
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
                        size = 36.dp,
                    )
                }
                SeekWithTimes(state, Modifier.height(28.dp))
            }
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                JfIconButton(R.drawable.ic_lyrics, stringResource(R.string.np_lyrics), { onOpen(NowPlayingPanel.Lyrics) }, size = 40.dp, iconSize = 20.dp)
                JfIconButton(R.drawable.ic_queue, stringResource(R.string.np_queue), { onOpen(NowPlayingPanel.Queue) }, size = 40.dp, iconSize = 20.dp)
                DeviceButton(state, size = 40.dp)
                PlayerVolume(state, Modifier.widthIn(max = 200.dp))
            }
        }
    }
}
