// Wires the core client to the stores and the platform adapters, and hands every event to the
// store that keeps its slice. Nothing here decides anything about playback or the library: the
// stores show what the core reports and send what the person asks for (ADR-0001).

import AppKit
import Observation
import SwiftUI

/// The sections of the Settings page, in docs/ui.md's order.
enum SettingsTab: Hashable, CaseIterable {
    case appearance, playback, library, downloads, sources, extensions, about

    var title: LocalizedStringKey {
        switch self {
        case .appearance: return "Appearance"
        case .playback: return "Playback"
        case .library: return "Library"
        case .downloads: return "Downloads"
        case .sources: return "Sources"
        case .extensions: return "Extensions"
        case .about: return "About"
        }
    }
}

@MainActor
@Observable
final class AppModel {
    /// The one model of the running app (the window, the Dock menu and the media keys share it).
    static let shared = AppModel.launch()

    let client: CoreClient
    /// Why the core could not start, when it could not.
    let startupError: String?

    let tracker: CommandTracker
    let toasts: ToastCenter
    let router: Router
    let system: SystemState
    let playback: PlaybackStore
    let library: LibraryStore
    let settings: SettingsStore
    let theme: ThemeStore
    let lyrics: LyricsStore
    let queue: QueueStore
    let search: SearchStore
    let sources: SourcesStore
    /// Apple Music through MusicKit, a player the app provides (ADR-0018 item 5).
    let appleMusic: AppleMusicStore
    let pages: LibraryPages
    let artwork: ArtworkLoader
    /// Jellyfesh Pro (ADR-0021), updates, AI Lyrics (ADR-0019) and the look (ADR-0020).
    let pro: ProStore
    let updates: UpdatesStore
    let aiLyrics: AiLyricsStore
    let look: LookStore
    /// Desktop Lyrics' window (ADR-0022 §6).
    let desktopLyrics: DesktopLyricsStore
    /// The mini player's window (ADR-0026 §4).
    let miniPlayer: MiniPlayerStore
    /// The sources' likes and playlists (ADR-0024).
    let sourceLibrary: SourceLibraryStore
    /// Downloads from the person's servers (ADR-0025).
    let downloads: DownloadsStore
    /// Discover, details and comments answers, to whoever asked.
    let answers: CoreAnswers
    /// How this build reached the person (the build flags).
    let distribution: AppDistribution
    /// The section the Settings page shows.
    var settingsTab: SettingsTab = .appearance
    /// The extension Settings › Extensions opens to (Appearance › Customize…), until it has.
    var extensionsFocus: String?

    private let nowPlayingCenter: NowPlayingCenter
    private let dockIcon: DockIcon
    private let keys: SharedKeyMonitor
    /// The network, for the core's Auto quality and downloads (ADR-0026 §1, ADR-0025 §3).
    private let network = NetworkMonitor()
    @ObservationIgnored private var deviceWatcher: DeviceWatcher?
    @ObservationIgnored private var started = false
    /// The app is quitting: its windows closing is not the person closing the main window.
    @ObservationIgnored private var quitting = false
    /// Music folders that moved since the last launch (sandboxed builds), passed to the core once
    /// it has started.
    @ObservationIgnored private var movedFolders: [FolderAccess.Move] = []
    /// The downloads folder's new path, when it moved since the last launch (sandboxed builds).
    @ObservationIgnored private var movedDownloadFolder: String?
    @ObservationIgnored private var appliedAppearance: AppearanceOption?

    init(client: CoreClient, startupError: String? = nil, distribution: AppDistribution = .source) {
        let tracker = CommandTracker()
        let toasts = ToastCenter()
        let playback = PlaybackStore(client: client, tracker: tracker)
        let artwork = ArtworkLoader(client: client)
        self.client = client
        self.startupError = startupError
        self.tracker = tracker
        self.toasts = toasts
        self.playback = playback
        self.artwork = artwork
        router = Router()
        system = SystemState()
        library = LibraryStore(client: client, tracker: tracker, toasts: toasts)
        settings = SettingsStore(client: client, tracker: tracker, toasts: toasts)
        theme = ThemeStore()
        lyrics = LyricsStore()
        queue = QueueStore(client: client)
        search = SearchStore(client: client, tracker: tracker)
        let sources = SourcesStore(client: client, tracker: tracker, toasts: toasts)
        self.sources = sources
        appleMusic = AppleMusicStore(client: client, sources: sources, toasts: toasts, service: { MusicKitService() })
        pages = LibraryPages(client: client)
        pro = ProStore(client: client, tracker: tracker)
        updates = UpdatesStore(client: client, tracker: tracker)
        aiLyrics = AiLyricsStore(client: client, tracker: tracker, toasts: toasts)
        look = LookStore()
        desktopLyrics = DesktopLyricsStore()
        miniPlayer = MiniPlayerStore()
        sourceLibrary = SourceLibraryStore(client: client, tracker: tracker, toasts: toasts)
        downloads = DownloadsStore(client: client, tracker: tracker, toasts: toasts)
        answers = CoreAnswers(client: client, tracker: tracker)
        self.distribution = distribution
        nowPlayingCenter = NowPlayingCenter(playback: playback, artwork: artwork)
        dockIcon = DockIcon()
        keys = SharedKeyMonitor()
    }

    /// Starts the core in the default folders with the plugins bundled in the app and this
    /// build's distribution. When it cannot start, the app runs on an offline client and the
    /// window says why.
    static func launch() -> AppModel {
        let folders = FFICoreClient.defaultDirectories()
        let distribution = AppDistribution.current()
        // The sandboxed build reaches its music folders through bookmarks, opened before the
        // core opens the library.
        let folderAccess = FolderAccess(enabled: distribution.isAppStore)
        let moved = folderAccess.restore()
        // So is the downloads folder the person chose, before the core downloads into it.
        let downloadAccess = FolderAccess(enabled: distribution.isAppStore, key: FolderAccess.downloadsKey)
        let downloadMoves = downloadAccess.restore()
        do {
            let files = FileManager.default
            try files.createDirectory(atPath: folders.data, withIntermediateDirectories: true)
            try files.createDirectory(atPath: folders.cache, withIntermediateDirectories: true)
            let client = try FFICoreClient(dataDirectory: folders.data, cacheDirectory: folders.cache,
                                           bundledPlugins: FFICoreClient.bundledPluginPaths(),
                                           distribution: distribution)
            let model = AppModel(client: client, distribution: distribution)
            model.library.folderAccess = folderAccess
            model.movedFolders = moved
            model.downloads.folderAccess = downloadAccess
            model.movedDownloadFolder = downloadMoves.last?.to
            model.desktopLyrics.attach(DesktopLyricsWindow { [unowned model] in
                AnyView(DesktopLyricsView().stores(model))
            })
            model.miniPlayer.attach(MiniPlayerWindow { [unowned model] in
                AnyView(MiniPlayerView().stores(model))
            })
            return model
        } catch {
            let reason = CoreClientError.describe(error)
            return AppModel(client: OfflineCoreClient(reason: reason), startupError: reason,
                            distribution: distribution)
        }
    }

    /// Subscribes to the core, reads its state and starts the platform adapters. Safe to call
    /// more than once.
    func start() {
        guard !started else { return }
        started = true
        settings.onChange = { [weak self] value in
            self?.applyAppSettings(value)
        }
        let state = client.start { [weak self] events in
            self?.handle(events)
        }
        apply(state)
        guard startupError == nil else { return }
        for move in movedFolders {
            library.addFolder(move.to)
        }
        movedFolders = []
        if let folder = movedDownloadFolder {
            settings.update { $0.downloads.folder = folder }
            movedDownloadFolder = nil
        }
        nowPlayingCenter.start()
        nowPlayingCenter.update()
        system.start()
        Task { [appleMusic] in
            await appleMusic.start()
        }
        if distribution.isAppStore {
            // Pro is sold through the App Store here; the core hears what the person owns.
            Task { [pro] in
                await pro.attach(StoreKitPurchases())
            }
        }
        network.start { [weak self] state in
            self?.client.send(.setNetworkState(online: state.online, metered: state.metered))
        }
        let watcher = DeviceWatcher { [weak self] in
            self?.playback.refreshDevices()
        }
        watcher.start()
        deviceWatcher = watcher
        keys.start(playPause: { [weak self] in
            self?.playback.togglePlayPause()
        }, skip: { [weak self] forward in
            self?.skip(forward: forward) ?? false
        }, back: { [weak self] window in
            self?.goBack(in: window) ?? false
        })
    }

    /// Saves the session and stops the core (the app is quitting).
    func shutdown() {
        quitting = true
        network.stop()
        deviceWatcher?.stop()
        keys.stop()
        client.shutdown()
    }

    /// Files opened from Finder or dropped on the Dock icon: plugins install, audio plays.
    func open(_ urls: [URL]) {
        start()
        var audio: [TrackReference] = []
        for url in urls where url.isFileURL {
            if url.pathExtension.lowercased() == "jfpkg" {
                sources.install(path: url.path)
            } else {
                audio.append(.file(path: url.path))
            }
        }
        if !audio.isEmpty {
            playback.play(tracks: audio)
        }
    }

    // MARK: Events

    private func apply(_ state: CoreState) {
        playback.apply(state.playback)
        playback.apply(queue: state.queue)
        playback.apply(nowPlaying: state.nowPlaying)
        playback.apply(devices: state.devices)
        playback.apply(muted: state.muted)
        queue.apply(state.queue)
        lyrics.apply(state.lyrics)
        theme.apply(state.theme)
        settings.apply(state.settings)
        sources.apply(state.sources)
        // Shown only as events: the state before this launch has no session that just ended.
        playback.apply(remote: state.remote)
        library.apply(scanning: state.scanning)
        library.apply(revision: state.libraryRevision)
        aiLyrics.apply(state.aiLyrics)
        look.apply(state.look)
        pro.apply(state.pro)
        updates.apply(state.update)
        sourceLibrary.apply(state.sourceLibrary)
        downloads.apply(state.downloads)
    }

    func handle(_ events: [CoreEvent]) {
        var mediaChanged = false
        for event in events {
            switch event {
            case .playback(let state):
                playback.apply(state)
                mediaChanged = true
            case .queue(let state):
                playback.apply(queue: state)
                queue.apply(state)
                mediaChanged = true
            case .nowPlaying(let info):
                playback.apply(nowPlaying: info)
                mediaChanged = true
            case .lyrics(let state):
                lyrics.apply(state)
            case .theme(let state):
                theme.apply(state)
            case .scan(let state):
                library.apply(scan: state)
            case .libraryChanged(let revision):
                library.apply(revision: revision)
                queue.libraryChanged()
            case .playlistCreated(let request, let id):
                library.playlistCreated(request: request, id: id)
            case .settings(let value):
                settings.apply(value)
            case .devices(let state):
                playback.apply(devices: state)
            case .sources(let state):
                let signedIn = sources.signedInSources
                sources.apply(state)
                // A picture a source could not give may come once it is signed in (or out).
                if sources.signedInSources != signedIn {
                    artwork.forgetFailures()
                }
            case .remote(let state):
                // A session that ended with an error says so once.
                if let message = playback.apply(remote: state) {
                    toasts.show(message)
                }
                mediaChanged = true
            case .signIn(let update):
                sources.apply(signIn: update)
            case .searchResults(let batch):
                search.apply(batch)
            case .browsePage(let batch):
                sources.apply(browse: batch)
            case .buffering(let uid, let buffering):
                playback.apply(bufferingUID: uid, buffering: buffering)
                mediaChanged = true
            case .commandFailed(let id, let message):
                // Unwatched failures still reach the person, except features still on their way.
                if !tracker.fail(id, message: message), !isNotAvailableMessage(message) {
                    toasts.show(message)
                }
            case .error(let message):
                toasts.show(message)
            case .aiLyrics(let state):
                aiLyrics.apply(state)
            case .look(let state):
                look.apply(state)
            case .pro(let state):
                pro.apply(state)
            case .update(let state):
                updates.apply(state)
            case .discover(let answer):
                answers.apply(discover: answer)
            case .details(let answer):
                answers.apply(details: answer)
            case .comments(let answer):
                answers.apply(comments: answer)
            case .artwork(let fetched):
                artwork.artworkFetched(fetched)
            case .searchPage(let page):
                search.apply(page: page)
            case .sourceLibrary(let state):
                sourceLibrary.apply(state)
            case .sourceLibraryPage(let page):
                sourceLibrary.apply(page: page)
            case .downloads(let state):
                downloads.apply(state)
            }
        }
        if mediaChanged {
            nowPlayingCenter.update()
        }
    }

    // MARK: Settings the app applies

    /// Appearance, the Dock icon, Desktop Lyrics and the mini player are the shell's to apply
    /// (docs/core-api.md, "Settings").
    private func applyAppSettings(_ value: AppSettings) {
        if value.appearance != appliedAppearance {
            appliedAppearance = value.appearance
            switch value.appearance {
            case .system: NSApplication.shared.appearance = nil
            case .light: NSApplication.shared.appearance = NSAppearance(named: .aqua)
            case .dark: NSApplication.shared.appearance = NSAppearance(named: .darkAqua)
            }
        }
        dockIcon.apply(value.appIcon)
        desktopLyrics.apply(enabled: value.desktopLyrics)
        miniPlayer.apply(keepOnTop: value.miniPlayer.keepOnTop,
                         opensWhenWindowCloses: value.miniPlayer.openWhenWindowClosed,
                         showsLyricsLine: value.miniPlayer.showLyricsLine)
    }

    // MARK: Navigation helpers

    /// Shows the Settings page (ADR-0023: a page in the main window), at `tab` when one is given;
    /// Now Playing closes. The menus open the main window first.
    func openSettings(_ tab: SettingsTab? = nil) {
        if let tab { settingsTab = tab }
        router.showNowPlaying(false, reduceMotion: true)
        router.select(.settings)
    }

    /// The main window closed: Open When the Window Is Closed opens the mini player while music
    /// plays.
    func mainWindowClosed() {
        guard !quitting else { return }
        miniPlayer.mainWindowClosed(playing: playback.isPlaying)
    }

    /// Shift-Right and Shift-Left: 5 seconds forward or back, while something plays.
    private func skip(forward: Bool) -> Bool {
        guard playback.hasTrack else { return false }
        playback.seek(byUs: forward ? 5_000_000 : -5_000_000)
        return true
    }

    /// Esc in the main window goes back a page; Now Playing closes on its own Esc.
    private func goBack(in window: NSWindow) -> Bool {
        guard window === system.mainWindow, !router.nowPlayingShown else { return false }
        return router.back()
    }
}
