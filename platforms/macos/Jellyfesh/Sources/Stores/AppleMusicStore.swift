// Apple Music (ADR-0010, ADR-0018 item 5): offered only to a copy of the app signed so that
// MusicKit can work, turned on only after the person agrees, and shown only once MusicKit
// authorises the app, a developer token can be fetched and the subscription can play the
// catalogue; otherwise hidden entirely. Once on, the app registers its player with the core under
// `apple-music`, and the catalogue's songs and albums play through it as a remote session.
// Isolation (ADR-0010): results stay in memory and in their own group, nothing reaches the
// library, and the core shows no synced lyrics while Apple Music plays.

import Foundation
import Observation

/// A catalogue song as the Apple Music views show it.
struct AppleMusicSong: Identifiable, Hashable, Sendable {
    var id: String
    var title: String
    var artist: String?
    var album: String?
    var durationMs: UInt64?
    var artworkURL: String?
    /// Opens the song in Apple Music.
    var link: String?

    /// What plays it: a source track of the app's own player.
    var track: TrackReference {
        .source(SourceSong(plugin: AppleMusicIDs.source, id: id, title: title, artist: artist, album: album,
                           durationMs: durationMs, artworkURL: artworkURL, isrc: nil))
    }
}

/// A catalogue album as the Apple Music views show it.
struct AppleMusicAlbum: Identifiable, Hashable, Sendable {
    var id: String
    var title: String
    var artist: String?
    var artworkURL: String?
    var link: String?

    /// What plays it: the album as a source container of the app's own player.
    var collection: CollectionReference { .source(plugin: AppleMusicIDs.source, id: id) }
}

struct AppleMusicResults: Hashable, Sendable {
    var songs: [AppleMusicSong]
    var albums: [AppleMusicAlbum]

    static let empty = AppleMusicResults(songs: [], albums: [])
    var isEmpty: Bool { songs.isEmpty && albums.isEmpty }
}

/// What the Apple Music views and checks ask of MusicKit (MusicKitService; a fake in tests).
@MainActor
protocol AppleMusicService: AnyObject {
    /// This copy of the app is signed so that MusicKit can work (read without MusicKit).
    var isSigned: Bool { get }
    /// Asks the person to allow access (the system's dialog, once); true when allowed.
    func requestAuthorization() async -> Bool
    /// Whether access is allowed, without asking.
    func isAuthorized() -> Bool
    /// A developer token can be fetched (the app identifier has the MusicKit App Service).
    func canFetchTokens() async -> Bool
    /// The person's subscription can play the catalogue.
    func canPlayCatalog() async -> Bool
    func search(_ term: String) async throws -> AppleMusicResults
    func makeControls() -> AppleMusicControls
}

/// Whether Apple Music shows, and how far it got.
enum AppleMusicAvailability: Equatable {
    /// Not shown: this copy of the app cannot use MusicKit, or the checks failed.
    case hidden
    /// Can be turned on (Sources › Add a Source); MusicKit has not been called.
    case offered
    /// Asking the person, then checking access, tokens and the subscription.
    case checking
    /// On: the player is registered and the catalogue can be searched.
    case available
}

/// One search of the catalogue, as a page shows it.
@MainActor
@Observable
final class AppleMusicSearch {
    private(set) var query = ""
    private(set) var results: AppleMusicResults = .empty
    private(set) var isSearching = false
    private(set) var error: String?
    @ObservationIgnored private var generation = 0

    /// Searches the catalogue for `term` with `service` (nil while Apple Music is off); a newer
    /// search supersedes this one.
    func run(_ term: String, using service: AppleMusicService?) async {
        let words = term.trimmingCharacters(in: .whitespacesAndNewlines)
        generation += 1
        let mine = generation
        query = words
        error = nil
        guard !words.isEmpty, let service else {
            results = .empty
            isSearching = false
            return
        }
        isSearching = true
        do {
            let found = try await service.search(words)
            guard mine == generation else { return }
            results = found
        } catch {
            guard mine == generation else { return }
            results = .empty
            self.error = AppleMusicText.failure(error)
        }
        isSearching = false
    }

    func clear() {
        generation += 1
        query = ""
        results = .empty
        error = nil
        isSearching = false
    }
}

@MainActor
@Observable
final class AppleMusicStore {
    /// The name the source and its player show (a product name: never translated).
    static let name = "Apple Music"
    /// UserDefaults keys: the person turned Apple Music on; the build whose token check failed.
    static let turnedOnKey = "AppleMusicTurnedOn"
    static let unavailableBuildKey = "AppleMusicUnavailableBuild"

    private let client: CoreClient
    private let sources: SourcesStore
    private let toasts: ToastCenter
    private let defaults: UserDefaults
    private let build: String
    private let deviceName: String
    /// Makes the MusicKit service on first use (MusicKitService; a fake in tests).
    private let makeService: @MainActor () -> AppleMusicService?
    @ObservationIgnored private var service: AppleMusicService?
    @ObservationIgnored private var started = false

    private(set) var availability: AppleMusicAvailability = .hidden
    /// The Apple Music page's search, and Apple Music's own group on the Search page.
    let pageSearch: AppleMusicSearch
    let unifiedSearch: AppleMusicSearch

    init(client: CoreClient, sources: SourcesStore, toasts: ToastCenter, defaults: UserDefaults = .standard,
         build: String = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "",
         deviceName: String = String(localized: "This Mac"),
         service: @escaping @MainActor () -> AppleMusicService?) {
        self.client = client
        self.sources = sources
        self.toasts = toasts
        self.defaults = defaults
        self.build = build
        self.deviceName = deviceName
        makeService = service
        pageSearch = AppleMusicSearch()
        unifiedSearch = AppleMusicSearch()
    }

    var isAvailable: Bool { availability == .available }

    /// The catalogue, while Apple Music is on.
    var catalog: AppleMusicService? { isAvailable ? service : nil }

    /// Decides whether Apple Music is offered; checks again, without asking, when the person had
    /// turned it on. Call once at launch.
    func start() async {
        guard !started else { return }
        started = true
        guard let service = loadService(), service.isSigned,
              defaults.string(forKey: Self.unavailableBuildKey) != build
        else {
            availability = .hidden
            return
        }
        guard defaults.bool(forKey: Self.turnedOnKey) else {
            availability = .offered
            return
        }
        availability = .checking
        guard service.isAuthorized() else {
            // Access was taken back in System Settings: offer it again.
            defaults.set(false, forKey: Self.turnedOnKey)
            availability = .offered
            return
        }
        await finishChecks(service, announce: false)
    }

    /// Turns Apple Music on, after the person agreed (the view asks first): asks macOS for access,
    /// then checks the tokens and the subscription. Any failure hides Apple Music and says why once.
    func turnOn() async {
        guard availability == .offered, let service = loadService() else { return }
        availability = .checking
        guard await service.requestAuthorization() else {
            availability = .hidden
            toasts.show(String(localized: "Jellyfesh cannot use Apple Music without access. You can allow it in System Settings › Privacy & Security › Media & Apple Music."))
            return
        }
        await finishChecks(service, announce: true)
    }

    /// Turns Apple Music off: its player leaves the core and the section goes back to Add a Source.
    func turnOff() {
        guard availability == .available else { return }
        defaults.set(false, forKey: Self.turnedOnKey)
        client.unregisterExternalPlayer(id: AppleMusicIDs.source)
        sources.setExternalSource(AppleMusicIDs.source, name: nil)
        pageSearch.clear()
        unifiedSearch.clear()
        availability = .offered
    }

    private func finishChecks(_ service: AppleMusicService, announce: Bool) async {
        guard await service.canFetchTokens() else {
            // The app identifier lacks the MusicKit App Service (an ad-hoc or unprovisioned
            // build): this build never can, so it is not offered again.
            defaults.set(build, forKey: Self.unavailableBuildKey)
            defaults.set(false, forKey: Self.turnedOnKey)
            availability = .hidden
            if announce {
                toasts.show(String(localized: "Apple Music is not available in this copy of Jellyfesh."))
            }
            return
        }
        guard await service.canPlayCatalog() else {
            availability = .hidden
            if announce {
                toasts.show(String(localized: "Apple Music needs a subscription that can play its catalogue."))
            }
            return
        }
        do {
            try client.registerExternalPlayer(id: AppleMusicIDs.source, name: Self.name,
                                              player: AppleMusicPlayer(controls: service.makeControls(),
                                                                       deviceName: deviceName))
        } catch {
            availability = .hidden
            if announce { toasts.show(CoreClientError.describe(error)) }
            return
        }
        sources.setExternalSource(AppleMusicIDs.source, name: Self.name)
        defaults.set(true, forKey: Self.turnedOnKey)
        availability = .available
    }

    private func loadService() -> AppleMusicService? {
        if let service { return service }
        service = makeService()
        return service
    }
}
