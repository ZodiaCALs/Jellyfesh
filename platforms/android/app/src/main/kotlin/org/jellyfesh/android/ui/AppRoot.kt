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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.windowInsetsPadding
import org.jellyfesh.android.ui.nav.SidebarPanel
import org.jellyfesh.android.ui.nav.SidebarWidth
import org.jellyfesh.logic.nav.Layout
import org.jellyfesh.logic.nav.Sidebar
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
import org.jellyfesh.android.billing.StoreNotice
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
import org.jellyfesh.android.ui.player.NowPlayingScreen
import org.jellyfesh.android.ui.player.PipPlayer
import org.jellyfesh.android.ui.player.lyricsBarHeight
import org.jellyfesh.android.ui.player.lyricsBarTranslates
import org.jellyfesh.android.ui.search.SearchScreen
import org.jellyfesh.android.ui.settings.ExtensionScreen
import org.jellyfesh.android.ui.settings.SettingsScreen
import org.jellyfesh.android.ui.signals.SignalMessages
import org.jellyfesh.android.ui.sources.SourceBrowseScreen
import org.jellyfesh.android.ui.sources.DiscoverScreen
import org.jellyfesh.android.ui.sources.ProvideLocalNetwork
import org.jellyfesh.android.ui.library.ProvideDownloadGate
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
import org.jellyfesh.logic.player.MiniPlayerKind
import org.jellyfesh.logic.player.NowPlayingLayouts
import org.jellyfesh.logic.player.NowPlayingPanel
import org.jellyfesh.logic.lyrics.LyricsBars
import org.jellyfesh.logic.nav.Screen
import org.jellyfesh.logic.player.PlayerModel
import org.jellyfesh.logic.seek.SeekGeometry
import org.jellyfesh.logic.state.CoreState
import org.jellyfesh.logic.theme.LookStyle
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
    val look = remember(state.look) { LookStyle(state.look) }
    // The look's glass scales the Transparency setting, which (with Reduce Transparency) still wins.
    val transparency = look.transparency(state.settings.transparency)
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
        JfTheme(theme = state.theme, dark = dark, effects = effects, look = look) {
            val time = rememberPlaybackTime(graph)
            CompositionLocalProvider(LocalPlaybackTime provides time) {
                val pictureInPicture by graph.pictureInPicture.collectAsStateWithLifecycle()
                when (val s = startup) {
                    CoreStartup.Starting -> Box(Modifier.fillMaxSize()) { MistyField(Modifier.fillMaxSize()) }
                    is CoreStartup.Failed -> StartupFailed(s.message)
                    // The mini player in picture-in-picture: the window shows nothing else.
                    CoreStartup.Ready -> if (pictureInPicture) PipPlayer(state) else MainUi(graph, state, overlays)
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
    // What the store says about a purchase the person asked for (Google Play builds).
    val context = LocalContext.current
    LaunchedEffect(graph.purchases) {
        graph.purchases?.notices?.collect { notice ->
            overlays.toast(
                context.getString(
                    when (notice) {
                        StoreNotice.Pending -> R.string.pro_pending
                        StoreNotice.NothingToRestore -> R.string.pro_nothing_to_restore
                        StoreNotice.Failed -> R.string.pro_store_failed
                    },
                ),
            )
        }
    }
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
        // Android 17: Local Network access before connecting to a source on the home network;
        // Android 8 and 9: storage access before the first download (ADR-0025 §2).
        ProvideLocalNetwork(state.sources.plugins) { ProvideDownloadGate {
            BoxWithConstraints(Modifier.fillMaxSize()) {
                val windowWidth = maxWidth.value
                // ADR-0023: the sidebar panel from 761 dp, the phone layout's tab bar below that.
                val sidebar = Layout.usesSidebar(windowWidth)
                // From 900 dp (tablets, desktop mode) the full player bar replaces the capsule.
                val wide = SeekGeometry.usesPlayerBar(windowWidth)
                // The look's player style (ADR-0020): v0.0.6's Bar unless an appearance plugin chose another.
                val playerKind = NowPlayingLayouts.miniPlayer(Jf.look.miniPlayer, wide)
                // The bar floats over every page, Settings included, and hides only under Now Playing.
                val showsPlayer = hasPlayer && !navState.nowPlayingOpen
                val chromeHeight: Dp = chromeHeight(
                    tabs = !sidebar,
                    player = playerKind.takeIf { showsPlayer },
                    // The strip above the capsule; the full bar shows the line under the title.
                    lyricsBar = lyricsBarHeight(lyricsBarTranslates(state)).takeIf { lyricsBar && showsPlayer && playerKind != MiniPlayerKind.Bar },
                )
                // Library screens and their chrome. Hidden from accessibility while a layer covers them.
                val covered = navState.nowPlayingOpen || overlays.isOpen
                GlassBackdrop(
                    Modifier.fillMaxSize().then(if (covered) Modifier.clearAndSetSemantics {} else Modifier),
                    invalidateKey = state.theme,
                ) {
                    MistyField(Modifier.fillMaxSize())
                    Row(Modifier.fillMaxSize()) {
                        if (sidebar) {
                            SidebarPanel(
                                selected = Sidebar.selected(navState),
                                appIcon = state.settings.appIcon,
                                onSelect = nav::select,
                                modifier = Modifier
                                    .width(SidebarWidth)
                                    .fillMaxHeight()
                                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Start + WindowInsetsSides.Vertical)),
                            )
                        }
                        Box(Modifier.weight(1f).fillMaxHeight()) {
                            CompositionLocalProvider(
                                LocalChromePadding provides PaddingValues(bottom = chromeHeight + navigationInset),
                                LocalWindowWidthDp provides windowWidth,
                            ) {
                                // Pages fade in and rise 8 dp (dur-2 Fluid); going back fades (dur-1); under
                                // reduced motion both are a short fade. Settings' sections change in place.
                                AnimatedContent(
                                    targetState = Triple(navState.tab, navState.screen, navState.stack.size),
                                    contentKey = { (tab, screen, depth) -> Triple(tab, if (screen is Screen.Settings) SETTINGS_PAGE else screen, depth) },
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
                                    // The content fades out just above the floating chrome (Luminous), and
                                    // beside the sidebar its sides follow the web's fluid gutter.
                                    modifier = Modifier
                                        .fadeOutAbove(chromeHeight + navigationInset)
                                        .then(if (sidebar) Modifier.padding(horizontal = pageInset(windowWidth), vertical = 0.dp).padding(top = Tokens.Space.s4.dp) else Modifier),
                                ) { (tab, target, _) ->
                                    saveable.SaveableStateProvider("${tab.name}/$target") {
                                        ScreenContent(target, state)
                                    }
                                }
                            }
                            BottomChrome(
                                state = state,
                                tab = navState.tab,
                                kind = playerKind,
                                tabs = !sidebar,
                                player = showsPlayer,
                                onSelect = nav::select,
                                lyricsBar = lyricsBar,
                                sides = if (sidebar) gutter(windowWidth) else null,
                                onOpenNowPlaying = { panel ->
                                    requestedPanel = panel
                                    nav.openNowPlaying()
                                },
                                modifier = Modifier.align(Alignment.BottomCenter),
                            )
                        }
                    }
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
    }
}

/** Settings' page, whatever section it shows: changing sections is no page change. */
private object SETTINGS_PAGE

/** Jellyfesh Web's fluid gutter beside the sidebar: 4 % of the window, from 18 to 32 dp. */
private fun gutter(windowWidthDp: Float): Dp =
    (windowWidthDp * (Tokens.Layout.gutterFluid.vw ?: 4f) / 100f).coerceIn(Tokens.Layout.gutterFluid.min, Tokens.Layout.gutterFluid.max).dp

/** What the pages add to their own 16 dp sides so they sit on the gutter beside the sidebar. */
private fun pageInset(windowWidthDp: Float): Dp = (gutter(windowWidthDp) - 16.dp).coerceAtLeast(0.dp)

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
        is Screen.Discover -> DiscoverScreen(screen.pluginId, state)
        is Screen.SourceBrowse -> SourceBrowseScreen(screen, state)
        is Screen.Settings -> SettingsScreen(screen.section, state)
        is Screen.Extension -> ExtensionScreen(screen.pluginId, state)
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

/** The activity a view's context belongs to (the window's, and what Play Billing launches over). */
internal tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}
