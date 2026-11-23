// Downloads (ADR-0025, docs/ui.md "Downloads"): downloads in progress, then the downloaded albums
// and songs, with the space used. The core's download queue has not reached this build yet, so
// there is nothing to list: the page says so as an empty page does.

import SwiftUI

struct DownloadsView: View {
    var body: some View {
        ScrollView {
            PageHeader(title: String(localized: "Downloads"))
                .pagePadding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .overlay {
            EmptyState(title: "No Downloads Yet", symbol: "arrow.down.circle",
                       message: "Download albums and playlists from your servers to play them offline.")
        }
        .navigationTitle(Text("Downloads"))
    }
}
