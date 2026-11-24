// Where the window is: the place shown (a sidebar or tab place, ADR-0023), the pages pushed on it,
// and Now Playing.

import Observation
import SwiftUI

/// A place of the sidebar panel (docs/ui.md, "Navigation"), in its order.
enum SidebarItem: Hashable, CaseIterable {
    case home, albums, artists, songs, playlists, favorites, downloads, search, sources
    /// Settings, a page in the main window (ADR-0023), at the panel's foot.
    case settings

    /// The places the phone layout's Library tab switches between.
    static let libraryPlaces: [SidebarItem] = [.albums, .artists, .songs, .playlists, .favorites, .downloads]

    /// A place of the Library tab.
    var isLibraryPlace: Bool { Self.libraryPlaces.contains(self) }

    var title: LocalizedStringKey {
        switch self {
        case .home: return "Home"
        case .albums: return "Albums"
        case .artists: return "Artists"
        case .songs: return "Songs"
        case .playlists: return "Playlists"
        case .favorites: return "Favourites"
        case .downloads: return "Downloads"
        case .search: return "Search"
        case .sources: return "Sources"
        case .settings: return "Settings"
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
        case .downloads: return "arrow.down.circle"
        case .search: return "magnifyingglass"
        case .sources: return "antenna.radiowaves.left.and.right"
        case .settings: return "slider.horizontal.3"
        }
    }

    /// The place for one of the look's places.
    init(_ place: LookPlace) {
        switch place {
        case .home: self = .home
        case .albums: self = .albums
        case .artists: self = .artists
        case .songs: self = .songs
        case .playlists: self = .playlists
        case .favourites: self = .favorites
        case .search: self = .search
        case .sources: self = .sources
        }
    }

    /// The sidebar panel's rows for the look (docs/ui.md, "Navigation"): Home and the library's
    /// places, a line, then Search and Sources. A customized look (ADR-0020) lists the places it
    /// shows in its order, without the line, as Jellyfesh Web does. Downloads, which the look does
    /// not place, follows the last of the library's places. Open Files… and Settings stay at the
    /// panel's foot.
    static func panelEntries(for look: LookStyle) -> [PanelEntry] {
        var items = look.sidebar.map(SidebarItem.init)
        let lastLibrary = items.lastIndex { $0 != .search && $0 != .sources }
        items.insert(.downloads, at: lastLibrary.map { $0 + 1 } ?? items.endIndex)
        var entries = items.map(PanelEntry.place)
        if !look.customized, let first = items.firstIndex(where: { $0 == .search || $0 == .sources }), first > 0 {
            entries.insert(.separator, at: first)
        }
        return entries
    }
}

/// A row of the sidebar panel: a place, or the line between the library and Search and Sources.
enum PanelEntry: Hashable {
    case place(SidebarItem)
    case separator
}

/// A tab of the phone layout (ADR-0023: windows up to 760 pt wide).
enum PhoneTab: Hashable, CaseIterable {
    case home, library, search, sources, settings

    var title: LocalizedStringKey {
        switch self {
        case .home: return "Home"
        case .library: return "Library"
        case .search: return "Search"
        case .sources: return "Sources"
        case .settings: return "Settings"
        }
    }

    var symbol: String {
        switch self {
        case .home: return SidebarItem.home.symbol
        case .library: return "books.vertical"
        case .search: return SidebarItem.search.symbol
        case .sources: return SidebarItem.sources.symbol
        case .settings: return SidebarItem.settings.symbol
        }
    }

    /// The tabs in the look's order: Home, Library (Albums … Favourites, and Downloads), Search and
    /// Sources as the look places them, then Settings (Jellyfesh Web's phone tabs).
    static func tabs(for look: LookStyle) -> [PhoneTab] {
        var tabs: [PhoneTab] = []
        for place in look.sidebar {
            let tab: PhoneTab
            switch place {
            case .home: tab = .home
            case .albums, .artists, .songs, .playlists, .favourites: tab = .library
            case .search: tab = .search
            case .sources: tab = .sources
            }
            if !tabs.contains(tab) { tabs.append(tab) }
        }
        return tabs + [.settings]
    }
}

/// A page pushed on a section.
enum Route: Hashable {
    case album(AlbumSummary)
    case artist(ArtistSummary)
    case playlist(Int64)
    /// A source's container (`node` nil is its root); `link` opens it in its service. `item` is the
    /// container as the list it was chosen from showed it (its kind, artist and artwork), when known.
    /// `songsListed` false: a source playlist whose songs its service lists only in its own app.
    case browse(plugin: String, node: String?, title: String, link: String? = nil, item: SourceResult? = nil,
                songsListed: Bool = true)
    /// A source's Discover page (ADR-0022): its shelves, with Browse for its own folders.
    case discover(plugin: String, title: String)
    /// Apple Music's catalogue (a player the app provides, ADR-0018 item 5).
    case appleMusic
}

enum NowPlayingPanel: Hashable, CaseIterable {
    /// Comments shows for sources whose plugin shows comments (ADR-0022).
    case lyrics, queue, info, comments
}

@MainActor
@Observable
final class Router {
    private(set) var selection: SidebarItem = .home
    /// The place the Library tab shows: the last of Albums … Downloads shown.
    private(set) var libraryPlace: SidebarItem = .albums
    var path: [Route] = []
    private(set) var nowPlayingShown = false
    var panel: NowPlayingPanel = .lyrics
    /// Bumped to move keyboard focus into the search field.
    private(set) var searchFocusRequest = 0
    /// Get Info's sheet, while it is open.
    var infoRequest: InfoRequest?

    func select(_ item: SidebarItem) {
        if item.isLibraryPlace { libraryPlace = item }
        guard item != selection || !path.isEmpty else { return }
        selection = item
        path = []
    }

    /// The tab whose place shows (the phone layout).
    var tab: PhoneTab {
        switch selection {
        case .home: return .home
        case .search: return .search
        case .sources: return .sources
        case .settings: return .settings
        case .albums, .artists, .songs, .playlists, .favorites, .downloads: return .library
        }
    }

    /// Shows a tab's place: the Library tab shows the library's place shown last.
    func selectTab(_ tab: PhoneTab) {
        let place: SidebarItem
        switch tab {
        case .home: place = .home
        case .library: place = libraryPlace
        case .search: place = .search
        case .sources: place = .sources
        case .settings: place = .settings
        }
        select(place)
    }

    func open(_ route: Route) {
        path.append(route)
    }

    /// A playlist's page, on Playlists (a playlist made here opens once the core names it).
    func openPlaylist(_ id: Int64) {
        select(.playlists)
        open(.playlist(id))
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
