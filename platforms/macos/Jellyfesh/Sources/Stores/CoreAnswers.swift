// Answers to the commands that ask the core for a page (docs/core-api.md, "Discover", "Details and
// comments"): each answer carries the request id `send` returned, and goes to whoever asked. A
// failed command (the core answering "not yet" while v0.0.7 is being built, a source that
// failed) goes to the same asker.

import Foundation

@MainActor
final class CoreAnswers {
    private let client: CoreClient
    private let tracker: CommandTracker

    private var discoverHandlers: [CommandID: (DiscoverAnswer) -> Void] = [:]
    private var detailsHandlers: [CommandID: (DetailsAnswer) -> Void] = [:]
    private var commentsHandlers: [CommandID: (CommentsAnswer) -> Void] = [:]
    private var order: [CommandID] = []
    private let capacity = 64

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    /// A source's Discover page (`library` for the library's).
    @discardableResult
    func discover(source: String, onAnswer: @escaping (DiscoverAnswer) -> Void,
                  onFailure: @escaping (String) -> Void) -> CommandID {
        let id = ask(.discover(source: source), onFailure: onFailure)
        if id != 0 { discoverHandlers[id] = onAnswer }
        return id
    }

    /// More about an item.
    @discardableResult
    func details(_ item: ItemReference, onAnswer: @escaping (DetailsAnswer) -> Void,
                 onFailure: @escaping (String) -> Void) -> CommandID {
        let id = ask(.details(item), onFailure: onFailure)
        if id != 0 { detailsHandlers[id] = onAnswer }
        return id
    }

    /// A page of an item's comments: `page` nil is the first.
    @discardableResult
    func comments(_ item: ItemReference, page: String?, onAnswer: @escaping (CommentsAnswer) -> Void,
                  onFailure: @escaping (String) -> Void) -> CommandID {
        let id = ask(.comments(item, page: page), onFailure: onFailure)
        if id != 0 { commentsHandlers[id] = onAnswer }
        return id
    }

    func apply(discover answer: DiscoverAnswer) {
        discoverHandlers.removeValue(forKey: answer.request)?(answer)
    }

    func apply(details answer: DetailsAnswer) {
        detailsHandlers.removeValue(forKey: answer.request)?(answer)
    }

    func apply(comments answer: CommentsAnswer) {
        commentsHandlers.removeValue(forKey: answer.request)?(answer)
    }

    private func ask(_ command: CoreCommand, onFailure: @escaping (String) -> Void) -> CommandID {
        let id = client.send(command)
        tracker.watch(id) { [weak self] message in
            self?.forget(id)
            onFailure(message)
        }
        guard id != 0 else { return 0 }
        order.append(id)
        if order.count > capacity {
            forget(order.removeFirst())
        }
        return id
    }

    private func forget(_ id: CommandID) {
        discoverHandlers[id] = nil
        detailsHandlers[id] = nil
        commentsHandlers[id] = nil
        order.removeAll { $0 == id }
    }
}
