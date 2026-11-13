// Wires the core client to the stores and the platform adapters, and hands every event to the
// store that keeps its slice. Nothing here decides anything about playback or the library: the
// stores show what the core reports and send what the person asks for (ADR-0001).

import AppKit
import Observation
import SwiftUI

/// The tabs of the Settings window.
enum SettingsTab: Hashable {
    case appearance, playback, library, sources
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
    let pages: LibraryPages
    let artwork: ArtworkLoader
    /// The Settings tab to show when the window opens.
    var settingsTab: SettingsTab = .appearance

    private let nowPlayingCenter: NowPlayingCenter
    private let dockIcon: DockIcon
    private let spaceKey: SpaceKeyMonitor
    @ObservationIgnored private var deviceWatcher: DeviceWatcher?
    @ObservationIgnored private var started = false
    @ObservationIgnored private var appliedAppearance: AppearanceOption?

    init(client: CoreClient, startupError: String? = nil) {
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
        sources = SourcesStore(client: client, tracker: tracker, toasts: toasts)
        pages = LibraryPages(client: client)
        nowPlayingCenter = NowPlayingCenter(playback: playback, artwork: artwork)
        dockIcon = DockIcon()
        spaceKey = SpaceKeyMonitor()
    }

    /// Starts the core in the default folders with the plugins bundled in the app. When it
    /// cannot start, the app runs on an offline client and the window says why.
    static func launch() -> AppModel {
        let folders = FFICoreClient.defaultDirectories()
        do {
            let files = FileManager.default
            try files.createDirectory(atPath: folders.data, withIntermediateDirectories: true)
            try files.createDirectory(atPath: folders.cache, withIntermediateDirectories: true)
            let client = try FFICoreClient(dataDirectory: folders.data, cacheDirectory: folders.cache,
                                           bundledPlugins: FFICoreClient.bundledPluginPaths())
            return AppModel(client: client)
        } catch {
            let reason = CoreClientError.describe(error)
            return AppModel(client: OfflineCoreClient(reason: reason), startupError: reason)
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
        nowPlayingCenter.start()
        nowPlayingCenter.update()
        system.start()
        let watcher = DeviceWatcher { [weak self] in
            self?.playback.refreshDevices()
        }
        watcher.start()
        deviceWatcher = watcher
        spaceKey.start { [weak self] in
            self?.playback.togglePlayPause()
        }
    }

    /// Saves the session and stops the core (the app is quitting).
    func shutdown() {
        deviceWatcher?.stop()
        spaceKey.stop()
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
        library.apply(scanning: state.scanning)
        library.apply(revision: state.libraryRevision)
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
                sources.apply(state)
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
            }
        }
        if mediaChanged {
            nowPlayingCenter.update()
        }
    }

    // MARK: Settings the app applies

    /// Appearance and the Dock icon are the shell's to apply (docs/core-api.md, "Settings").
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
    }

    // MARK: Navigation helpers

    /// Opens Settings at `tab`.
    func showSettings(_ tab: SettingsTab, using openSettings: OpenSettingsAction) {
        settingsTab = tab
        openSettings()
    }
}
