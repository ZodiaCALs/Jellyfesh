// The main window, in Jellyfesh Web's layout (ADR-0023): the field behind everything; from 761 pt
// wide the floating sidebar panel beside the pages, up to 760 pt the pages over the phone tabs;
// the player floating over every page, Settings included; Now Playing opening over everything.
// The window frame and its title-bar controls stay the system's (a hidden title bar over the
// field). Every view below gets the theme's palette for the window's appearance and the effects
// the system and the person allow.

import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var model
    @Environment(Router.self) private var router
    @Environment(ThemeStore.self) private var theme
    @Environment(SettingsStore.self) private var settings
    @Environment(SystemState.self) private var system
    @Environment(LookStore.self) private var look
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var contrast
    /// The name typed for a New Playlist….
    @State private var playlistName = ""

    var body: some View {
        let effects = currentEffects
        let palette = theme.palette(for: colorScheme)
        let style = LookStyle(state: look.state)
        ZStack {
            if let error = model.startupError {
                StartupErrorView(message: error)
            } else {
                AppShell()
                    .allowsHitTesting(!router.nowPlayingShown)
                    .accessibilityHidden(router.nowPlayingShown)
                if router.nowPlayingShown {
                    NowPlayingView()
                        .transition(effects.animates ? .deeper : .opacity)
                        .zIndex(1)
                }
            }
        }
        .overlay(alignment: .top) {
            ToastOverlay()
        }
        .background {
            WindowVisibilityReader(onChange: { visible in
                system.windowVisible = visible
            }, onWindow: { window in
                system.mainWindow = window
            })
        }
        .alert("New Playlist", isPresented: Binding(
            get: { model.library.newPlaylist != nil },
            set: { if !$0 { model.library.cancelNewPlaylist() } }
        )) {
            TextField("Name", text: $playlistName)
            Button("Create") {
                model.library.createNewPlaylist(named: playlistName)
            }
            Button("Cancel", role: .cancel) {
                model.library.cancelNewPlaylist()
            }
        }
        .audioConsentQuestion(model.aiLyrics)
        .onChange(of: model.library.newPlaylist?.id) { _, _ in
            // Every New Playlist… starts with an empty name.
            playlistName = ""
        }
        .frame(minWidth: ShellLayout.minimumWidth, minHeight: ShellLayout.minimumHeight)
        .proSheet(in: .main)
        .getInfoSheet()
        .environment(\.palette, palette)
        .environment(\.effects, effects)
        .environment(\.look, style)
        // Text without a face of its own (controls, menus) in the look's typeface.
        .fontDesign(style.customized ? style.fontDesign : nil)
        .tint(palette[Tokens.Accent.accent])
        .onChange(of: effects.reduceMotion || effects.reduceEffects, initial: true) { _, value in
            theme.reduceMotion = value
        }
        .onChange(of: model.library.createdPlaylist) { _, created in
            // A playlist made from this window (New Playlist, Import) opens when the core names it.
            if created != nil, let id = model.library.consumeCreatedPlaylist() {
                router.showNowPlaying(false, reduceMotion: true)
                router.openPlaylist(id)
            }
        }
        .onChange(of: model.playback.hasTrack) { _, hasTrack in
            if !hasTrack && router.nowPlayingShown {
                router.showNowPlaying(false, reduceMotion: !effects.animates)
            }
        }
        .task {
            model.start()
        }
    }

    private var currentEffects: Effects {
        let reduceEffects = settings.current.reduceEffects
        // The look's glass scales the setting's transparency: a look without glass is solid, and
        // the setting and Reduce Transparency still win (ADR-0020).
        let transparency = LookStyle(state: look.state).transparency(setting: Double(settings.current.transparency))
        return Effects(
            reduceMotion: reduceMotion,
            // Misty MAT-08: Reduce Transparency, Increase Contrast, Low Power Mode and the app's
            // Reduce motion and transparency all turn glass into solid Cloud, as does a
            // Transparency of 0 % (or a look without glass).
            solidGlass: reduceTransparency || contrast == .increased || system.lowPower || reduceEffects
                || transparency <= 0.001,
            reduceEffects: reduceEffects,
            windowVisible: system.windowVisible,
            transparency: transparency
        )
    }
}

/// Jellyfesh Web's layout for the window's width (ShellLayout): the floating sidebar panel beside
/// the pages from 761 pt, the pages alone (over the phone tabs) up to 760 pt; the field behind both.
struct AppShell: View {
    /// The window's width, once measured (the default window's until then).
    @State private var width: CGFloat?

    var body: some View {
        let layout = ShellLayout(width: width ?? 1180)
        HStack(spacing: 0) {
            if layout.showsSidebar {
                SidebarPanel()
                    .padding([.leading, .top, .bottom], ShellLayout.margin)
            }
            ContentColumn(layout: layout)
        }
        .background {
            MistyField()
        }
        .background {
            GeometryReader { geometry in
                Color.clear
                    .onAppear { width = geometry.size.width }
                    .onChange(of: geometry.size.width) { _, value in width = value }
            }
        }
    }
}

/// The place shown and the pages pushed on it, with the player floating over them as a Veil
/// capsule: the player bar in windows 900 pt and wider, the capsule in narrower ones (a customized
/// look may ask for the capsule at every width, or a smaller one at the trailing corner, ADR-0020).
/// In the phone layout the tabs float under the player. The pages fade out just above them
/// (Luminous). The player floats over every page, Settings included, and is not drawn while Now
/// Playing is open: Now Playing has its own dock (ADR-0023 §2).
struct ContentColumn: View {
    var layout: ShellLayout

    @Environment(Router.self) private var router
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look

    var body: some View {
        @Bindable var router = router
        let player = LuminousGeometry.bottomPlayer(style: look.miniPlayer, wide: layout.showsPlayerBar)
        let showsPlayer = playback.hasTrack && !router.nowPlayingShown
        let playerHeight: CGFloat? = showsPlayer ? (player == .bar ? SeekGeometry.barHeight : Tokens.Player.miniHeight) : nil
        NavigationStack(path: $router.path) {
            SectionPage(item: router.selection)
                .pageEntrance()
                .id(router.selection)
                .environment(\.showsLibrarySwitcher, layout.showsTabs && router.selection.isLibraryPlace)
                .navigationDestination(for: Route.self) { route in
                    // Back is the page's own, at its top, as on the web.
                    RoutePage(route: route)
                        .pageEntrance()
                        .environment(\.showsBackButton, true)
                        .environment(\.showsLibrarySwitcher, false)
                        .navigationBarBackButtonHidden(true)
                }
        }
        .mask {
            PlayerFadeMask(playerZone: ShellLayout.bottomZone(playerHeight: playerHeight, tabs: layout.showsTabs))
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            VStack(spacing: 0) {
                if showsPlayer {
                    ZStack(alignment: player == .floating ? .bottomTrailing : .bottom) {
                        switch player {
                        case .bar: PlayerBar()
                        case .mini: MiniPlayer()
                        case .floating: MiniPlayer(floating: true)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: player == .floating ? .trailing : .center)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                }
                if layout.showsTabs {
                    PhoneTabBar()
                }
            }
            .animation(Motion.unless(effects.reduceMotion, Motion.move), value: showsPlayer)
            .animation(Motion.unless(effects.reduceMotion, Motion.move), value: player)
        }
        .softScrollEdges()
    }
}

/// The page for a sidebar section.
struct SectionPage: View {
    var item: SidebarItem

    var body: some View {
        switch item {
        case .home: HomeView()
        case .albums: AlbumsView()
        case .artists: ArtistsView()
        case .songs: SongsView()
        case .playlists: PlaylistsView()
        case .favorites: FavoritesView()
        case .downloads: DownloadsView()
        case .search: SearchView()
        case .sources: SourcesView()
        case .settings: SettingsPage()
        }
    }
}

/// A page pushed on a section.
struct RoutePage: View {
    var route: Route

    var body: some View {
        switch route {
        case .album(let album): AlbumDetailView(album: album)
        case .artist(let artist): ArtistDetailView(artist: artist)
        case .playlist(let id): PlaylistDetailView(playlistID: id)
        case .browse(let plugin, let node, let title, let link, let item):
            BrowseView(plugin: plugin, node: node, title: title, link: link, item: item)
        case .discover(let plugin, let title):
            DiscoverView(plugin: plugin, title: title)
        case .appleMusic: AppleMusicView()
        }
    }
}
