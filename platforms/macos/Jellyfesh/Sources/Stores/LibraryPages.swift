// The sidebar sections' lists, kept for the window's life so switching sections keeps what was
// loaded. Each queries the core again when the library revision moves; the order of every list
// is the core's (ADR-0001: no sorting in the shell).

import Foundation
import Observation

/// Home: what was added and played recently.
@MainActor
@Observable
final class HomeModel {
    static let shelfSize = 12
    static let listSize = 10

    private let client: CoreClient
    private(set) var stats: LibraryStatistics?
    private(set) var recentAlbums: [AlbumSummary] = []
    private(set) var recentlyPlayed: [SongRow] = []
    private(set) var mostPlayed: [SongRow] = []
    private(set) var loaded = false
    @ObservationIgnored private var loadedRevision: UInt64?

    init(client: CoreClient) {
        self.client = client
    }

    func load(revision: UInt64) async {
        guard loadedRevision != revision else { return }
        loadedRevision = revision
        async let stats = client.stats()
        // The core orders albums by when they were added; newest first.
        async let albums = client.albums(order: .added, descending: true, offset: 0, limit: Self.shelfSize)
        async let played = client.recentlyPlayed(limit: Self.listSize)
        async let most = client.mostPlayed(limit: Self.listSize)
        self.stats = try? await stats
        recentAlbums = (try? await albums) ?? []
        recentlyPlayed = (try? await played) ?? []
        mostPlayed = (try? await most) ?? []
        loaded = true
    }
}

/// Albums: a grid in the order the person picks, sorted by the core.
@MainActor
@Observable
final class AlbumsModel {
    private let client: CoreClient
    private(set) var order: AlbumOrder = .title
    private(set) var descending = false
    private(set) var list: PagedList<AlbumSummary>

    init(client: CoreClient) {
        self.client = client
        list = Self.makeList(client: client, order: .title, descending: false)
    }

    func setOrder(_ order: AlbumOrder, descending: Bool, revision: UInt64) async {
        guard order != self.order || descending != self.descending else { return }
        self.order = order
        self.descending = descending
        list = Self.makeList(client: client, order: order, descending: descending)
        await list.load(revision: revision)
    }

    private static func makeList(client: CoreClient, order: AlbumOrder, descending: Bool) -> PagedList<AlbumSummary> {
        PagedList(pageSize: 120) { offset, limit in
            try await client.albums(order: order, descending: descending, offset: offset, limit: limit)
        }
    }
}

/// Songs: every track, sorted by the core.
@MainActor
@Observable
final class SongsModel {
    private let client: CoreClient
    private(set) var sort: SongSort = .artist
    private(set) var descending = false
    private(set) var list: PagedList<Song>

    init(client: CoreClient) {
        self.client = client
        list = Self.makeList(client: client, sort: .artist, descending: false)
    }

    func setSort(_ sort: SongSort, descending: Bool, revision: UInt64) async {
        guard sort != self.sort || descending != self.descending else { return }
        self.sort = sort
        self.descending = descending
        list = Self.makeList(client: client, sort: sort, descending: descending)
        await list.load(revision: revision)
    }

    /// The query of the loaded order, to play the list from a row.
    var query: SongQuery {
        SongQuery(offset: 0, limit: 0, sort: sort, descending: descending)
    }

    private static func makeList(client: CoreClient, sort: SongSort, descending: Bool) -> PagedList<Song> {
        PagedList(pageSize: 200) { offset, limit in
            try await client.songs(SongQuery(offset: offset, limit: limit, sort: sort, descending: descending))
        }
    }
}

/// The lists behind the sidebar sections.
@MainActor
final class LibraryPages {
    let home: HomeModel
    let albums: AlbumsModel
    let artists: PagedList<ArtistSummary>
    let songs: SongsModel
    let favorites: PagedList<TrackRowItem>

    init(client: CoreClient) {
        home = HomeModel(client: client)
        albums = AlbumsModel(client: client)
        songs = SongsModel(client: client)
        artists = PagedList(pageSize: 200) { offset, limit in
            try await client.artists(offset: offset, limit: limit)
        }
        favorites = PagedList(pageSize: 200) { offset, limit in
            try await client.favorites(offset: offset, limit: limit).map(TrackRowItem.init)
        }
    }
}
