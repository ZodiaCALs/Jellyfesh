package org.jellyfesh.android.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.flow.SharedFlow
import org.jellyfesh.android.AppGraph
import org.jellyfesh.android.CoreStartup
import org.jellyfesh.android.R
import org.jellyfesh.android.system.DeviceEffects
import org.jellyfesh.android.ui.components.EmptyState
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.OverlayHost
import org.jellyfesh.android.ui.components.Overlays
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.home.HomeScreen
import org.jellyfesh.android.ui.library.AlbumScreen
import org.jellyfesh.android.ui.library.ArtistScreen
import org.jellyfesh.android.ui.library.LibraryScreen
import org.jellyfesh.android.ui.library.PlaylistScreen
import org.jellyfesh.android.ui.nav.BottomChrome
import org.jellyfesh.android.ui.nav.LocalNav
import org.jellyfesh.android.ui.nav.NavController
import org.jellyfesh.android.ui.nav.chromeHeight
import org.jellyfesh.android.ui.player.NowPlayingPanel
import org.jellyfesh.android.ui.player.NowPlayingScreen
import org.jellyfesh.android.ui.player.lyricsBarHeight
import org.jellyfesh.android.ui.player.lyricsBarTranslates
import org.jellyfesh.android.ui.search.SearchScreen
import org.jellyfesh.android.ui.settings.SettingsScreen
import org.jellyfesh.android.ui.signals.SignalMessages
import org.jellyfesh.android.ui.sources.SourceBrowseScreen
import org.jellyfesh.android.ui.sources.SourceScreen
import org.jellyfesh.android.ui.sources.SourcesScreen
import org.jellyfesh.android.ui.theme.GlassBackdrop
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfEffects
import org.jellyfesh.android.ui.theme.JfMotion
import org.jellyfesh.android.ui.theme.JfTheme
import org.jellyfesh.android.ui.theme.LocalPlaybackRunning
import org.jellyfesh.android.ui.theme.MistyField
import org.jellyfesh.android.ui.theme.fadeOutAbove
import org.jellyfesh.android.ui.time.LocalPlaybackTime
import org.jellyfesh.android.ui.time.rememberPlaybackTime
import org.jellyfesh.logic.lyrics.LyricsBars
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.theme.AppearanceRule
import org.jellyfesh.tokens.Tokens

/** The UI: the theme from the core's slices, the screens, Now Playing and the overlays. */
@Composable
fun AppRoot(graph: AppGraph, authRedirects: SharedFlow<String>) {
    val state by graph.store.state.collectAsStateWithLifecycle()
    val startup by graph.startup.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val dark = AppearanceRule.isDark(state.settings.appearance, isSystemInDarkTheme())
    // The system's "Remove animations", its blur and contrast settings and battery saver change
    // without telling the app: read them again whenever it comes back to the foreground.
    var systemReducesMotion by remember { mutableStateOf(DeviceEffects.systemReducesMotion(context)) }
    var systemChecks by remember { mutableIntStateOf(0) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            systemReducesMotion = DeviceEffects.systemReducesMotion(context)
            systemChecks++
        }
    }
    val reduceEffects = state.settings.reduceEffects
    val transparency = state.settings.transparency
    val effects = remember(reduceEffects, systemReducesMotion, transparency, systemChecks) {
        JfEffects(
            glassTier = DeviceEffects.glassTier(context, reduceEffects),
            reduceMotion = systemReducesMotion || reduceEffects,
            systemReducesMotion = systemReducesMotion,
            transparency = transparency,
        )
    }
    SystemBars(dark)
    val overlays = remember { Overlays() }
    CompositionLocalProvider(
        LocalGraph provides graph,
        LocalOverlays provides overlays,
        LocalAuthRedirects provides authRedirects,
    ) {
        JfTheme(theme = state.theme, dark = dark, effects = effects) {
            val time = rememberPlaybackTime(graph)
            CompositionLocalProvider(LocalPlaybackTime provides time) {
                when (val s = startup) {
                    CoreStartup.Starting -> Box(Modifier.fillMaxSize()) { MistyField(Modifier.fillMaxSize()) }
                    is CoreStartup.Failed -> StartupFailed(s.message)
                    CoreStartup.Ready -> MainUi(graph, state, overlays)
                }
            }
        }
    }
}

@Composable
private fun StartupFailed(message: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        MistyField(Modifier.fillMaxSize())
        EmptyState(
            icon = R.drawable.ic_warning,
            title = stringResource(R.string.startup_failed_title),
            body = stringResource(R.string.startup_failed_body, message),
        )
    }
}

@Composable
private fun MainUi(graph: AppGraph, state: CoreState, overlays: Overlays) {
    val nav = remember { NavController(graph.nav) }
    val navState by graph.nav.collectAsStateWithLifecycle()
    val reduce = Jf.effects.reduceMotion
    val hasPlayer = PlayerModel.track(state) != null
    // Show Lyrics Bar: the line being sung above the player, while the song playing here has timed lyrics.
    val lyricsBar = hasPlayer && LyricsBars.shows(state.settings.showLyricsBar, state.currentLyrics, PlayerModel.isRemote(state))
    var requestedPanel by remember { mutableStateOf<NowPlayingPanel?>(null) }
    val navigationInset = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    SignalMessages(graph, overlays)
    val rise = with(androidx.compose.ui.platform.LocalDensity.current) { 8.dp.roundToPx() }
    CompositionLocalProvider(LocalNav provides nav, LocalPlaybackRunning provides (PlayerModel.isPlaying(state.playback.status) && !PlayerModel.isRemote(state))) {
        BackHandler(enabled = navState.canGoBack && !overlays.isOpen) { nav.back() }
        val saveable = rememberSaveableStateHolder()
        BoxWithConstraints(Modifier.fillMaxSize()) {
            // From 900 dp (tablets, desktop mode) the player bar replaces the mini-player.
            val wide = SeekGeometry.usesPlayerBar(maxWidth.value)
            val windowWidth = maxWidth.value
            // The floating chrome above the system navigation bar: the tab bar capsule, and the
            // mini-player pill (or the player bar) when something plays.
            val chromeHeight: Dp = chromeHeight(hasPlayer, wide, lyricsBarHeight(lyricsBarTranslates(state)).takeIf { lyricsBar })
            // Library screens and their chrome. Hidden from accessibility while a layer covers them.
            val covered = navState.nowPlayingOpen || overlays.isOpen
            GlassBackdrop(
                Modifier.fillMaxSize().then(if (covered) Modifier.clearAndSetSemantics {} else Modifier),
                invalidateKey = state.theme,
            ) {
                MistyField(Modifier.fillMaxSize())
                CompositionLocalProvider(
                    LocalChromePadding provides PaddingValues(bottom = chromeHeight + navigationInset),
                    LocalWindowWidthDp provides windowWidth,
                ) {
                    val screen = navState.screen
                    // Pages fade in and rise 8 dp (dur-2 Fluid); going back fades (dur-1); under reduced
                    // motion both are a short fade.
                    AnimatedContent(
                        targetState = Triple(navState.tab, screen, navState.stack.size),
                        transitionSpec = {
                            val back = targetState.first == initialState.first && targetState.third < initialState.third
                            when {
                                reduce -> fadeIn(JfMotion.fade(Tokens.Motion.dur1Ms, true)) togetherWith fadeOut(JfMotion.fadeOut(Tokens.Motion.dur1Ms, true))
                                back -> fadeIn(JfMotion.fluid(Tokens.Motion.dur1Ms, false)) togetherWith fadeOut(JfMotion.exit(Tokens.Motion.dur1Ms, false))
                                else -> (fadeIn(JfMotion.fluid(Tokens.Motion.dur2Ms, false)) + slideIn(JfMotion.fluid(Tokens.Motion.dur2Ms, false)) { IntOffset(0, rise) }) togetherWith
                                    fadeOut(JfMotion.exit(Tokens.Motion.dur1Ms, false))
                            }
                        },
                        label = "screen",
                        // The content fades out just above the floating chrome (Luminous).
                        modifier = Modifier.fadeOutAbove(chromeHeight + navigationInset),
                    ) { (tab, target, _) ->
                        saveable.SaveableStateProvider("${tab.name}/$target") {
                            ScreenContent(target, state)
                        }
                    }
                }
                BottomChrome(
                    state = state,
                    tab = navState.tab,
                    wide = wide,
                    onSelect = nav::select,
                    lyricsBar = lyricsBar,
                    onOpenNowPlaying = { panel ->
                        requestedPanel = panel
                        nav.openNowPlaying()
                    },
                    modifier = Modifier.align(Alignment.BottomCenter),
                )
            }
            AnimatedVisibility(
                visible = navState.nowPlayingOpen,
                enter = slideInVertically(JfMotion.fluid(JfMotion.OPEN_NOW_PLAYING_MS, reduce)) { it / 6 } +
                    fadeIn(JfMotion.fade(JfMotion.OPEN_NOW_PLAYING_MS, reduce)),
                exit = slideOutVertically(JfMotion.exit(JfMotion.CLOSE_NOW_PLAYING_MS, reduce)) { it / 6 } +
                    fadeOut(JfMotion.fadeOut(JfMotion.CLOSE_NOW_PLAYING_MS, reduce)),
            ) {
                Box(Modifier.fillMaxSize().then(if (overlays.isOpen) Modifier.clearAndSetSemantics {} else Modifier)) {
                    CompositionLocalProvider(LocalWindowWidthDp provides windowWidth) {
                        NowPlayingScreen(state, onClose = nav::closeNowPlaying, requestedPanel = requestedPanel)
                    }
                }
            }
            OverlayHost(overlays, bottomInset = if (navState.nowPlayingOpen) 0.dp else chromeHeight)
        }
    }
}

@Composable
private fun ScreenContent(screen: Screen, state: CoreState) {
    when (screen) {
        Screen.Home -> HomeScreen(state)
        is Screen.Library -> LibraryScreen(screen.section, state)
        Screen.Search -> SearchScreen(state)
        Screen.Sources -> SourcesScreen(state)
        is Screen.Album -> AlbumScreen(screen.id, screen.title, state)
        is Screen.Artist -> ArtistScreen(screen.id, screen.name, state)
        is Screen.Playlist -> PlaylistScreen(screen.id, screen.name, state)
        is Screen.Source -> SourceScreen(screen.pluginId, state)
        is Screen.SourceBrowse -> SourceBrowseScreen(screen, state)
        is Screen.Settings -> SettingsScreen(screen.section, state)
    }
}

/** Status and navigation bar glyphs follow the app's appearance, not only the system's. */
@Composable
private fun SystemBars(dark: Boolean) {
    val view = LocalView.current
    SideEffect {
        val window = view.context.findActivity()?.window ?: return@SideEffect
        WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
    }
}

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}
