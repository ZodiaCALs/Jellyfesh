// A source's Discover page (ADR-0022, docs/ui.md "Discover"): the shelves the core answers with,
// from the source's plugin or made by the core from its suggestions and the top of Browse. Asked
// again with Try Again after a failure. The words of the shelves the core makes come from their
// ids (`DiscoverText`).

import Foundation
import Observation

@MainActor
@Observable
final class DiscoverModel {
    let source: String
    private(set) var shelves: [DiscoverShelf] = []
    /// An answer has come (with shelves, or with none).
    private(set) var loaded = false
    private(set) var loading = false
    /// Why the page could not be had: the core's answer, or the command's failure.
    private(set) var error: String?

    init(source: String) {
        self.source = source
    }

    /// Asks the core for the page (again, after a failure).
    func load(_ answers: CoreAnswers) {
        loading = true
        error = nil
        let id = answers.discover(source: source, onAnswer: { [weak self] answer in
            guard let self else { return }
            self.loading = false
            self.loaded = true
            self.shelves = answer.shelves.filter { !$0.items.isEmpty }
            let message = answer.error?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            self.error = message.isEmpty ? nil : message
        }, onFailure: { [weak self] message in
            self?.loading = false
            self?.error = message
        })
        if id == 0 {
            loading = false
            error = String(localized: "The player has stopped.")
        }
    }
}

/// What a Discover page says, as rules apart from the views.
enum DiscoverText {
    /// A shelf's title: the core's own shelves are worded by their ids ("For You", the source's
    /// name for the rest of its top level, and the library's), others keep the source's words.
    static func title(_ shelf: DiscoverShelf, sourceName: String) -> String {
        switch shelf.id {
        case "for_you": return String(localized: "For You")
        case "browse": return sourceName
        case "recently_added": return String(localized: "Recently Added")
        case "most_played": return String(localized: "Most Played")
        case "rediscover": return String(localized: "Rediscover")
        default: return shelf.title
        }
    }

    /// The tracks of a shelf that play here, and where `item` is among them; nil when it is not
    /// one of them (it cannot play here).
    static func queue(of shelf: DiscoverShelf, from item: SourceResult) -> (tracks: [TrackReference], start: Int)? {
        let playable = shelf.items.filter { $0.kind == .track && $0.playable && $0.track != nil }
        guard let start = playable.firstIndex(where: { $0.id == item.id }) else { return nil }
        return (playable.compactMap(\.track), start)
    }
}
