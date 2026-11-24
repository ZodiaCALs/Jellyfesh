// Music folders (and the downloads folder, under its own key) in the sandboxed (Mac App Store)
// build: a chosen folder's bookmark is kept, held open while the app runs, restored at launch (a
// moved folder reported at its new path, a stale bookmark made again, a dead one dropped) and
// forgotten with the folder. Other builds keep nothing. On a stand-in for Foundation's
// security-scoped bookmarks.

import Foundation
import XCTest
@testable import Jellyfesh

/// Bookmarks as calls: a bookmark is the folder's path, and `moves` and `stale` say how it
/// resolves.
private final class FakeBookmarks: FolderBookmarks {
    var moves: [String: String] = [:]
    var stale: Set<String> = []
    var dead: Set<String> = []
    private(set) var started: [String] = []
    private(set) var stopped: [String] = []
    private(set) var made: [String] = []

    func bookmark(for url: URL) throws -> Data {
        made.append(url.path)
        return Data(url.path.utf8)
    }

    func resolve(_ data: Data) throws -> (url: URL, stale: Bool) {
        let path = String(decoding: data, as: UTF8.self)
        if dead.contains(path) { throw CocoaError(.fileNoSuchFile) }
        return (URL(fileURLWithPath: moves[path] ?? path), stale.contains(path))
    }

    func startAccessing(_ url: URL) -> Bool {
        started.append(url.path)
        return true
    }

    func stopAccessing(_ url: URL) {
        stopped.append(url.path)
    }
}

@MainActor
final class FolderAccessTests: XCTestCase {
    private var suites: [String] = []

    override func tearDown() {
        for name in suites {
            UserDefaults.standard.removePersistentDomain(forName: name)
        }
        suites = []
        super.tearDown()
    }

    private func freshDefaults() -> UserDefaults {
        let name = "org.jellyfesh.tests.folders.\(UUID().uuidString)"
        suites.append(name)
        return UserDefaults(suiteName: name)!
    }

    func testAChosenFolderIsKeptAndHeld() {
        let defaults = freshDefaults()
        let bookmarks = FakeBookmarks()
        let access = FolderAccess(enabled: true, defaults: defaults, bookmarks: bookmarks)
        access.remember(URL(fileURLWithPath: "/Music/Ripped"))
        XCTAssertEqual(access.folders, ["/Music/Ripped"])
        XCTAssertEqual(bookmarks.started, ["/Music/Ripped"])

        // The next launch.
        let next = FakeBookmarks()
        let restored = FolderAccess(enabled: true, defaults: defaults, bookmarks: next)
        XCTAssertEqual(restored.restore(), [])
        XCTAssertEqual(next.started, ["/Music/Ripped"])
        XCTAssertEqual(next.made, [], "a fresh bookmark is not made again")

        restored.forget("/Music/Ripped")
        XCTAssertEqual(next.stopped, ["/Music/Ripped"])
        XCTAssertEqual(restored.folders, [])
    }

    func testRestoringFollowsMovesAndDropsDeadBookmarks() {
        let defaults = freshDefaults()
        let first = FolderAccess(enabled: true, defaults: defaults, bookmarks: FakeBookmarks())
        for path in ["/Music/A", "/Music/B", "/Music/C"] {
            first.remember(URL(fileURLWithPath: path))
        }
        let bookmarks = FakeBookmarks()
        bookmarks.moves = ["/Music/A": "/Volumes/Archive/A"]
        bookmarks.stale = ["/Music/B"]
        bookmarks.dead = ["/Music/C"]
        let access = FolderAccess(enabled: true, defaults: defaults, bookmarks: bookmarks)
        XCTAssertEqual(access.restore(), [FolderAccess.Move(from: "/Music/A", to: "/Volumes/Archive/A")])
        XCTAssertEqual(access.folders, ["/Music/B", "/Volumes/Archive/A"])
        XCTAssertEqual(Set(bookmarks.made), ["/Music/B", "/Volumes/Archive/A"], "stale and moved bookmarks are made again")
        XCTAssertEqual(Set(bookmarks.started), ["/Music/B", "/Volumes/Archive/A"])
    }

    func testOtherBuildsKeepNothing() {
        let defaults = freshDefaults()
        let bookmarks = FakeBookmarks()
        let access = FolderAccess(enabled: false, defaults: defaults, bookmarks: bookmarks)
        access.remember(URL(fileURLWithPath: "/Music"))
        XCTAssertEqual(access.restore(), [])
        access.forget("/Music")
        XCTAssertEqual(access.folders, [])
        XCTAssertEqual(bookmarks.made, [])
        XCTAssertEqual(bookmarks.started, [])
    }

    func testTheLibrarySendsTheChosenFolderAndForgetsARemovedOne() {
        let client = FakeCoreClient()
        let library = LibraryStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        let bookmarks = FakeBookmarks()
        library.folderAccess = FolderAccess(enabled: true, defaults: freshDefaults(), bookmarks: bookmarks)
        library.addFolder(URL(fileURLWithPath: "/Music"))
        XCTAssertEqual(client.sent.last, .addRoot(path: "/Music"))
        XCTAssertEqual(library.folderAccess?.folders, ["/Music"])
        library.removeFolder("/Music")
        XCTAssertEqual(client.sent.last, .removeRoot(path: "/Music"))
        XCTAssertEqual(bookmarks.stopped, ["/Music"])
        XCTAssertEqual(library.folderAccess?.folders, [])
    }

    func testTheDownloadsFolderIsKeptApartAndOnlyTheLatestIsHeld() {
        let defaults = freshDefaults()
        let bookmarks = FakeBookmarks()
        let music = FolderAccess(enabled: true, defaults: defaults, bookmarks: bookmarks)
        music.remember(URL(fileURLWithPath: "/Music/Jazz"))
        let access = FolderAccess(enabled: true, defaults: defaults, bookmarks: bookmarks, key: FolderAccess.downloadsKey)
        let store = DownloadsStore(client: FakeCoreClient(), tracker: CommandTracker(), toasts: ToastCenter())
        store.folderAccess = access
        store.keepAccess(to: URL(fileURLWithPath: "/Volumes/Disk/Downloads"))
        store.keepAccess(to: URL(fileURLWithPath: "/Volumes/Disk/Other"))
        XCTAssertEqual(access.folders, ["/Volumes/Disk/Other"])
        XCTAssertEqual(bookmarks.stopped, ["/Volumes/Disk/Downloads"], "the folder before it is let go")
        XCTAssertEqual(music.folders, ["/Music/Jazz"], "the music folders' bookmarks are kept apart")
    }
}
