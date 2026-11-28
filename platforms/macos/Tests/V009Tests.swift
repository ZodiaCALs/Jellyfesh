// v0.0.9: cards' play buttons and menus (docs/ui.md, "Rows and cards").

import XCTest
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
