// Apple Music (ADR-0010, ADR-0018 item 5) without MusicKit: its ids, what the core's calls ask
// MusicKit to play, MusicKit's state as the core's report, the player answering on the core's
// thread, and when Apple Music is offered, turned on and shown.

import XCTest
@testable import Jellyfesh

final class AppleMusicMappingTests: XCTestCase {
    private func media(_ id: String, kind: ResultKind = .track) -> ExternalMedia {
        ExternalMedia(kind: kind, id: id, title: id, artist: nil, album: nil, durationMs: nil, artworkURL: nil)
    }

    func testCatalogueIdsNameAlbumsAndPlaylists() {
        XCTAssertEqual(AppleMusicContainer(id: "1440857781"), .album("1440857781"))
        XCTAssertEqual(AppleMusicContainer(id: "pl.u-8aAVZAGIo2bjXb"), .playlist("pl.u-8aAVZAGIo2bjXb"))
        XCTAssertNil(AppleMusicContainer(id: "l.abc"), "a library id")
        XCTAssertNil(AppleMusicContainer(id: "pl."))
        XCTAssertNil(AppleMusicContainer(id: ""))
        XCTAssertNil(AppleMusicContainer(id: "12a"))
        XCTAssertNil(AppleMusicContainer(id: "１２"), "full-width digits are no catalogue id")
    }

    func testTheCoresCallsBecomeRequests() {
        // The core sends what was chosen as an album; the id says what it is.
        XCTAssertEqual(AppleMusicPlayRequest.from(context: media("pl.x", kind: .album), tracks: [], start: 2),
                       .playlist(id: "pl.x", start: 2))
        XCTAssertEqual(AppleMusicPlayRequest.from(context: media("123", kind: .album), tracks: [], start: -1),
                       .album(id: "123", start: 0))
        XCTAssertNil(AppleMusicPlayRequest.from(context: media("l.x", kind: .album), tracks: [], start: 0))
        // Tracks that are not the catalogue's are left out; the start stays on the track chosen.
        XCTAssertEqual(AppleMusicPlayRequest.from(context: nil,
                                                  tracks: [media("1"), media("2"), media("x"), media("3")], start: 3),
                       .songs(ids: ["1", "2", "3"], start: 2))
        XCTAssertEqual(AppleMusicPlayRequest.from(context: nil, tracks: [media("1"), media("2")], start: 9),
                       .songs(ids: ["1", "2"], start: 1))
        XCTAssertNil(AppleMusicPlayRequest.from(context: nil, tracks: [], start: 0))
        XCTAssertNil(AppleMusicPlayRequest.from(context: nil, tracks: [media("x")], start: 0))
    }

    func testMusicKitsStateAsTheCoresReport() {
        var snapshot = AppleMusicPlayerSnapshot(status: .stopped, entry: nil, positionSeconds: .nan, shuffle: false,
                                                repeatMode: .off)
        XCTAssertNil(AppleMusicMapping.report(snapshot, context: nil, deviceName: "This Mac"), "nothing plays")

        snapshot.entry = AppleMusicPlayerSnapshot.Entry(id: "1", title: "Song", artist: "Artist", album: "Album",
                                                        durationSeconds: 215.5, artworkURL: nil)
        snapshot.status = .paused
        snapshot.positionSeconds = 12.3456
        snapshot.shuffle = true
        snapshot.repeatMode = .all
        let album = media("123", kind: .album)
        let report = AppleMusicMapping.report(snapshot, context: album, deviceName: "This Mac")
        XCTAssertEqual(report?.playing, false)
        XCTAssertEqual(report?.positionMs, 12_346)
        XCTAssertEqual(report?.item?.durationMs, 215_500)
        XCTAssertEqual(report?.item?.kind, .track)
        XCTAssertEqual(report?.context, album)
        XCTAssertEqual(report?.shuffle, true)
        XCTAssertEqual(report?.repeatMode, .all)
        XCTAssertEqual(report?.device?.id, AppleMusicIDs.device)
        XCTAssertEqual(report?.device?.kind, .computer)
        XCTAssertNil(report?.device?.volumePercent, "MusicKit has no volume: the control hides")

        snapshot.status = .seeking
        snapshot.positionSeconds = .nan
        XCTAssertEqual(AppleMusicMapping.report(snapshot, context: nil, deviceName: "This Mac")?.playing, true)
        XCTAssertEqual(AppleMusicMapping.report(snapshot, context: nil, deviceName: "This Mac")?.positionMs, 0)
        XCTAssertNil(AppleMusicMapping.milliseconds(-1))
        XCTAssertNil(AppleMusicMapping.milliseconds(.infinity))
    }

    func testCatalogueItemsPlayThroughTheAppsPlayer() {
        let song = AppleMusicSong(id: "1", title: "Song", artist: "A", album: "B", durationMs: 1_000, artworkURL: nil,
                                  link: "https://music.apple.com/song/1")
        guard case .source(let track) = song.track else { return XCTFail("a source track") }
        XCTAssertEqual(track.plugin, "apple-music")
        XCTAssertEqual(track.id, "1")
        let album = AppleMusicAlbum(id: "123", title: "Album", artist: nil, artworkURL: nil, link: nil)
        XCTAssertEqual(album.collection, .source(plugin: "apple-music", id: "123", kind: .album))
    }
}

/// MusicKit's player, faked on the main actor.
@MainActor
final class FakeAppleMusicControls: AppleMusicControls {
    var calls: [String] = []
    var played: [AppleMusicPlayRequest] = []
    var failure: Error?
    var current = AppleMusicPlayerSnapshot(status: .stopped, entry: nil, positionSeconds: .nan, shuffle: false,
                                           repeatMode: .off)

    private func check() throws {
        if let failure { throw failure }
    }

    func play(_ request: AppleMusicPlayRequest) async throws {
        try check()
        played.append(request)
    }

    func resume() async throws {
        try check()
        calls.append("resume")
    }

    func pause() { calls.append("pause") }

    func next() async throws {
        try check()
        calls.append("next")
    }

    func previous() async throws {
        try check()
        calls.append("previous")
    }

    func seek(toSeconds seconds: Double) { calls.append("seek \(seconds)") }
    func setShuffle(_ on: Bool) { calls.append("shuffle \(on)") }
    func setRepeat(_ mode: RepeatOption) { calls.append("repeat \(mode)") }
    func snapshot() -> AppleMusicPlayerSnapshot { current }
}

/// Runs `work` on a background thread, as the core calls a player, and waits for it.
private func onCoreThread<T>(_ work: @escaping () throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { continuation in
        DispatchQueue.global().async {
            do {
                continuation.resume(returning: try work())
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

private struct Broken: LocalizedError {
    var errorDescription: String? { "The catalogue did not answer." }
}

@MainActor
final class AppleMusicPlayerTests: XCTestCase {
    func testCallsReachMusicKitOnTheMainActor() async throws {
        let controls = FakeAppleMusicControls()
        let player = AppleMusicPlayer(controls: controls, deviceName: "This Mac")
        try await onCoreThread {
            try player.play(context: nil, tracks: [ExternalMedia(kind: .track, id: "42", title: "Song", artist: nil,
                                                                 album: nil, durationMs: nil, artworkURL: nil)],
                            start: 0)
        }
        XCTAssertEqual(controls.played, [.songs(ids: ["42"], start: 0)])
        try await onCoreThread { try player.pause() }
        try await onCoreThread { try player.seek(positionMs: 1_500) }
        try await onCoreThread { try player.setShuffle(true) }
        try await onCoreThread { try player.setRepeat(.one) }
        try await onCoreThread { try player.next() }
        XCTAssertEqual(controls.calls, ["pause", "seek 1.5", "shuffle true", "repeat one", "next"])
    }

    func testTheReportCarriesTheContextItWasAskedFor() async throws {
        let controls = FakeAppleMusicControls()
        let player = AppleMusicPlayer(controls: controls, deviceName: "Studio")
        let album = ExternalMedia(kind: .album, id: "123", title: "123", artist: nil, album: nil, durationMs: nil,
                                  artworkURL: nil)
        try await onCoreThread { try player.play(context: album, tracks: [], start: 0) }
        XCTAssertEqual(controls.played, [.album(id: "123", start: 0)])
        controls.current = AppleMusicPlayerSnapshot(
            status: .playing,
            entry: AppleMusicPlayerSnapshot.Entry(id: "7", title: "Track", artist: nil, album: nil,
                                                  durationSeconds: 100, artworkURL: nil),
            positionSeconds: 1, shuffle: false, repeatMode: .off
        )
        let report = try await onCoreThread { try player.state() }
        XCTAssertEqual(report?.context, album)
        XCTAssertEqual(report?.item?.id, "7")
        XCTAssertEqual(report?.device?.name, "Studio")
        XCTAssertEqual(try player.devices().map(\.id), [AppleMusicIDs.device])
    }

    func testFailuresAreSentences() async {
        let controls = FakeAppleMusicControls()
        let player = AppleMusicPlayer(controls: controls, deviceName: "This Mac")
        controls.failure = Broken()
        do {
            try await onCoreThread { try player.resume() }
            XCTFail("resume should fail")
        } catch {
            XCTAssertEqual(error as? AppPlayerError, AppPlayerError(reason: "The catalogue did not answer."))
        }
        do {
            try await onCoreThread { try player.transfer(device: "elsewhere") }
            XCTFail("only this Mac")
        } catch {
            XCTAssertNotNil(error as? AppPlayerError)
        }
        XCTAssertNoThrow(try player.transfer(device: AppleMusicIDs.device))
        XCTAssertThrowsError(try player.setVolume(percent: 50), "MusicKit has no volume")
        XCTAssertThrowsError(try player.play(context: nil, tracks: [], start: 0), "nothing to play")
    }

    func testTheMainThreadIsNeverMadeToWaitForItself() {
        let controls = FakeAppleMusicControls()
        let player = AppleMusicPlayer(controls: controls, deviceName: "This Mac")
        XCTAssertThrowsError(try player.pause())
        XCTAssertTrue(controls.calls.isEmpty)
    }
}

/// The account checks and the catalogue, faked.
@MainActor
final class FakeAppleMusicService: AppleMusicService {
    var isSigned = true
    var allows = true
    var authorized = false
    var tokens = true
    var subscription = true
    var found = AppleMusicResults.empty
    private(set) var requests = 0
    private(set) var searches: [String] = []

    func requestAuthorization() async -> Bool {
        requests += 1
        authorized = allows
        return allows
    }

    func isAuthorized() -> Bool { authorized }
    func canFetchTokens() async -> Bool { tokens }
    func canPlayCatalog() async -> Bool { subscription }

    func search(_ term: String) async throws -> AppleMusicResults {
        searches.append(term)
        return found
    }

    func makeControls() -> AppleMusicControls { FakeAppleMusicControls() }
}

@MainActor
final class AppleMusicStoreTests: XCTestCase {
    private var suites: [String] = []

    override func tearDown() {
        for name in suites {
            UserDefaults.standard.removePersistentDomain(forName: name)
        }
        suites = []
        super.tearDown()
    }

    private func defaults() -> UserDefaults {
        let name = "org.jellyfesh.tests.applemusic.\(UUID().uuidString)"
        suites.append(name)
        return UserDefaults(suiteName: name)!
    }

    // Main-actor objects are made in the body: default arguments are evaluated outside the actor.
    private func makeStore(_ service: FakeAppleMusicService?, client: FakeCoreClient? = nil,
                       sources: SourcesStore? = nil, toasts: ToastCenter? = nil,
                       defaults: UserDefaults, build: String = "6") -> AppleMusicStore {
        let client = client ?? FakeCoreClient()
        let toasts = toasts ?? ToastCenter()
        let sources = sources ?? SourcesStore(client: client, tracker: CommandTracker(), toasts: toasts)
        return AppleMusicStore(client: client, sources: sources, toasts: toasts, defaults: defaults, build: build,
                               deviceName: "This Mac", service: { service })
    }

    func testACopyMusicKitCannotWorkInNeverOffersIt() async {
        let service = FakeAppleMusicService()
        service.isSigned = false
        let store = makeStore(service, defaults: defaults())
        await store.start()
        XCTAssertEqual(store.availability, .hidden)
        XCTAssertEqual(service.requests, 0)
    }

    func testItIsOfferedWithoutTouchingMusicKitThenTurnedOnWhenAsked() async {
        let service = FakeAppleMusicService()
        let client = FakeCoreClient()
        let sources = SourcesStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let saved = defaults()
        let store = makeStore(service, client: client, sources: sources, defaults: saved)
        await store.start()
        XCTAssertEqual(store.availability, .offered)
        XCTAssertEqual(service.requests, 0, "nothing is asked before the person turns it on")
        XCTAssertNil(store.catalog)

        await store.turnOn()
        XCTAssertEqual(store.availability, .available)
        XCTAssertEqual(service.requests, 1)
        XCTAssertEqual(client.externalPlayers["apple-music"]?.name, "Apple Music")
        XCTAssertTrue(sources.isRemote("apple-music"))
        XCTAssertTrue(saved.bool(forKey: AppleMusicStore.turnedOnKey))
        XCTAssertNotNil(store.catalog)

        store.turnOff()
        XCTAssertEqual(store.availability, .offered)
        XCTAssertNil(client.externalPlayers["apple-music"])
        XCTAssertFalse(sources.isRemote("apple-music"))
        XCTAssertFalse(saved.bool(forKey: AppleMusicStore.turnedOnKey))
    }

    func testWithoutTokensThisBuildHidesItForGood() async {
        let service = FakeAppleMusicService()
        service.tokens = false
        let toasts = ToastCenter()
        let saved = defaults()
        let store = makeStore(service, toasts: toasts, defaults: saved)
        await store.start()
        await store.turnOn()
        XCTAssertEqual(store.availability, .hidden)
        XCTAssertNotNil(toasts.message, "it says why once")

        let again = makeStore(FakeAppleMusicService(), defaults: saved)
        await again.start()
        XCTAssertEqual(again.availability, .hidden, "the same build is not offered again")
        let newer = makeStore(FakeAppleMusicService(), defaults: saved, build: "7")
        await newer.start()
        XCTAssertEqual(newer.availability, .offered, "a new build may be signed for MusicKit")
    }

    func testWithoutASubscriptionOrAccessItHides() async {
        let noSubscription = FakeAppleMusicService()
        noSubscription.subscription = false
        let first = makeStore(noSubscription, defaults: defaults())
        await first.start()
        await first.turnOn()
        XCTAssertEqual(first.availability, .hidden)

        let denied = FakeAppleMusicService()
        denied.allows = false
        let client = FakeCoreClient()
        let second = makeStore(denied, client: client, defaults: defaults())
        await second.start()
        await second.turnOn()
        XCTAssertEqual(second.availability, .hidden)
        XCTAssertTrue(client.externalPlayers.isEmpty)
    }

    func testOnceTurnedOnItChecksAgainWithoutAsking() async {
        let service = FakeAppleMusicService()
        service.authorized = true
        let saved = defaults()
        saved.set(true, forKey: AppleMusicStore.turnedOnKey)
        let store = makeStore(service, defaults: saved)
        await store.start()
        XCTAssertEqual(store.availability, .available)
        XCTAssertEqual(service.requests, 0)

        let revoked = FakeAppleMusicService()
        let other = defaults()
        other.set(true, forKey: AppleMusicStore.turnedOnKey)
        let again = makeStore(revoked, defaults: other)
        await again.start()
        XCTAssertEqual(again.availability, .offered, "access taken back: it can be turned on again")
    }

    func testTheCatalogueIsSearchedOnlyWhileOn() async {
        let service = FakeAppleMusicService()
        service.found = AppleMusicResults(songs: [AppleMusicSong(id: "1", title: "Tide", artist: nil, album: nil,
                                                                 durationMs: nil, artworkURL: nil, link: nil)],
                                          albums: [])
        let store = makeStore(service, defaults: defaults())
        await store.start()
        await store.pageSearch.run("tide", using: store.catalog)
        XCTAssertTrue(service.searches.isEmpty)
        XCTAssertTrue(store.pageSearch.results.isEmpty)

        await store.turnOn()
        await store.pageSearch.run(" tide ", using: store.catalog)
        XCTAssertEqual(service.searches, ["tide"])
        XCTAssertEqual(store.pageSearch.query, "tide")
        XCTAssertEqual(store.pageSearch.results.songs.map(\.title), ["Tide"])
        store.pageSearch.clear()
        XCTAssertTrue(store.pageSearch.results.isEmpty)
    }
}
