// Music folders in the Mac App Store build, which runs in the App Sandbox: a folder chosen in the
// open panel stays reachable only while the app holds the panel's grant. A security-scoped
// bookmark keeps the grant for later launches (Jellyfesh-AppStore.entitlements has
// com.apple.security.files.bookmarks.app-scope). At launch, before the core opens the library,
// every folder's bookmark is resolved and its access held while the app runs. A folder that moved
// is passed to the core at its new path, and a stale bookmark is made again. Other builds are not
// sandboxed and keep nothing. The file system calls are behind `FolderBookmarks`, so tests use a
// stand-in.

import Foundation

/// Security-scoped bookmarks, or a stand-in in tests.
protocol FolderBookmarks {
    func bookmark(for url: URL) throws -> Data
    /// The folder a bookmark names now, and whether the bookmark should be made again.
    func resolve(_ data: Data) throws -> (url: URL, stale: Bool)
    func startAccessing(_ url: URL) -> Bool
    func stopAccessing(_ url: URL)
}

/// Foundation's security-scoped bookmarks.
struct SecurityScopedBookmarks: FolderBookmarks {
    func bookmark(for url: URL) throws -> Data {
        try url.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil)
    }

    func resolve(_ data: Data) throws -> (url: URL, stale: Bool) {
        var stale = false
        let url = try URL(resolvingBookmarkData: data, options: .withSecurityScope, relativeTo: nil,
                          bookmarkDataIsStale: &stale)
        return (url, stale)
    }

    func startAccessing(_ url: URL) -> Bool {
        url.startAccessingSecurityScopedResource()
    }

    func stopAccessing(_ url: URL) {
        url.stopAccessingSecurityScopedResource()
    }
}

@MainActor
final class FolderAccess {
    /// Where the bookmarks are kept: folder path → bookmark data.
    static let defaultsKey = "MusicFolderBookmarks"

    /// A folder whose bookmark now names another path.
    struct Move: Equatable {
        var from: String
        var to: String
    }

    private let enabled: Bool
    private let defaults: UserDefaults
    private let bookmarks: FolderBookmarks
    /// The folders whose access is held, by path.
    private var held: [String: URL] = [:]

    /// `enabled` is true for sandboxed builds (the Mac App Store's).
    init(enabled: Bool, defaults: UserDefaults = .standard, bookmarks: FolderBookmarks = SecurityScopedBookmarks()) {
        self.enabled = enabled
        self.defaults = defaults
        self.bookmarks = bookmarks
    }

    /// The paths that have a bookmark.
    var folders: [String] { stored.keys.sorted() }

    /// Keeps a folder chosen in the open panel reachable on later launches.
    func remember(_ url: URL) {
        guard enabled, let data = try? bookmarks.bookmark(for: url) else { return }
        var all = stored
        all[url.path] = data
        stored = all
        hold(url, as: url.path)
    }

    /// At launch: resolves every bookmark and holds its folder's access. Returns the folders that
    /// moved, to pass to the core at their new paths. Bookmarks that no longer resolve are dropped.
    func restore() -> [Move] {
        guard enabled else { return [] }
        var all = stored
        var moves: [Move] = []
        for (path, data) in all.sorted(by: { $0.key < $1.key }) {
            guard let resolved = try? bookmarks.resolve(data) else {
                all[path] = nil
                continue
            }
            let now = resolved.url.path
            if now != path {
                all[path] = nil
                moves.append(Move(from: path, to: now))
            }
            if resolved.stale || now != path, let fresh = try? bookmarks.bookmark(for: resolved.url) {
                all[now] = fresh
            } else if now != path {
                all[now] = data
            }
            hold(resolved.url, as: now)
        }
        stored = all
        return moves
    }

    /// A folder left the library: its access and bookmark go.
    func forget(_ path: String) {
        guard enabled else { return }
        if let url = held.removeValue(forKey: path) {
            bookmarks.stopAccessing(url)
        }
        var all = stored
        all[path] = nil
        stored = all
    }

    private func hold(_ url: URL, as path: String) {
        guard held[path] == nil, bookmarks.startAccessing(url) else { return }
        held[path] = url
    }

    private var stored: [String: Data] {
        get { (defaults.dictionary(forKey: Self.defaultsKey) as? [String: Data]) ?? [:] }
        set { defaults.set(newValue, forKey: Self.defaultsKey) }
    }
}
