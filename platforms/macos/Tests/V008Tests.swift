// v0.0.8's app rules on top of the core's contract: the network the core hears about, and the
// quality badge's words.

import XCTest
@testable import Jellyfesh

final class NetworkStateTests: XCTestCase {
    func testAnExpensiveOrConstrainedPathIsMetered() {
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: false, constrained: false),
                       NetworkState(online: true, metered: false))
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: true, constrained: false),
                       NetworkState(online: true, metered: true), "a phone's hotspot")
        XCTAssertEqual(NetworkState.from(satisfied: true, expensive: false, constrained: true),
                       NetworkState(online: true, metered: true), "Low Data Mode")
    }

    func testWithoutAPathNothingIsMetered() {
        XCTAssertEqual(NetworkState.from(satisfied: false, expensive: true, constrained: true),
                       NetworkState(online: false, metered: false))
    }
}

final class QualityMenuTests: XCTestCase {
    private let flac = AudioQuality(codec: "flac", sampleRateHz: 96_000, bitDepth: 24, channels: 2,
                                    bitrateKbps: 2_300, lossless: true, label: .hiResLossless)
    private let aac = AudioQuality(codec: "aac", sampleRateHz: 44_100, bitDepth: nil, channels: 2,
                                   bitrateKbps: 256, lossless: false, label: .high)

    func testTheFormatLineIsTheDepthAndRateOrTheBitrate() {
        XCTAssertEqual(QualityMenuContent.formatLine(flac), "FLAC · 24-bit · 96 kHz")
        XCTAssertEqual(QualityMenuContent.formatLine(aac), "AAC · 256 kbit/s")
        XCTAssertNil(QualityMenuContent.formatLine(AudioQuality(codec: nil, sampleRateHz: nil, bitDepth: nil,
                                                                channels: nil, bitrateKbps: nil, lossless: nil,
                                                                label: nil)))
    }

    func testAStreamOffersItsLevelsAutoFirst() {
        let menu = QualityMenuContent(playing: flac, offered: [.auto, .lossless, .high, .normal, .low], chosen: .high,
                                      note: nil)
        XCTAssertEqual(menu.badge, "Hi-Res Lossless", "the core's label for what plays")
        XCTAssertEqual(menu.levels, [.auto, .lossless, .high, .normal, .low])
        XCTAssertEqual(menu.chosen, .high)
        XCTAssertNil(menu.note)
    }

    func testWithoutLevelsTheMenuSaysWhy() {
        let file = QualityMenuContent(playing: aac, offered: [], chosen: .auto, note: .filesPlayAsTheyAre)
        XCTAssertTrue(file.levels.isEmpty)
        XCTAssertEqual(file.note, "Files play as they are.")
        let remote = QualityMenuContent(playing: nil, offered: [], chosen: .lossless, note: .setInService("Spotify"))
        XCTAssertEqual(remote.note, "Quality is set in Spotify.")
        XCTAssertEqual(remote.badge, "Lossless", "the chosen level before anything is decoded")
        XCTAssertNil(remote.formatLine)
    }
}

@MainActor
final class QualityMenuStoreTests: XCTestCase {
    func testTheBadgeFollowsWhatPlays() {
        let store = PlaybackStore(client: FakeCoreClient(), tracker: CommandTracker())
        XCTAssertNil(store.qualityMenu(chosen: .auto), "no badge with nothing playing")
        var now = NowPlayingInfo(uid: 1, track: .file(path: "/Music/a.flac"), path: "/Music/a.flac", title: "A",
                                 artist: nil, album: nil, durationMs: 1_000, artwork: nil, format: nil)
        now.qualityNote = .filesPlayAsTheyAre
        store.apply(nowPlaying: now)
        XCTAssertEqual(store.qualityMenu(chosen: .auto)?.note, "Files play as they are.")
        XCTAssertEqual(store.qualityMenu(chosen: .auto)?.levels, [])

        now.qualities = [.auto, .lossless, .high]
        now.qualityNote = nil
        store.apply(nowPlaying: now)
        let menu = store.qualityMenu(chosen: .high)
        XCTAssertEqual(menu?.levels, [.auto, .lossless, .high])
        XCTAssertEqual(menu?.chosen, .high)
        XCTAssertEqual(menu?.badge, "High", "the chosen level until the engine has opened the song")
    }
}

/// A date a test moves by hand.
private final class DateBox {
    var now = Date(timeIntervalSince1970: 1_000_000)
}

@MainActor
final class SourceLibraryStoreTests: XCTestCase {
    private func entry(_ id: String, kind: ResultKind = .playlist, songsListed: Bool = true) -> SourceListEntry {
        SourceListEntry(item: Fixture.result(id, source: "p", kind: kind), owned: true, songsListed: songsListed)
    }

    private func list(_ part: SourceListPart, _ entries: [SourceListEntry], next: String? = nil) -> SourceList {
        SourceList(part: part, entries: entries, total: nil, next: next, refreshedAtMs: 1_000, loading: false,
                   error: nil)
    }

    func testThePagesRefreshWhenTheyOpenAndAgainAfterTenMinutes() {
        let client = FakeCoreClient()
        let clock = DateBox()
        let store = SourceLibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter(),
                                       now: { clock.now })
        store.pageOpened(.favourites)
        XCTAssertEqual(client.sent.last, .refreshSourceLibrary(plugin: nil))
        let sent = client.sent.count
        store.pageOpened(.favourites)
        XCTAssertEqual(client.sent.count, sent, "not again within ten minutes")
        store.pageOpened(.playlists)
        XCTAssertEqual(client.sent.count, sent + 1, "each page asks once")
        clock.now = clock.now.addingTimeInterval(601)
        store.pageOpened(.favourites)
        XCTAssertEqual(client.sent.count, sent + 2, "returned to after ten minutes")
    }

    func testFavouritesAndPlaylistsShowWhatTheSourcesKeep() {
        let store = SourceLibraryStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(SourceLibraryState(version: 1, sources: [
            SourceLibraryInfo(plugin: "p", name: "Jellyfin", attribution: nil, lists: [
                list(.likedSongs, [entry("s1", kind: .track)]), list(.likedAlbums, []), list(.playlists, [entry("pl1")]),
            ]),
            SourceLibraryInfo(plugin: "q", name: "Subsonic", attribution: nil, lists: [
                list(.likedSongs, []), list(.playlists, []),
            ]),
        ]))
        XCTAssertEqual(store.likes.map(\.plugin), ["p"], "a source with nothing to show is left out")
        XCTAssertEqual(store.likes.first?.lists.map(\.part), [.likedSongs], "and so is an empty list")
        XCTAssertEqual(store.playlists.map(\.plugin), ["p"])
        XCTAssertEqual(store.playlists.first?.lists.map(\.part), [.playlists])
    }

    func testMoreJoinsItsListOnce() {
        let client = FakeCoreClient()
        let store = SourceLibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(SourceLibraryState(version: 1, sources: [
            SourceLibraryInfo(plugin: "p", name: "Jellyfin", attribution: nil,
                              lists: [list(.playlists, [entry("pl1")], next: "c2")]),
        ]))
        store.more(plugin: "p", part: .playlists)
        XCTAssertEqual(client.sent.last, .sourceLibraryMore(plugin: "p", part: .playlists, cursor: "c2"))
        store.apply(page: SourceLibraryPageBatch(request: client.lastCommandID, plugin: "p", part: .playlists,
                                                 entries: [entry("pl1"), entry("pl2", songsListed: false)], next: nil,
                                                 error: nil))
        let entries = store.playlists.first?.lists.first?.entries
        XCTAssertEqual(entries?.map(\.item.itemID), ["pl1", "pl2"])
        XCTAssertEqual(entries?.last?.songsListed, false)
        XCTAssertNil(store.playlists.first?.lists.first?.next)
        XCTAssertTrue(store.loadingMore.isEmpty)
    }

    func testTheCachedListsSayWhenTheyWereUpdated() {
        let line = SourceLibraryStore.updatedLine(refreshedAt: Date(timeIntervalSince1970: 0),
                                                  now: Date(timeIntervalSince1970: 300))
        XCTAssertTrue(line.hasPrefix("Updated "), line)
        XCTAssertTrue(line.contains("5"), line)
    }

    func testSaveAsPlaylistNamesTheCopy() {
        let client = FakeCoreClient()
        let store = SourceLibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        XCTAssertNotNil(store.savePlaylistCopy(plugin: "p", id: "pl1", name: "  Road Trip "))
        XCTAssertEqual(client.sent.last, .savePlaylistCopy(plugin: "p", id: "pl1", name: "Road Trip"))
        XCTAssertNil(store.savePlaylistCopy(plugin: "p", id: "pl1", name: "  "), "a blank name sends nothing")
    }
}

@MainActor
final class DownloadsStoreTests: XCTestCase {
    private func entry(_ id: UInt64, album: String?, phase: DownloadPhase, done: UInt64 = 0,
                       total: UInt64? = nil) -> DownloadEntry {
        DownloadEntry(id: id, track: SourceSong(plugin: "p", id: "t\(id)", title: "Song \(id)", artist: "Artist",
                                                album: album, durationMs: nil, artworkURL: nil, isrc: nil),
                      phase: phase, bytesDone: done, bytesTotal: total, path: nil, libraryID: nil, original: true)
    }

    func testInProgressFirstThenTheFinishedByAlbum() {
        let store = DownloadsStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(DownloadsState(version: 1, folder: "/Music/Jellyfesh", items: [
            entry(1, album: "A", phase: .downloading, done: 50, total: 200),
            entry(2, album: "A", phase: .failed(.notAllowed(service: "Jellyfin"))),
            entry(3, album: "A", phase: .done), entry(4, album: "A", phase: .done), entry(5, album: "B", phase: .done),
        ], bytesUsed: 3_000_000, paused: nil))
        XCTAssertEqual(store.inProgress.map(\.id), [1, 2])
        XCTAssertEqual(store.albums.map(\.title), ["A", "B"])
        XCTAssertEqual(store.albums.first?.entries.map(\.id), [3, 4])
        XCTAssertEqual(DownloadsStore.progress(store.inProgress[0]), 0.25)
        XCTAssertNil(DownloadsStore.progress(store.inProgress[1]), "no size known")
    }

    func testAFinishedDownloadPlaysItsLibraryTrackOrTheSourceSong() {
        var linked = entry(3, album: "A", phase: .done)
        linked.libraryID = 42
        XCTAssertEqual(DownloadsStore.track(linked), .library(id: 42))
        let unlinked = entry(4, album: "A", phase: .done)
        XCTAssertEqual(DownloadsStore.track(unlinked), .source(unlinked.track))
    }

    func testDownloadsAreWordedAsTheSpecSays() {
        XCTAssertEqual(DownloadsStore.problem(.notAllowed(service: "Jellyfin")),
                       "Your Jellyfin server doesn't allow downloads for this account.")
        XCTAssertEqual(DownloadsStore.problem(.network("Timed out")), "Timed out")
        XCTAssertEqual(DownloadsStore.paused(.metered), "Downloads wait for Wi-Fi.")
    }

    func testTheChoicesGoToTheCore() {
        let client = FakeCoreClient()
        let store = DownloadsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.download(.collection(plugin: "p", id: "al1", kind: .album))
        XCTAssertEqual(client.sent.last, .download(.collection(plugin: "p", id: "al1", kind: .album)))
        store.retry(2)
        XCTAssertEqual(client.sent.last, .retryDownload(id: 2))
        store.cancel(1)
        XCTAssertEqual(client.sent.last, .cancelDownload(id: 1))
        store.remove([3, 4])
        XCTAssertEqual(Array(client.sent.suffix(2)), [.removeDownload(id: 3), .removeDownload(id: 4)])
        store.removeAll()
        XCTAssertEqual(client.sent.last, .removeAllDownloads)
    }

    func testFoldersShowUnderTheHomeWithATilde() {
        XCTAssertEqual(DownloadsStore.displayPath("/Users/kim/Music/Jellyfesh", home: "/Users/kim"), "~/Music/Jellyfesh")
        XCTAssertEqual(DownloadsStore.displayPath("/Users/kimberly/Music", home: "/Users/kim"), "/Users/kimberly/Music")
        XCTAssertEqual(DownloadsStore.displayPath("/Volumes/Disk/Music", home: nil), "/Volumes/Disk/Music")
    }

    func testQualityNamesAndTheFileNamesPreview() {
        XCTAssertEqual(DownloadQualityOption.allCases.map(\.title), ["Original", "High", "Normal", "Low"])
        let store = DownloadsStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        XCTAssertEqual(store.preview("{album}/{title}"), "Preview/{album}/{title}", "the core's rules make the path")
    }

    private func result(_ kind: ResultKind) -> SourceResult {
        SourceResult(source: "p", kind: kind, itemID: "x", title: "X", subtitle: nil, album: nil, durationMs: nil,
                     artwork: nil, playable: true, unplayableReason: nil, track: nil, alsoIn: [])
    }

    func testDownloadIsOfferedForASourcesAlbumsAndPlaylists() {
        XCTAssertEqual(SourcePage.downloadRequest(plugin: "p", item: result(.album), offered: true),
                       .collection(plugin: "p", id: "x", kind: .album))
        XCTAssertEqual(SourcePage.downloadRequest(plugin: "p", item: result(.playlist), offered: true),
                       .collection(plugin: "p", id: "x", kind: .playlist))
        XCTAssertNil(SourcePage.downloadRequest(plugin: "p", item: result(.artist), offered: true))
        XCTAssertNil(SourcePage.downloadRequest(plugin: "p", item: result(.album), offered: false),
                     "not from a source that offers no downloads")
    }

    func testRowsCarryTheDownloadedMark() {
        var song = result(.track)
        song.downloaded = true
        XCTAssertTrue(TrackItem(result: song, artwork: nil).downloaded)
        XCTAssertFalse(TrackItem(result: result(.track), artwork: nil).downloaded)
        let row = SongRow(track: .source(SourceSong(plugin: "p", id: "t", title: "T", artist: nil, album: nil,
                                                    durationMs: nil, artworkURL: nil, isrc: nil)),
                          title: "T", artist: nil, album: nil, durationMs: nil, artwork: nil, artworkHash: nil,
                          favorite: false, missing: false, downloaded: true)
        XCTAssertTrue(TrackItem(row: row, id: "r1").downloaded)
    }
}
