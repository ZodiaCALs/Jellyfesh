// Links and remote playback (ADR-0018, docs/ui.md "Links and remote playback"): what the player
// shows while a remote session runs, the commands it sends, the Devices list, the error shown once
// when a session ends, Open in {Service}, and the conversions of the new core types.

import XCTest
import JellyfeshFFI
@testable import Jellyfesh

@MainActor
final class RemotePlaybackTests: XCTestCase {
    private let client = FakeCoreClient()
    private lazy var store = PlaybackStore(client: client, tracker: CommandTracker())

    func testARemoteSessionIsWhatThePlayerShows() {
        store.apply(nowPlaying: NowPlayingInfo(uid: 4, track: .library(id: 1), path: nil, title: "Local", artist: "A",
                                               album: nil, durationMs: 60_000, artwork: nil, format: nil))
        store.apply(Fixture.playback(status: .paused, positionUs: 1_000_000, monoNs: client.nowNs, rate: 0))
        XCTAssertEqual(store.playing?.title, "Local")
        XCTAssertFalse(store.isRemote)

        store.apply(remote: Fixture.remote(positionUs: 10_000_000, monoNs: client.nowNs))
        XCTAssertTrue(store.isRemote)
        XCTAssertTrue(store.hasTrack)
        XCTAssertTrue(store.isPlaying, "the session plays while the engine is paused")
        let playing = store.playing
        XCTAssertEqual(playing?.title, "Remote Song")
        XCTAssertEqual(playing?.artist, "Remote Artist")
        XCTAssertEqual(playing?.link, "https://open.example.com/track/t1")
        XCTAssertEqual(playing?.source, "org.example.spotify")
        XCTAssertEqual(playing?.remote?.service, "Spotify")
        XCTAssertEqual(playing?.remote?.device, "Device phone")
        XCTAssertNil(playing?.track, "nothing of the engine's")
        XCTAssertEqual(RemoteText.playingOn(playing!.remote!), String(localized: "Playing on \("Device phone")"))
        XCTAssertTrue(store.shuffle)
        XCTAssertEqual(store.repeatMode, .one)
        XCTAssertEqual(store.durationUs, 180_000_000)
        client.nowNs += 2_000_000_000
        XCTAssertEqual(store.positionUs(), 12_000_000, "the session's own anchor")
    }

    func testWithoutADeviceTheServiceIsNamed() {
        store.apply(remote: Fixture.remote(device: nil))
        XCTAssertEqual(store.playing.map { RemoteText.playingOn($0.remote!) }, "Spotify")
        var nameless = Fixture.remote(device: nil)
        nameless.item = nil
        store.apply(remote: nameless)
        XCTAssertEqual(store.playing?.title, "Spotify", "no track reported yet")
    }

    func testTheTransportSendsTheUsualCommands() {
        store.apply(queue: Fixture.queue(position: 0, length: 1, current: Fixture.entry(1, track: 1), upNext: []))
        store.apply(remote: Fixture.remote(monoNs: client.nowNs))
        store.togglePlayPause()
        XCTAssertEqual(client.sent.last, .togglePlayPause)
        store.toggleShuffle()
        XCTAssertEqual(client.sent.last, .setShuffle(false), "the session's shuffle is on")
        store.cycleRepeat()
        XCTAssertEqual(client.sent.last, .setRepeat(.off), "the session repeats one")
        store.next()
        XCTAssertEqual(client.sent.last, .next)
        store.seek(toUs: 30_000_000)
        XCTAssertEqual(client.sent.last, .seek(positionMs: 30_000))
        XCTAssertEqual(store.positionUs(), 30_000_000, "the seek shows until the session answers")
        store.apply(remote: Fixture.remote(version: 2, positionUs: 30_000_000, monoNs: client.nowNs, generation: 2))
        XCTAssertNil(store.pendingSeekUs)
    }

    func testTheVolumeIsThePlayingDevicesAndHidesWithoutOne() {
        store.apply(remote: Fixture.remote(device: Fixture.device("phone", active: true, volume: 40)))
        XCTAssertTrue(store.showsVolume)
        XCTAssertFalse(store.canMute, "a remote device has only its volume")
        XCTAssertEqual(store.displayVolume, 0.4, accuracy: 1e-6)
        store.setVolume(0.555, final: true)
        XCTAssertEqual(client.sent.last, .remote(.setVolume(percent: 56)))
        let count = client.sent.count
        store.toggleMute()
        XCTAssertEqual(client.sent.count, count, "no mute")

        store.apply(remote: Fixture.remote(version: 2, device: Fixture.device("tv", kind: .tv, volume: nil)))
        XCTAssertFalse(store.showsVolume)
        store.setVolume(0.2, final: true)
        store.nudgeVolume(by: 0.05)
        XCTAssertEqual(client.sent.count, count, "nothing to set")
    }

    func testVolumePercentIsWhole() {
        XCTAssertEqual(PlaybackStore.volumePercent(0.555), 56)
        XCTAssertEqual(PlaybackStore.volumePercent(-1), 0)
        XCTAssertEqual(PlaybackStore.volumePercent(2), 100)
        XCTAssertEqual(PlaybackStore.volumePercent(.nan), 0)
    }

    func testDevicesListsEveryDeviceWithThePlayingOneChecked() {
        let kitchen = Fixture.device("kitchen", kind: .speaker, active: true)
        let laptop = Fixture.device("laptop", kind: .computer)
        let nameless = Fixture.device("x", name: "  ")
        var session = Fixture.remote(device: laptop, devices: [kitchen, laptop, nameless])
        XCTAssertEqual(PlaybackStore.deviceRows(session).map(\.id), ["kitchen", "laptop"])
        XCTAssertEqual(PlaybackStore.deviceRows(session).map(\.checked), [false, true])

        session.device = Fixture.device("car", kind: .car)
        XCTAssertEqual(PlaybackStore.deviceRows(session).map(\.id), ["car", "kitchen", "laptop"],
                       "the playing device comes first until the list has it")
        XCTAssertEqual(PlaybackStore.deviceRows(session).first?.checked, true)

        session.device = nil
        XCTAssertEqual(PlaybackStore.deviceRows(session).map(\.checked), [true, false],
                       "without a playing device, the active one")
        XCTAssertEqual(DeviceGlyph.symbol(.tv), "tv")
        XCTAssertEqual(DeviceGlyph.symbol(nil), "speaker.wave.2")
    }

    func testDevicesRefreshTransferAndStopControlling() {
        store.apply(remote: Fixture.remote())
        store.refreshRemote()
        XCTAssertEqual(client.sent.last, .remote(.refresh))
        store.transferRemote(to: "kitchen")
        XCTAssertEqual(client.sent.last, .remote(.transfer(device: "kitchen")))
        store.endRemote()
        XCTAssertEqual(client.sent.last, .remote(.end))
    }

    func testASessionEndingWithAnErrorSaysSoOnce() {
        XCTAssertNil(store.apply(remote: Fixture.remote(version: 1)))
        XCTAssertNil(store.apply(remote: Fixture.remote(version: 2, error: "The service is slow.")),
                     "errors while it runs show in its quieter line")
        XCTAssertEqual(store.playing?.remote?.error, "The service is slow.")
        XCTAssertEqual(store.apply(remote: Fixture.ended(version: 3, error: "Sign in to Spotify again.")),
                       "Sign in to Spotify again.")
        XCTAssertNil(store.apply(remote: Fixture.ended(version: 3, error: "Sign in to Spotify again.")), "once")
        XCTAssertNil(store.apply(remote: Fixture.ended(version: 4, error: nil)))
        XCTAssertFalse(store.isRemote)
        XCTAssertFalse(store.hasTrack)
        XCTAssertNil(store.playing)
    }

    func testTheEnginesTrackShowsAgainAfterTheSession() {
        store.apply(nowPlaying: NowPlayingInfo(uid: 4, track: .library(id: 1), path: nil, title: "Local", artist: nil,
                                               album: nil, durationMs: nil, artwork: nil, format: nil))
        store.apply(remote: Fixture.remote())
        store.apply(remote: Fixture.ended(version: 2, error: nil))
        XCTAssertEqual(store.playing?.title, "Local")
        XCTAssertTrue(store.showsVolume)
        XCTAssertTrue(store.canMute)
    }
}

@MainActor
final class LinkTests: XCTestCase {
    func testLinksAreHttpsOnly() {
        XCTAssertEqual(ServiceLink.url("https://y.qq.com/n/ryqq/songDetail/1")?.host, "y.qq.com")
        XCTAssertNil(ServiceLink.url("http://y.qq.com/n/ryqq/songDetail/1"))
        XCTAssertNil(ServiceLink.url("javascript:alert(1)"))
        XCTAssertNil(ServiceLink.url("file:///etc/hosts"))
        XCTAssertNil(ServiceLink.url(""))
        XCTAssertNil(ServiceLink.url(nil))
    }

    func testAnItemThatCannotPlayHereOpensItsLink() {
        var result = Fixture.result("1", source: "org.example.qq")
        result.playable = false
        result.unplayableReason = "Not playable here"
        result.link = "https://y.qq.com/n/ryqq/songDetail/1"
        let item = TrackItem(result: result, artwork: nil)
        XCTAssertEqual(item.link, result.link)
        XCTAssertEqual(item.source, "org.example.qq")
        XCTAssertFalse(item.canPlay)
        XCTAssertTrue(item.opensLink)

        result.link = nil
        XCTAssertFalse(TrackItem(result: result, artwork: nil).opensLink, "nothing to open")
        let playable = Fixture.result("2", source: "org.example.qq", track: .library(id: 2))
        XCTAssertFalse(TrackItem(result: playable, artwork: nil).opensLink, "it plays")
        XCTAssertNil(TrackItem(result: Fixture.result("3", source: SearchStore.library), artwork: nil).source,
                     "the library is no service")
    }

    func testNowPlayingCarriesItsLink() {
        let song = SourceSong(plugin: "org.example.netease", id: "9", title: "Song", artist: nil, album: nil,
                              durationMs: nil, artworkURL: nil, isrc: nil)
        let now = NowPlayingInfo(uid: 1, track: .source(song), path: nil, title: "Song", artist: nil, album: nil,
                                 durationMs: nil, artwork: nil, format: nil, link: "https://music.163.com/song?id=9")
        let item = TrackItem(nowPlaying: now)
        XCTAssertEqual(item.link, "https://music.163.com/song?id=9")
        XCTAssertEqual(item.source, "org.example.netease")
        XCTAssertTrue(item.canPlay)
        XCTAssertFalse(item.opensLink)
    }

    func testRemoteSourcesAndPlayersTheAppProvides() {
        let store = SourcesStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        var spotify = Fixture.plugin("org.example.spotify")
        spotify.remotePlayback = true
        spotify.attribution = "spotify"
        store.apply(SourcesState(version: 1, plugins: [spotify, Fixture.plugin()], allowCommunity: false))
        XCTAssertTrue(store.isRemote("org.example.spotify"))
        XCTAssertFalse(store.isRemote("org.example.subsonic"))
        let track = SourceSong(plugin: "org.example.spotify", id: "t", title: "T", artist: nil, album: nil,
                               durationMs: nil, artworkURL: nil, isrc: nil)
        XCTAssertTrue(store.isRemote(.source(track)))
        XCTAssertFalse(store.isRemote(.library(id: 1)))
        XCTAssertEqual(store.attribution(of: "org.example.spotify"), String(localized: "Content from \("Example")"))
        XCTAssertNil(store.attribution(of: "org.example.subsonic"))

        XCTAssertFalse(store.isRemote("apple-music"))
        store.setExternalSource("apple-music", name: "Apple Music")
        XCTAssertTrue(store.isRemote("apple-music"))
        XCTAssertEqual(store.name(of: "apple-music"), "Apple Music")
        store.setExternalSource("apple-music", name: nil)
        XCTAssertFalse(store.isRemote("apple-music"))
    }
}

/// The core's new types through the generated bindings.
final class RemoteConversionTests: XCTestCase {
    private func snapshot() -> RemoteSnapshot {
        let item = SourceItem(source: "org.example.spotify", kind: .track, id: "t1", title: "Song", subtitle: "Artist",
                              album: "Album", durationMs: 200_000, artwork: .url(url: "https://img.example.com/a.jpg"),
                              playable: true, unplayableReason: nil, track: nil, alsoIn: [],
                              link: "https://open.example.com/track/t1", artistId: "ar1", albumId: "al1",
                              downloaded: false)
        let kitchen = RemoteDevice(id: "k", name: "Kitchen", kind: .speaker, active: true, volumePercent: 30)
        return RemoteSnapshot(version: 7, source: "org.example.spotify", sourceName: "Spotify", status: .paused,
                              item: item, context: nil,
                              anchor: PlaybackAnchor(trackGeneration: 3, positionUs: 5_000, anchorMonoNs: 9,
                                                     rate: 0, durationUs: 200_000_000),
                              shuffle: true, repeat: .all, device: kitchen,
                              devices: [kitchen, RemoteDevice(id: "c", name: "Car", kind: .car, active: false,
                                                              volumePercent: nil)],
                              error: "Slow", qualityNote: nil)
    }

    func testTheRemoteSliceConverts() {
        let session = RemoteSession(snapshot())
        XCTAssertEqual(session.version, 7)
        XCTAssertEqual(session.source, "org.example.spotify")
        XCTAssertEqual(session.sourceName, "Spotify")
        XCTAssertEqual(session.status, .paused)
        XCTAssertEqual(session.item?.link, "https://open.example.com/track/t1")
        XCTAssertEqual(session.item?.artwork, .url("https://img.example.com/a.jpg"))
        XCTAssertEqual(session.item?.artistID, "ar1")
        XCTAssertEqual(session.item?.albumID, "al1")
        XCTAssertEqual(session.anchor.trackGeneration, 3)
        XCTAssertEqual(session.repeatMode, .all)
        XCTAssertEqual(session.device, ServiceDevice(id: "k", name: "Kitchen", kind: .speaker, active: true,
                                                     volumePercent: 30))
        XCTAssertEqual(session.devices.map(\.kind), [.speaker, .car])
        XCTAssertEqual(session.error, "Slow")
        XCTAssertEqual(CoreEvent(Event.remote(snapshot: snapshot())), .remote(session))
    }

    func testRemoteCommandsConvert() {
        XCTAssertEqual(CoreCommand.remote(.refresh).ffi, Command.remote(action: .refresh))
        XCTAssertEqual(CoreCommand.remote(.transfer(device: "k")).ffi, Command.remote(action: .transfer(device: "k")))
        XCTAssertEqual(CoreCommand.remote(.setVolume(percent: 55)).ffi, Command.remote(action: .setVolume(percent: 55)))
        XCTAssertEqual(CoreCommand.remote(.end).ffi, Command.remote(action: .end))
    }

    func testAPlayersFailuresReachTheCoreAsSentences() {
        let stub = StubPlayer()
        let ffi = FFIExternalPlayer(stub)
        XCTAssertNoThrow(try ffi.resume())
        stub.failure = AppPlayerError(reason: "Apple Music plays only on this Mac.")
        XCTAssertThrowsError(try ffi.transfer(device: "x")) { error in
            XCTAssertEqual(error as? ExternalPlayerError, .Failed(reason: "Apple Music plays only on this Mac."))
        }
        stub.failure = Described()
        XCTAssertThrowsError(try ffi.pause()) { error in
            XCTAssertEqual(error as? ExternalPlayerError, .Failed(reason: "Described."))
        }
    }

    func testThePlayersAnswersConvert() throws {
        let stub = StubPlayer()
        let ffi = FFIExternalPlayer(stub)
        stub.report = ExternalPlayerReport(
            item: ExternalMedia(kind: .track, id: "1", title: "Song", artist: "A", album: nil, durationMs: 1_000,
                                artworkURL: nil),
            context: nil, playing: true, positionMs: 500, shuffle: false, repeatMode: .one,
            device: ServiceDevice(id: "this-mac", name: "This Mac", kind: .computer, active: true, volumePercent: nil)
        )
        let state = try XCTUnwrap(try ffi.state())
        XCTAssertEqual(state.item?.id, "1")
        XCTAssertEqual(state.item?.kind, .track)
        XCTAssertEqual(state.positionMs, 500)
        XCTAssertEqual(state.`repeat`, .one)
        XCTAssertEqual(state.device?.kind, .computer)
        XCTAssertNil(state.device?.volumePercent)
        XCTAssertEqual(try ffi.devices().map(\.id), ["this-mac"])

        try ffi.play(context: ExternalItem(kind: .album, id: "1440857781", title: "1440857781", artist: nil, album: nil,
                                           durationMs: nil, artworkUrl: nil),
                     tracks: [], start: 2)
        XCTAssertEqual(stub.played?.context?.id, "1440857781")
        XCTAssertEqual(stub.played?.start, 2)
        try ffi.setRepeat(mode: .all)
        XCTAssertEqual(stub.repeatMode, .all)
    }
}

/// An `AppPlayer` that records what the core asks.
private final class StubPlayer: AppPlayer, @unchecked Sendable {
    var failure: Error?
    var report: ExternalPlayerReport?
    var played: (context: ExternalMedia?, tracks: [ExternalMedia], start: Int)?
    var repeatMode: RepeatOption?

    private func check() throws {
        if let failure { throw failure }
    }

    func devices() throws -> [ServiceDevice] {
        try check()
        return [ServiceDevice(id: "this-mac", name: "This Mac", kind: .computer, active: true, volumePercent: nil)]
    }

    func state() throws -> ExternalPlayerReport? {
        try check()
        return report
    }

    func play(context: ExternalMedia?, tracks: [ExternalMedia], start: Int) throws {
        try check()
        played = (context, tracks, start)
    }

    func resume() throws { try check() }
    func pause() throws { try check() }
    func next() throws { try check() }
    func previous() throws { try check() }
    func seek(positionMs: UInt64) throws { try check() }
    func setVolume(percent: UInt32) throws { try check() }
    func setShuffle(_ on: Bool) throws { try check() }

    func setRepeat(_ mode: RepeatOption) throws {
        try check()
        repeatMode = mode
    }

    func transfer(device: String) throws { try check() }
}

private struct Described: LocalizedError {
    var errorDescription: String? { "Described." }
}
