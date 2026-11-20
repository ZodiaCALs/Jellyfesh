// The comments on the song playing (ADR-0022, docs/ui.md "Comments"), for sources whose plugin
// shows them: the hot comments of the first page ("Popular"), then the latest ones, a page at a
// time as the list reaches its end. Read-only; the core never stores them.

import Foundation
import Observation

@MainActor
@Observable
final class CommentsModel {
    let item: ItemReference
    /// The hot comments ("Popular"), from the first page.
    private(set) var popular: [SourceComment] = []
    /// The latest comments, every page so far.
    private(set) var latest: [SourceComment] = []
    private(set) var total: UInt64?
    /// The source's credit, shown with the comments.
    private(set) var attribution: String?
    /// The page to ask for next; nil at the end.
    private(set) var next: String?
    private(set) var loading = false
    /// The first page has come.
    private(set) var loaded = false
    private(set) var error: String?

    init(item: ItemReference) {
        self.item = item
    }

    /// The comments of a song, when its source shows them: a source's song (not a remote
    /// session's) whose plugin declares comments.
    static func item(for track: TrackReference?, remote: Bool, showsComments: (String) -> Bool) -> ItemReference? {
        guard !remote, case .source(let song)? = track, showsComments(song.plugin) else { return nil }
        return .source(plugin: song.plugin, kind: .track, id: song.id)
    }

    /// The first page (again, after a failure).
    func load(_ answers: CoreAnswers) {
        ask(page: nil, answers)
    }

    /// The next page, once the list reaches its end.
    func loadMore(_ answers: CoreAnswers) {
        guard let next, !loading else { return }
        ask(page: next, answers)
    }

    private func ask(page: String?, _ answers: CoreAnswers) {
        loading = true
        error = nil
        let id = answers.comments(item, page: page, onAnswer: { [weak self] answer in
            self?.apply(answer, first: page == nil)
        }, onFailure: { [weak self] message in
            self?.loading = false
            self?.error = message
        })
        if id == 0 {
            loading = false
            error = String(localized: "The player has stopped.")
        }
    }

    private func apply(_ answer: CommentsAnswer, first: Bool) {
        loading = false
        if let message = answer.error?.trimmingCharacters(in: .whitespacesAndNewlines), !message.isEmpty {
            error = message
            return
        }
        if first {
            popular = answer.hot
            latest = []
            loaded = true
        }
        let known = Set(latest.map(\.id))
        latest += answer.comments.filter { !known.contains($0.id) }
        total = answer.total ?? total
        attribution = answer.attribution ?? attribution
        next = answer.next
    }
}
