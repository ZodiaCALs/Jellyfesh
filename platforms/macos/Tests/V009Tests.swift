// v0.0.9: cards' play buttons and menus, and source songs that keep their album, artist and link
// wherever they show (docs/ui.md, "Rows and cards"; ADR-0027 §2).

import XCTest
import JellyfeshFFI
@testable import Jellyfesh

final class SourceCardTests: XCTestCase {
    private func item(_ kind: ResultKind, playable: Bool = true, artistID: String? = nil) -> SourceResult {
        var item = SourceResult(source: "p", kind: kind, itemID: "x", title: "X", subtitle: "The Artist", album: nil,
                                durationMs: nil, artwork: nil, playable: playable, unplayableReason: nil, track: nil,
                                alsoIn: [])
        item.artistID = artistID
        return item
    }

    func testAlbumAndPlaylistCardsPlayWhole() {
        XCTAssertEqual(SourcePage.playsWhole(item(.album)), .source(plugin: "p", id: "x", kind: .album))
        XCTAssertEqual(SourcePage.playsWhole(item(.playlist)), .source(plugin: "p", id: "x", kind: .playlist))
        XCTAssertNil(SourcePage.playsWhole(item(.album, playable: false)), "one that cannot play here opens instead")
        XCTAssertNil(SourcePage.playsWhole(item(.artist)), "an artist card opens its page")
        XCTAssertNil(SourcePage.playsWhole(item(.folder)))
        XCTAssertNil(SourcePage.playsWhole(item(.track)))
    }

    func testGoToArtistLeadsToTheSourcesArtistOfAnAlbum() {
        let artist = SourcePage.artist(of: item(.album, artistID: "ar-1"))
        XCTAssertEqual(artist?.source, "p")
        XCTAssertEqual(artist?.kind, .artist)
        XCTAssertEqual(artist?.itemID, "ar-1")
        XCTAssertEqual(artist?.title, "The Artist")
        XCTAssertNil(SourcePage.artist(of: item(.album)), "not when the source does not name the artist")
        XCTAssertNil(SourcePage.artist(of: item(.artist, artistID: "ar-1")), "an artist is its own page")
        XCTAssertNil(SourcePage.artist(of: item(.track, artistID: "ar-1")), "a song's menu has its own")
    }
}

final class SourceRowTests: XCTestCase {
    private func song(artistID: String? = nil, albumID: String? = nil) -> SourceSong {
        SourceSong(plugin: "org.jellyfesh.subsonic", id: "t1", title: "Song", artist: "Artist", album: "Album",
                   durationMs: nil, artworkURL: nil, isrc: nil, artistID: artistID, albumID: albumID)
    }

    private func row(_ track: TrackReference, artistID: String? = nil, albumID: String? = nil,
                     link: String? = nil) -> SongRow {
        SongRow(track: track, title: "Song", artist: "Artist", album: "Album", durationMs: nil, artwork: nil,
                artworkHash: nil, favorite: true, missing: false, artistID: artistID, albumID: albumID, link: link)
    }

    func testASourceRowKeepsItsAlbumArtistAndLink() {
        let item = TrackItem(row: row(.source(song()), artistID: "ar1", albumID: "al1",
                                      link: "https://music.example.com/t1"), id: "f0")
        XCTAssertEqual(item.source, "org.jellyfesh.subsonic")
        XCTAssertEqual(item.sourceArtistID, "ar1")
        XCTAssertEqual(item.sourceAlbumID, "al1")
        XCTAssertEqual(item.link, "https://music.example.com/t1")
    }

    func testARowWithoutIdsUsesThoseItsSongWasQueuedWith() {
        let item = TrackItem(row: row(.source(song(artistID: "ar2", albumID: "al2"))), id: "h0")
        XCTAssertEqual(item.sourceArtistID, "ar2")
        XCTAssertEqual(item.sourceAlbumID, "al2")
        let stored = TrackItem(row: row(.source(song())), id: "h1")
        XCTAssertEqual(stored.source, "org.jellyfesh.subsonic", "a row stored before still names its source")
        XCTAssertNil(stored.sourceArtistID)
        XCTAssertNil(stored.link)
    }

    func testALibraryRowHasNoSource() {
        let item = TrackItem(row: row(.library(id: 4)), id: "f1")
        XCTAssertNil(item.source)
        XCTAssertNil(item.sourceArtistID)
        XCTAssertNil(item.sourceAlbumID)
    }

    func testTheQueueAndNowPlayingKeepTheSongsIds() {
        var item = TrackItem(id: "q1", track: .source(song(artistID: "ar3", albumID: "al3")), title: "Song",
                             artist: "Artist", album: nil, durationMs: nil, artwork: nil)
        item.keepSource(of: item.track)
        XCTAssertEqual(item.source, "org.jellyfesh.subsonic")
        XCTAssertEqual(item.sourceArtistID, "ar3")
        XCTAssertEqual(item.sourceAlbumID, "al3")
    }

    func testASourceSongKeepsItsIdsThroughTheCore() {
        let original = song(artistID: "ar4", albumID: "al4")
        XCTAssertEqual(original.ffi.artistId, "ar4")
        XCTAssertEqual(original.ffi.albumId, "al4")
        XCTAssertEqual(SourceSong(original.ffi), original, "played, queued or added to a playlist, it keeps them")
    }

    func testATrackRowsIdsAndLinkReachItsRow() {
        let stored = JellyfeshFFI.TrackRow(track: .source(track: song().ffi), title: "Song", artist: "Artist",
                                           album: "Album", durationMs: nil, artwork: nil, artworkHash: nil,
                                           favorite: false, missing: false, downloaded: false, artistId: "ar5",
                                           albumId: "al5", link: "https://music.example.com/t1")
        let row = SongRow(stored)
        XCTAssertEqual(row.track, .source(song()))
        XCTAssertEqual(row.artistID, "ar5")
        XCTAssertEqual(row.albumID, "al5")
        XCTAssertEqual(row.link, "https://music.example.com/t1")
        let before = JellyfeshFFI.TrackRow(track: .source(track: song().ffi), title: "Song", artist: nil, album: nil,
                                           durationMs: nil, artwork: nil, artworkHash: nil, favorite: false,
                                           missing: false, downloaded: false)
        XCTAssertNil(SongRow(before).artistID, "a row stored before has none")
        XCTAssertNil(SongRow(before).link)
    }
}

