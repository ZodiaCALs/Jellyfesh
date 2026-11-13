// Albums: a lazy grid, sorted by the core in the order the person picks, a page at a time.

import SwiftUI

struct AlbumsView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library

    private var albums: AlbumsModel { model.pages.albums }

    private let columns = [
        GridItem(.adaptive(minimum: 150, maximum: 220), spacing: Tokens.Space.s5, alignment: .top),
    ]

    var body: some View {
        let list = albums.list
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s5) {
                PageHeader(title: String(localized: "Albums"), subtitle: subtitle)
                LazyVGrid(columns: columns, alignment: .leading, spacing: Tokens.Space.s6) {
                    ForEach(list.items) { album in
                        AlbumCard(album: album)
                            .onAppear { list.rowAppeared(album) }
                    }
                }
            }
            .padding(.horizontal, Tokens.Space.s7)
            .padding(.vertical, Tokens.Space.s6)
        }
        .overlay {
            if list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "No Albums", symbol: "square.stack",
                           message: "Albums appear here once your music folders are scanned.", error: list.error)
            }
        }
        .navigationTitle(Text("Albums"))
        .toolbar {
            ToolbarItem {
                Menu {
                    Picker("Sort By", selection: Binding(
                        get: { albums.order },
                        set: { order in Task { @MainActor in await albums.setOrder(order, descending: albums.descending, revision: library.revision) } }
                    )) {
                        Text("Title").tag(AlbumOrder.title)
                        Text("Artist").tag(AlbumOrder.artist)
                        Text("Year").tag(AlbumOrder.year)
                        Text("Date Added").tag(AlbumOrder.added)
                    }
                    .pickerStyle(.inline)
                    Toggle("Descending", isOn: Binding(
                        get: { albums.descending },
                        set: { value in Task { @MainActor in await albums.setOrder(albums.order, descending: value, revision: library.revision) } }
                    ))
                } label: {
                    Label("Sort", systemImage: "arrow.up.arrow.down")
                }
                .help(Text("Sort albums"))
            }
        }
        .task(id: library.revision) {
            await list.load(revision: library.revision)
        }
    }

    private var subtitle: String? {
        guard let count = library.stats?.albums, count > 0 else { return nil }
        return CountText.albums(Int(count))
    }
}
