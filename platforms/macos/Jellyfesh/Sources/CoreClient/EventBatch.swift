// Batching core events for the main thread.

enum EventBatch {
    /// The batch with every slice event dropped that a later event of the same slice replaces.
    ///
    /// Slices arrive whole (docs/core-api.md), so only the last of each kind in a batch matters;
    /// the remaining events keep their order. Discrete events (search results, sign-in steps,
    /// failures, finished scans, …) are always kept.
    static func coalesced(_ events: [CoreEvent]) -> [CoreEvent] {
        guard events.count > 1 else { return events }
        var seen = Set<SliceKind>()
        var kept: [CoreEvent] = []
        kept.reserveCapacity(events.count)
        for event in events.reversed() {
            if let kind = SliceKind(event) {
                if seen.contains(kind) { continue }
                seen.insert(kind)
            }
            kept.append(event)
        }
        return kept.reversed()
    }

    private enum SliceKind: Hashable {
        case playback, queue, nowPlaying, lyrics, theme, settings, devices, sources, scanProgress, library

        init?(_ event: CoreEvent) {
            switch event {
            case .playback: self = .playback
            case .queue: self = .queue
            case .nowPlaying: self = .nowPlaying
            case .lyrics: self = .lyrics
            case .theme: self = .theme
            case .settings: self = .settings
            case .devices: self = .devices
            case .sources: self = .sources
            case .libraryChanged: self = .library
            case .scan(.progress): self = .scanProgress
            case .scan, .playlistCreated, .signIn, .searchResults, .browsePage, .buffering,
                 .commandFailed, .error:
                return nil
            }
        }
    }
}
