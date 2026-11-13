// The lyrics of the current track as the core reports them (`Lyrics` slice): the document to show
// and the flat timeline the shared cursor port searches each frame.

import Foundation
import Observation

@MainActor
@Observable
final class LyricsStore {
    private(set) var state: LyricsState?

    func apply(_ value: LyricsState?) {
        state = value
    }

    /// The lyrics for the queue entry `uid`, or nil while the core has none for it yet.
    func document(for uid: UInt64?) -> LyricsDocument? {
        guard let state, let uid, state.uid == uid else { return nil }
        return state.document
    }

    /// The timeline for the queue entry `uid`, when its lyrics are timed.
    func timeline(for uid: UInt64?) -> LyricsTimeline? {
        guard let state, let uid, state.uid == uid, let document = state.document,
              document.timing != .unsynced
        else { return nil }
        return state.timeline
    }
}
