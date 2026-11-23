// Albums: a lazy grid, sorted by the core in the order the person picks, a page at a time.

import SwiftUI

struct AlbumsView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryStore.self) private var library
    @Environment(\.look) private var look

    private var albums: AlbumsModel { model.pages.albums }

    /// Luminous: large covers, four to a row in a wide window; spaced by the look's density.
    private var columns: [GridItem] {
        [GridItem(.adaptive(minimum: 190, maximum: 260), spacing: look.spacing(28), alignment: .top)]
    }

    var body: some View {
        let list = albums.list
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s5) {
                PageHeader(title: String(localized: "Albums"), subtitle: subtitle) {
                    if !list.items.isEmpty {
                        sortMenu
                    }
                }
                LazyVGrid(columns: columns, alignment: .leading, spacing: look.spacing(Tokens.Space.s7)) {
                    ForEach(list.items) { album in
                        AlbumCard(album: album)
                            .onAppear { list.rowAppeared(album) }
                    }
                }
            }
            .pagePadding()
        }
        .overlay {
            if list.items.isEmpty, list.loadedRevision != nil || list.error != nil {
                EmptyState(title: "No Albums Yet", symbol: "square.stack",
                           message: "Add a music folder to see your albums here.", error: list.error) {
                    AddMusicFolderButton()
                }
            }
        }
        .navigationTitle(Text("Albums"))
        .task(id: library.revision) {
            await list.load(revision: library.revision)
        }
    }

    /// Sort, at the end of the title as on the web: the orders and Descending.
    private var sortMenu: some View {
        Menu {
            Picker("Sort By", selection: Binding(
                get: { albums.order },
                set: { order in Task { @MainActor in await albums.choose(order, revision: library.revision) } }
            )) {
                Text("Title").tag(AlbumOrder.title)
                Text("Artist").tag(AlbumOrder.artist)
                Text("Year").tag(AlbumOrder.year)
                Text("Recently Added").tag(AlbumOrder.added)
            }
            .pickerStyle(.inline)
            Toggle("Descending", isOn: Binding(
                get: { albums.descending },
                set: { value in Task { @MainActor in await albums.setOrder(albums.order, descending: value, revision: library.revision) } }
            ))
        } label: {
            Label("Sort", systemImage: "arrow.up.arrow.down")
        }
        .fixedSize()
        .help(Text("Sort Albums"))
    }

    private var subtitle: String? {
        guard let count = library.stats?.albums, count > 0 else { return nil }
        return CountText.albums(Int(count))
    }
}
