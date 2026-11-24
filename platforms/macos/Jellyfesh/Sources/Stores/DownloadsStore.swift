// Downloads (ADR-0025, docs/ui.md "Downloads"): the core's queue and the songs it has downloaded,
// for the Downloads page, Settings › Downloads and the Download menu items. The core downloads,
// resumes, retries with backoff, waits while offline and keeps the files; this keeps what it
// reports, words it, and sends the person's choices.

import Foundation
import Observation

/// Finished downloads of one album, as the Downloads page lists them.
struct DownloadedAlbum: Identifiable, Hashable {
    var title: String?
    var artist: String?
    var entries: [DownloadEntry]

    var id: String { entries.first.map { String($0.id) } ?? "" }
}

@MainActor
@Observable
final class DownloadsStore {
    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter

    private(set) var state: DownloadsState = .empty

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
    }

    func apply(_ state: DownloadsState) {
        self.state = state
    }

    // MARK: What the pages show

    /// Downloads in progress (waiting, downloading or failed), in the queue's order.
    var inProgress: [DownloadEntry] {
        state.items.filter { $0.phase != .done }
    }

    /// The finished downloads by album, in the core's order.
    var albums: [DownloadedAlbum] {
        var albums: [DownloadedAlbum] = []
        for entry in state.items where entry.phase == .done {
            if let last = albums.indices.last, albums[last].title == entry.track.album {
                albums[last].entries.append(entry)
            } else {
                albums.append(DownloadedAlbum(title: entry.track.album, artist: entry.track.artist, entries: [entry]))
            }
        }
        return albums
    }

    /// "1.2 GB": the space the finished downloads take.
    var spaceUsed: String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: state.bytesUsed), countStyle: .file)
    }

    /// How far a download is, 0–1, when its size is known.
    nonisolated static func progress(_ entry: DownloadEntry) -> Double? {
        guard let total = entry.bytesTotal, total > 0 else { return nil }
        return min(max(Double(entry.bytesDone) / Double(total), 0), 1)
    }

    /// What a finished download plays: its library track once the library has it, else the
    /// source's song, which plays the file too (ADR-0025 §4).
    nonisolated static func track(_ entry: DownloadEntry) -> TrackReference {
        if let id = entry.libraryID { return .library(id: id) }
        return .source(entry.track)
    }

    /// Why a download failed, in the app's words.
    nonisolated static func problem(_ problem: DownloadProblem) -> String {
        switch problem {
        case .notAllowed(let service): return String(localized: "Your \(service) server doesn't allow downloads for this account.")
        case .notOffered(let service): return String(localized: "\(service) doesn't offer downloads.")
        case .network(let message), .storage(let message), .other(let message): return message
        }
    }

    /// Why downloads wait.
    nonisolated static func paused(_ pause: DownloadsPause) -> String {
        switch pause {
        case .offline: return String(localized: "Downloads wait until you're online.")
        case .metered: return String(localized: "Downloads wait for Wi-Fi.")
        }
    }

    // MARK: The person's choices

    /// Download: a source's song, album or playlist.
    func download(_ request: DownloadRequest) {
        watch(client.send(.download(request)))
    }

    /// Cancel: stops an unfinished download and forgets it.
    func cancel(_ id: UInt64) {
        watch(client.send(.cancelDownload(id: id)))
    }

    /// Try Again, for a failed download.
    func retry(_ id: UInt64) {
        watch(client.send(.retryDownload(id: id)))
    }

    /// Remove Download… (asked first): the files of a song, or of an album's songs, are deleted.
    func remove(_ ids: [UInt64]) {
        for id in ids {
            watch(client.send(.removeDownload(id: id)))
        }
    }

    /// Remove All Downloads… (asked first).
    func removeAll() {
        watch(client.send(.removeAllDownloads))
    }

    private func watch(_ id: CommandID) {
        tracker.watch(id) { [weak self] message in
            self?.toasts.show(isNotAvailableMessage(message)
                ? String(localized: "Downloads aren't available in this build yet.") : message)
        }
    }
}
