// Where the window is: the sidebar section, the pages pushed on it, and Now Playing.

import Observation
import SwiftUI

enum SidebarItem: Hashable {
    case home, albums, artists, songs, playlists, favorites, search, sources
    case playlist(Int64)

    static let library: [SidebarItem] = [.home, .albums, .artists, .songs, .playlists, .favorites]

    var title: LocalizedStringKey {
        switch self {
        case .home: return "Home"
        case .albums: return "Albums"
        case .artists: return "Artists"
        case .songs: return "Songs"
        case .playlists: return "Playlists"
        case .favorites: return "Favourites"
        case .search: return "Search"
        case .sources: return "Sources"
        case .playlist: return "Playlist"
        }
    }

    var symbol: String {
        switch self {
        case .home: return "house"
        case .albums: return "square.stack"
        case .artists: return "music.mic"
        case .songs: return "music.note"
        case .playlists: return "music.note.list"
        case .favorites: return "heart"
        case .search: return "magnifyingglass"
        case .sources: return "antenna.radiowaves.left.and.right"
        case .playlist: return "music.note.list"
        }
    }
}

/// A page pushed on a section.
enum Route: Hashable {
    case album(AlbumSummary)
    case artist(ArtistSummary)
    case playlist(Int64)
    /// A source's container (`node` nil is its root); `link` opens it in its service.
    case browse(plugin: String, node: String?, title: String, link: String? = nil)
    /// Apple Music's catalogue (a player the app provides, ADR-0018 item 5).
    case appleMusic
}

enum NowPlayingPanel: Hashable, CaseIterable {
    case lyrics, queue, info
}

@MainActor
@Observable
final class Router {
    private(set) var selection: SidebarItem = .home
    var path: [Route] = []
    private(set) var nowPlayingShown = false
    var panel: NowPlayingPanel = .lyrics
    /// Bumped to move keyboard focus into the search field.
    private(set) var searchFocusRequest = 0

    func select(_ item: SidebarItem) {
        guard item != selection || !path.isEmpty else { return }
        selection = item
        path = []
    }

    func open(_ route: Route) {
        path.append(route)
    }

    /// Back a page (Esc): false at a section's own page, where there is nothing to go back to.
    @discardableResult
    func back() -> Bool {
        guard !path.isEmpty else { return false }
        path.removeLast()
        return true
    }

    /// Opens or closes Now Playing: it rises from the player (`player.open-now-playing`) and
    /// sinks back on Exit (`player.close-now-playing`); with `reduceMotion`, a short fade.
    func showNowPlaying(_ shown: Bool, panel: NowPlayingPanel? = nil, reduceMotion: Bool) {
        if let panel { self.panel = panel }
        guard shown != nowPlayingShown else { return }
        let animation = reduceMotion ? Motion.shortFade : (shown ? Motion.openNowPlaying : Motion.closeNowPlaying)
        withAnimation(animation) {
            nowPlayingShown = shown
        }
    }

    func focusSearch() {
        showNowPlaying(false, reduceMotion: true)
        select(.search)
        searchFocusRequest += 1
    }
}
