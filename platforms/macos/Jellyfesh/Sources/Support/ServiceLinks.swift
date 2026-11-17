// Links that open an item in its service (ADR-0018, docs/ui.md "Links and remote playback"): the
// core fills them in from the source's own templates. Every item with one offers Open in
// {Service}; an item that cannot play here opens it as its main action.

import AppKit
import SwiftUI

enum ServiceLink {
    /// The link as a URL to open: `https` only, as the core promises.
    static func url(_ link: String?) -> URL? {
        guard let link = link?.trimmingCharacters(in: .whitespacesAndNewlines), !link.isEmpty,
              let url = URL(string: link), url.scheme?.lowercased() == "https", url.host != nil
        else { return nil }
        return url
    }

    /// Whether an item's main action (click, Return, VoiceOver's default action) opens its link:
    /// it cannot play here and has one.
    static func isMainAction(canPlay: Bool, link: String?) -> Bool {
        !canPlay && url(link) != nil
    }

    /// Opens the link in the browser or the service's app.
    static func open(_ link: String?) {
        guard let url = url(link) else { return }
        _ = NSWorkspace.shared.open(url)
    }
}

/// Open in {Service} (the source's name: Open in QQ Music), for an item with a link.
struct OpenInServiceButton: View {
    var link: String?
    /// The source the item comes from; its name names the service.
    var source: String?

    @Environment(SourcesStore.self) private var sources

    var body: some View {
        if ServiceLink.url(link) != nil, let source {
            Button("Open in \(sources.name(of: source))") {
                ServiceLink.open(link)
            }
        }
    }
}
