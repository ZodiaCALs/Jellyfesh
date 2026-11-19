// The main window: the library in a NavigationSplitView with the mini player or the player bar
// floating over its detail column, and Now Playing opening over everything. Every view below gets the theme's
// palette for the window's appearance and the effects the system and the person allow.

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
        ZStack {
            if let error = model.startupError {
                StartupErrorView(message: error)
            } else {
                LibraryWindow()
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
        .onChange(of: model.library.newPlaylist?.id) { _, _ in
            // Every New Playlist… starts with an empty name.
            playlistName = ""
        }
        .frame(minWidth: 900, minHeight: 600)
        .toolbar(router.nowPlayingShown ? .hidden : .visible, for: .windowToolbar)
        .environment(\.palette, palette)
        .environment(\.effects, effects)
        .environment(\.look, LookStyle(state: look.state))
        .tint(palette[Tokens.Accent.accent])
        .onChange(of: effects.reduceMotion || effects.reduceEffects, initial: true) { _, value in
            theme.reduceMotion = value
        }
        .onChange(of: model.library.createdPlaylist) { _, created in
            // A playlist made from this window (New Playlist, Import) opens when the core names it.
            if created != nil, let id = model.library.consumeCreatedPlaylist() {
                router.showNowPlaying(false, reduceMotion: true)
                router.select(.playlist(id))
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
        let transparency = Double(settings.current.transparency)
        return Effects(
            reduceMotion: reduceMotion,
            // Misty MAT-08: Reduce Transparency, Increase Contrast, Low Power Mode and the app's
            // Reduce motion and transparency all turn glass into solid Cloud, as does a
            // Transparency of 0 %.
            solidGlass: reduceTransparency || contrast == .increased || system.lowPower || reduceEffects
                || transparency <= 0.001,
            reduceEffects: reduceEffects,
            windowVisible: system.windowVisible,
            transparency: transparency
        )
    }
}

/// The library: sidebar sections and the pages pushed on them.
struct LibraryWindow: View {
    @State private var columns = NavigationSplitViewVisibility.all

    var body: some View {
        NavigationSplitView(columnVisibility: $columns) {
            Sidebar()
                .navigationSplitViewColumnWidth(min: 190, ideal: 220, max: 320)
        } detail: {
            DetailColumn()
        }
    }
}

/// The selected section, its pushed pages, and the mini player (columns under 900 pt) or the
/// player bar (900 pt and wider) floating over them as Veil capsules; the pages fade out just
/// above the player (Luminous). Under Now Playing the bar stays in place but stops redrawing its
/// progress.
struct DetailColumn: View {
    @Environment(Router.self) private var router
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.effects) private var effects
    @State private var width: CGFloat = 0

    var body: some View {
        @Bindable var router = router
        let wide = SeekGeometry.usesPlayerBar(width: width)
        NavigationStack(path: $router.path) {
            SectionPage(item: router.selection)
                .pageEntrance()
                .id(router.selection)
                .navigationDestination(for: Route.self) { route in
                    RoutePage(route: route)
                        .pageEntrance()
                }
        }
        .mask {
            PlayerFadeMask(playerZone: LuminousGeometry.playerZone(hasPlayer: playback.hasTrack, wide: wide))
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
        .safeAreaInset(edge: .bottom, spacing: 0) {
            ZStack(alignment: .bottom) {
                if wide {
                    PlayerBar()
                } else {
                    MiniPlayer()
                }
            }
            .environment(\.effects, barEffects)
            .animation(Motion.unless(effects.reduceMotion, Motion.move), value: playback.hasTrack)
            .animation(Motion.unless(effects.reduceMotion, Motion.move), value: wide)
        }
        .softScrollEdges()
    }

    /// Now Playing covers the bar: its progress stops redrawing until it closes.
    private var barEffects: Effects {
        var bar = effects
        bar.windowVisible = effects.windowVisible && !router.nowPlayingShown
        return bar
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
        case .search: SearchView()
        case .sources: SourcesView()
        case .playlist(let id): PlaylistDetailView(playlistID: id)
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
        case .appleMusic: AppleMusicView()
        }
    }
}
