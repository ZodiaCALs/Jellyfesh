import XCTest
@testable import Jellyfesh

@MainActor
final class PlaybackStoreTests: XCTestCase {
    private let client = FakeCoreClient()
    private lazy var store = PlaybackStore(client: client, tracker: CommandTracker())

    func testPositionFollowsTheAnchor() {
        store.apply(Fixture.playback(positionUs: 5_000_000, monoNs: client.nowNs))
        client.nowNs += 2_000_000_000
        XCTAssertEqual(store.positionUs(), 7_000_000)
        XCTAssertTrue(store.isPlaying)
        XCTAssertEqual(store.durationUs, 200_000_000)
    }

    func testPausedPositionStays() {
        store.apply(Fixture.playback(status: .paused, positionUs: 5_000_000, monoNs: client.nowNs, rate: 0))
        client.nowNs += 3_000_000_000
        XCTAssertEqual(store.positionUs(), 5_000_000)
        XCTAssertFalse(store.scheduleStart().running)
    }

    func testSeekShowsTheTargetUntilTheNextAnchor() {
        store.apply(Fixture.playback(generation: 1, positionUs: 10_000_000, monoNs: client.nowNs))
        store.seek(toUs: 30_000_000)
        XCTAssertEqual(client.sent.last, .seek(positionMs: 30_000))
        XCTAssertEqual(store.positionUs(), 30_000_000)
        XCTAssertFalse(store.scheduleStart().running, "a pending seek draws still")

        store.apply(Fixture.playback(generation: 2, positionUs: 30_000_000, monoNs: client.nowNs))
        XCTAssertNil(store.pendingSeekUs)
        client.nowNs += 1_000_000_000
        XCTAssertEqual(store.positionUs(), 31_000_000)
    }

    func testSeekStaysInTheTrack() {
        store.apply(Fixture.playback(positionUs: 1_000_000, monoNs: client.nowNs, durationUs: 60_000_000))
        store.seek(byUs: -5_000_000)
        XCTAssertEqual(client.sent.last, .seek(positionMs: 0))
        store.seek(toUs: 90_000_000)
        XCTAssertEqual(client.sent.last, .seek(positionMs: 60_000))
    }

    func testScheduleStartCarriesBothEstimates() {
        store.apply(Fixture.playback(positionUs: 4_000_000, monoNs: client.nowNs))
        client.nowNs += 500_000_000
        let start = store.scheduleStart()
        XCTAssertTrue(start.running)
        XCTAssertEqual(start.anchorPositionUs, 4_500_000)
        XCTAssertEqual(start.positionUs, 4_500_000)
        XCTAssertEqual(start.durationUs, 200_000_000)
    }

    func testFinalVolumeIsSentOnce() {
        store.setVolume(0.5, final: true)
        store.setVolume(0.5, final: true)
        XCTAssertEqual(client.sent.filter { $0 == .setVolume(0.5) }.count, 1)
        XCTAssertEqual(store.displayVolume, 0.5)
        store.setVolume(1.4, final: true)
        XCTAssertEqual(client.sent.last, .setVolume(1))
    }

    func testMuteIsReadBackAfterThePlaybackSlice() async {
        client.mutedResult = true
        store.toggleMute()
        XCTAssertEqual(client.sent.last, .setMuted(true))
        store.apply(Fixture.playback(monoNs: client.nowNs))
        await eventually { store.muted }
    }

    func testRepeatCyclesOffAllOne() {
        store.apply(queue: Fixture.queue(position: 0, length: 1, current: Fixture.entry(1, track: 1), upNext: []))
        store.cycleRepeat()
        XCTAssertEqual(client.sent.last, .setRepeat(.all))
        XCTAssertEqual(RepeatOption.all.next, .one)
        XCTAssertEqual(RepeatOption.one.next, .off)
    }

    func testMovingUpNextNamesTheIndexInTheWholeQueue() {
        let upNext = [Fixture.entry(10, track: 1), Fixture.entry(11, track: 2), Fixture.entry(12, track: 3)]
        store.apply(queue: Fixture.queue(position: 2, length: 6, current: Fixture.entry(9, track: 9), upNext: upNext))
        // SwiftUI's onMove: the first row dropped after the last (gap index 3).
        store.moveUpNext(from: 0, to: 3, in: upNext)
        XCTAssertEqual(client.sent.last, .moveInQueue(uid: 10, to: 5))
        // The last row dropped before the first.
        store.moveUpNext(from: 2, to: 0, in: upNext)
        XCTAssertEqual(client.sent.last, .moveInQueue(uid: 12, to: 3))
        let count = client.sent.count
        store.moveUpNext(from: 1, to: 2, in: upNext)
        XCTAssertEqual(client.sent.count, count, "dropping a row where it was sends nothing")
    }

    func testPlayingTracksClampsTheStart() {
        store.play(tracks: [.library(id: 1), .library(id: 2)], from: 7)
        XCTAssertEqual(client.sent.last, .playTracks([.library(id: 1), .library(id: 2)], start: 1))
        let count = client.sent.count
        store.play(tracks: [], from: 0)
        XCTAssertEqual(client.sent.count, count)
    }

    func testBufferingBelongsToTheTrackPlaying() {
        let now = NowPlayingInfo(uid: 4, track: .library(id: 1), path: nil, title: "A", artist: nil, album: nil,
                                 durationMs: nil, artwork: nil, format: nil)
        store.apply(nowPlaying: now)
        store.apply(bufferingUID: 3, buffering: true)
        XCTAssertFalse(store.buffering)
        store.apply(bufferingUID: 4, buffering: true)
        XCTAssertTrue(store.buffering)
        store.apply(nowPlaying: NowPlayingInfo(uid: 5, track: .library(id: 2), path: nil, title: "B", artist: nil,
                                               album: nil, durationMs: nil, artwork: nil, format: nil))
        XCTAssertFalse(store.buffering, "a new track starts without the old one's buffering")
    }
}

@MainActor
final class SettingsStoreTests: XCTestCase {
    func testChangesBuildOnTheChangeInFlight() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.setReduceEffects(true)
        store.setShowTranslation(false)
        guard case .setSettings(let sent)? = client.sent.last else {
            return XCTFail("expected SetSettings")
        }
        XCTAssertTrue(sent.reduceEffects, "the first change is kept")
        XCTAssertFalse(sent.showTranslation)
        XCTAssertEqual(store.current, sent)
    }

    func testTheCoresAnswerReplacesTheDraft() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.setAppearance(.dark)
        XCTAssertNotNil(store.draft)
        var confirmed = AppSettings.defaults
        confirmed.appearance = .dark
        store.apply(confirmed)
        XCTAssertNil(store.draft)
        XCTAssertEqual(store.current.appearance, .dark)
    }

    func testDefaultsMatchTheCore() {
        // core/jf-core/src/types.rs, Settings::default().
        XCTAssertEqual(AppSettings.defaults.transparency, 0.5)
        XCTAssertEqual(AppSettings.defaults.lyricsSize, .medium)
        XCTAssertEqual(AppSettings.defaults.lyricsAlign, .start)
    }

    func testUnchangedSettingsAreNotSent() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.setAppearance(.system)
        XCTAssertTrue(client.sent.isEmpty)
    }

    func testOnChangeSeesEveryChangeAtOnce() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        var seen: [String] = []
        store.onChange = { seen.append($0.appIcon) }
        store.apply(.defaults)
        store.setAppIcon("steel")
        XCTAssertEqual(seen, ["automatic", "steel"])
    }

    func testAFailedChangeShowsWhyAndFallsBack() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let toasts = ToastCenter()
        let store = SettingsStore(client: client, tracker: tracker, toasts: toasts)
        store.apply(.defaults)
        store.setWatchFolders(false)
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: "settings could not be saved"))
        XCTAssertNil(store.draft)
        XCTAssertTrue(store.current.watchFolders)
        XCTAssertEqual(toasts.message, "settings could not be saved")
    }

    func testPresetsComeFromTheCore() {
        let client = FakeCoreClient()
        client.equalizerPresetsResult = Fixture.equalizerPresets
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        XCTAssertEqual(store.equalizerPresets.map(\.id), ["flat", "bass_boost", "vocal", "treble"])
        XCTAssertEqual(EqualizerPresetText.name("bass_boost"), String(localized: "Bass Boost"))
        XCTAssertEqual(EqualizerPresetText.name("late_night"), "Late night", "an id this build does not know")
    }

    func testThePresetFollowsTheBandsAndShowsCustomOnceOneMoves() {
        let client = FakeCoreClient()
        client.equalizerPresetsResult = Fixture.equalizerPresets
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        // The preamp and the switch are not part of the choice.
        let boosted = Fixture.equalizer([6, 5, 4, 2, 0.5, 0, 0, 0, 0, 0], preampDb: -3, enabled: false)
        XCTAssertEqual(store.equalizerPreset(matching: boosted)?.id, "bass_boost")
        var moved = boosted
        moved.bands[9].gainDb = 1.5
        XCTAssertNil(store.equalizerPreset(matching: moved), "Custom")
        XCTAssertNil(store.equalizerPreset(matching: EqualizerConfig(enabled: false, preampDb: 0, bands: [])))
    }

    func testApplyingAPresetSendsItsSettings() {
        let client = FakeCoreClient()
        client.equalizerPresetsResult = Fixture.equalizerPresets
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.applyEqualizerPreset(Fixture.equalizerPresets[2])
        guard case .setSettings(let sent)? = client.sent.last else {
            return XCTFail("expected SetSettings")
        }
        XCTAssertEqual(sent.eq, Fixture.equalizerPresets[2].settings)
        XCTAssertTrue(sent.eq.enabled, "a preset switches the equalizer on")
        XCTAssertEqual(store.equalizerPreset(matching: store.current.eq)?.id, "vocal")
    }

    func testIconChoicesMatchTheTokens() {
        let ids = AppIconChoice.allCases.dropFirst().map(\.rawValue).joined(separator: ", ")
        XCTAssertEqual(ids, Tokens.Brand.iconOptions)
        XCTAssertEqual(AppIconChoice.automatic.thumbnailName, "IconThumb-ice")
        XCTAssertEqual(AppIconChoice.seaGlass.thumbnailName, "IconThumb-sea-glass")
    }
}

@MainActor
final class LibraryStoreTests: XCTestCase {
    func testAPlaylistMadeHereOpensWhenTheCoreNamesIt() {
        let client = FakeCoreClient()
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.createPlaylist(name: "Road")
        let request = client.lastCommandID
        store.playlistCreated(request: request + 7, id: 99)
        XCTAssertNil(store.createdPlaylist, "another window's playlist")
        store.playlistCreated(request: request, id: 42)
        XCTAssertEqual(store.consumeCreatedPlaylist(), 42)
        XCTAssertNil(store.createdPlaylist)
    }

    func testFailuresBecomeMessages() {
        let client = FakeCoreClient()
        let tracker = CommandTracker()
        let toasts = ToastCenter()
        let store = LibraryStore(client: client, tracker: tracker, toasts: toasts)
        store.addFolder("/Volumes/Music")
        XCTAssertEqual(client.sent.last, .addRoot(path: "/Volumes/Music"))
        XCTAssertTrue(tracker.fail(client.lastCommandID, message: "folder not found"))
        XCTAssertEqual(toasts.message, "folder not found")
    }

    func testMovingAnEntryNamesItsFinalIndex() {
        let client = FakeCoreClient()
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let row = SongRow(track: .library(id: 1), title: "A", artist: nil, album: nil, durationMs: nil, artwork: nil,
                          artworkHash: nil, favorite: false, missing: false)
        let rows = (0..<3).map { PlaylistRow(entryID: Int64(100 + $0), position: UInt32($0), row: row) }
        store.moveEntry(from: 0, to: 3, in: rows, playlist: 5)
        XCTAssertEqual(client.sent.last, .movePlaylistEntry(id: 5, entry: 100, to: 2))
    }

    func testBlankNamesAreNotSent() {
        let client = FakeCoreClient()
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.renamePlaylist(3, to: "   ")
        XCTAssertTrue(client.sent.isEmpty)
        store.renamePlaylist(3, to: " Evening ")
        XCTAssertEqual(client.sent.last, .renamePlaylist(id: 3, name: "Evening"))
    }

    func testNewPlaylistAsksForTheNameFirst() {
        let client = FakeCoreClient()
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.askForNewPlaylist()
        XCTAssertNotNil(store.newPlaylist)
        XCTAssertTrue(client.sent.isEmpty, "nothing is made before the name")
        store.createNewPlaylist(named: "  Road Trip ")
        XCTAssertEqual(client.sent, [.createPlaylist(name: "Road Trip", tracks: [])])
        XCTAssertNil(store.newPlaylist)
        // The playlist opens once the core names it.
        store.playlistCreated(request: client.lastCommandID, id: 12)
        XCTAssertEqual(store.consumeCreatedPlaylist(), 12)
    }

    func testABlankNameGivesTheDefaultAndCancelMakesNothing() {
        let client = FakeCoreClient()
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.askForNewPlaylist()
        store.cancelNewPlaylist()
        store.createNewPlaylist(named: "Ignored")
        XCTAssertTrue(client.sent.isEmpty)
        store.askForNewPlaylist(with: .tracks([.library(id: 4)]))
        store.createNewPlaylist(named: " ")
        XCTAssertEqual(client.sent, [.createPlaylist(name: String(localized: "New Playlist"), tracks: [.library(id: 4)])])
    }

    func testAnAlbumGoesIntoAPlaylistAsItsSongs() async {
        let client = FakeCoreClient()
        client.songsResult = [Fixture.song(1, album: 7), Fixture.song(2, album: 7), Fixture.song(3, album: 8)]
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.add(.album(id: 7), toPlaylist: 5)
        await eventually { !client.sent.isEmpty }
        XCTAssertEqual(client.sent, [.addToPlaylist(id: 5, tracks: [.library(id: 1), .library(id: 2)])])
        // Songs chosen in a list go at once.
        store.add(.tracks([.library(id: 3)]), toPlaylist: 5)
        XCTAssertEqual(client.sent.last, .addToPlaylist(id: 5, tracks: [.library(id: 3)]))
    }

    func testANewPlaylistFromAPageStartsWithItsSongs() async {
        let client = FakeCoreClient()
        client.songsResult = [Fixture.song(1, album: 7), Fixture.song(2, album: 7)]
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.askForNewPlaylist(with: .album(id: 7))
        store.createNewPlaylist(named: "Album Mix")
        await eventually { !client.sent.isEmpty }
        XCTAssertEqual(client.sent, [.createPlaylist(name: "Album Mix", tracks: [.library(id: 1), .library(id: 2)])])
    }

    func testAPlaylistIsReadPageByPage() async throws {
        let client = FakeCoreClient()
        client.playlistEntriesResult[3] = (0..<1_200).map { index in
            PlaylistRow(entryID: Int64(index), position: UInt32(index),
                        row: SongRow(track: .library(id: Int64(index)), title: "Song", artist: nil, album: nil,
                                     durationMs: nil, artwork: nil, artworkHash: nil, favorite: false, missing: false))
        }
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let tracks = try await store.tracks(of: .playlist(id: 3))
        XCTAssertEqual(tracks.count, 1_200)
        XCTAssertEqual(tracks.last, .library(id: 1_199))
        let none = try await store.tracks(of: .source(plugin: "p", id: "x"))
        XCTAssertTrue(none.isEmpty, "a source's container has no tracks to add")
    }

    func testTheRevisionRefreshesFoldersAndPlaylists() async {
        let client = FakeCoreClient()
        client.rootsResult = ["/Music"]
        client.playlistsResult = [PlaylistInfo(id: 1, name: "Road", entryCount: 2, durationMs: 1, updatedAtMs: 0,
                                               artworkHash: nil)]
        let store = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(revision: 3)
        await eventually { store.roots == ["/Music"] && store.playlists.count == 1 }
    }
}

@MainActor
final class SongsPageTests: XCTestCase {
    func testPlayAndShuffleTakeEverySongInTheOrderShown() async {
        let client = FakeCoreClient()
        client.songsResult = (1...2_500).map { Fixture.song(Int64($0)) }
        let model = SongsModel(client: client)
        let playback = PlaybackStore(client: client, tracker: CommandTracker())
        await model.playAll(shuffle: true, on: playback)
        guard case .playCollection(.tracks(let tracks), let start, let shuffle)? = client.sent.last else {
            return XCTFail("expected PlayCollection")
        }
        XCTAssertEqual(tracks.count, 2_500, "every song, not only the rows loaded")
        XCTAssertEqual(tracks.first, .library(id: 1))
        XCTAssertEqual(start, 0)
        XCTAssertTrue(shuffle)
    }

    func testAnEmptyLibraryPlaysNothing() async {
        let client = FakeCoreClient()
        let model = SongsModel(client: client)
        await model.playAll(shuffle: false, on: PlaybackStore(client: client, tracker: CommandTracker()))
        XCTAssertTrue(client.sent.isEmpty)
    }

    func testYearsAndRecentlyAddedStartNewestFirst() async {
        let client = FakeCoreClient()
        let songs = SongsModel(client: client)
        await songs.choose(.added, revision: 1)
        XCTAssertEqual(songs.sort, .added)
        XCTAssertTrue(songs.descending)
        await songs.choose(.title, revision: 1)
        XCTAssertEqual(songs.sort, .title)
        XCTAssertFalse(songs.descending)

        let albums = AlbumsModel(client: client)
        await albums.choose(.year, revision: 1)
        XCTAssertTrue(albums.descending)
        await albums.setOrder(.year, descending: false, revision: 1)
        XCTAssertFalse(albums.descending, "Descending still flips it")
        await albums.choose(.artist, revision: 1)
        XCTAssertFalse(albums.descending)

        XCTAssertTrue(SortDirection.startsDescending(AlbumOrder.added))
        XCTAssertFalse(SortDirection.startsDescending(AlbumOrder.title))
        XCTAssertTrue(SortDirection.startsDescending(SongSort.year))
        XCTAssertFalse(SortDirection.startsDescending(SongSort.album))
    }
}

@MainActor
final class RouterTests: XCTestCase {
    func testEscGoesBackAPageAtATime() {
        let router = Router()
        router.select(.playlists)
        router.open(.playlist(1))
        router.open(.playlist(2))
        XCTAssertTrue(router.back())
        XCTAssertEqual(router.path, [.playlist(1)])
        XCTAssertTrue(router.back())
        XCTAssertTrue(router.path.isEmpty)
        XCTAssertFalse(router.back(), "a section's own page has nothing to go back to")
        XCTAssertEqual(router.selection, .playlists)
    }
}

@MainActor
final class PagedListTests: XCTestCase {
    private final class Source {
        var items = (0..<450).map { Item(id: $0) }
        var calls: [(Int, Int)] = []
    }

    private struct Item: Identifiable, Equatable {
        var id: Int
    }

    func testLoadsPageByPageUntilTheEnd() async {
        let source = Source()
        let list = PagedList<Item>(pageSize: 200) { offset, limit in
            source.calls.append((offset, limit))
            return Array(source.items.dropFirst(offset).prefix(limit))
        }
        await list.load(revision: 1)
        XCTAssertEqual(list.items.count, 200)
        XCTAssertFalse(list.reachedEnd)
        await list.loadMore()
        await list.loadMore()
        XCTAssertEqual(list.items.count, 450)
        XCTAssertTrue(list.reachedEnd)
        await list.loadMore()
        XCTAssertEqual(source.calls.count, 3, "nothing more is asked after the end")
    }

    func testANewRevisionQueriesTheLoadedRangeAgain() async {
        let source = Source()
        let list = PagedList<Item>(pageSize: 200) { offset, limit in
            source.calls.append((offset, limit))
            return Array(source.items.dropFirst(offset).prefix(limit))
        }
        await list.load(revision: 1)
        await list.loadMore()
        await list.load(revision: 1)
        XCTAssertEqual(source.calls.count, 2, "the same revision is not loaded twice")
        await list.load(revision: 2)
        XCTAssertEqual(source.calls.last?.0, 0)
        XCTAssertEqual(source.calls.last?.1, 400)
        XCTAssertEqual(list.loadedRevision, 2)
    }

    func testRepeatedRowsAreDropped() async {
        let list = PagedList<Item>(pageSize: 2) { offset, _ in
            // The library changed between pages: the second page repeats a row.
            offset == 0 ? [Item(id: 1), Item(id: 2)] : [Item(id: 2), Item(id: 3)]
        }
        await list.load(revision: 1)
        await list.loadMore()
        XCTAssertEqual(list.items.map(\.id), [1, 2, 3])
    }

    func testErrorsAreKept() async {
        let list = PagedList<Item>(pageSize: 2) { _, _ in
            throw CoreClientError.core("library is locked")
        }
        await list.load(revision: 1)
        XCTAssertEqual(list.error, "library is locked")
        XCTAssertTrue(list.items.isEmpty)
    }
}
