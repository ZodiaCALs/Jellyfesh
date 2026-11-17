// Apple Music as a player the app provides (ADR-0010, ADR-0018 item 5): the core drives it like a
// remote source through `AppPlayer`, on a remote session's own thread, and each call may block.
// MusicKit's player lives on the main actor and answers asynchronously, so every call hops there
// and the core's thread waits for the answer (never the main thread, never longer than a few
// seconds). What MusicKit itself does sits behind `AppleMusicControls` (MusicKitService.swift), so
// the ids, the requests and the state mapping are tested without MusicKit.

import Foundation

/// What an Apple Music id names: catalogue albums have numeric ids, catalogue playlists start
/// with `pl.`.
enum AppleMusicContainer: Equatable, Sendable {
    case album(String)
    case playlist(String)

    init?(id: String) {
        let id = id.trimmingCharacters(in: .whitespacesAndNewlines)
        if id.hasPrefix("pl."), id.count > 3 {
            self = .playlist(id)
        } else if AppleMusicIDs.isCatalogNumber(id) {
            self = .album(id)
        } else {
            return nil
        }
    }
}

enum AppleMusicIDs {
    /// The source id the player is registered under, and the `plugin` of its tracks.
    static let source = "apple-music"
    /// The one device: this Mac.
    static let device = "this-mac"

    /// A catalogue id: digits only (songs and albums).
    static func isCatalogNumber(_ id: String) -> Bool {
        !id.isEmpty && id.utf8.allSatisfy { $0 >= 0x30 && $0 <= 0x39 }
    }
}

/// What to play: catalogue songs from one of them, or an album or playlist from its track `start`.
enum AppleMusicPlayRequest: Equatable, Sendable {
    case songs(ids: [String], start: Int)
    case album(id: String, start: Int)
    case playlist(id: String, start: Int)

    /// The core's call as a request: a context names an album or a playlist (its kind is not
    /// reliable, the core sends what the person chose as an album); otherwise the tracks play.
    /// Nil when there is nothing Apple Music can play.
    static func from(context: ExternalMedia?, tracks: [ExternalMedia], start: Int) -> AppleMusicPlayRequest? {
        if let context {
            switch AppleMusicContainer(id: context.id) {
            case .album(let id)?: return .album(id: id, start: max(start, 0))
            case .playlist(let id)?: return .playlist(id: id, start: max(start, 0))
            case nil: return nil
            }
        }
        var ids: [String] = []
        var first = 0
        for (index, track) in tracks.enumerated() where AppleMusicIDs.isCatalogNumber(track.id) {
            if index <= start { first = ids.count }
            ids.append(track.id)
        }
        guard !ids.isEmpty else { return nil }
        return .songs(ids: ids, start: min(max(first, 0), ids.count - 1))
    }
}

/// MusicKit's player state as plain values (MusicKitService.swift reads it), so the mapping to the
/// core's report is testable.
struct AppleMusicPlayerSnapshot: Equatable, Sendable {
    enum Status: Equatable, Sendable {
        case stopped, playing, paused, interrupted, seeking
    }

    /// The queue's current entry.
    struct Entry: Equatable, Sendable {
        var id: String
        var title: String
        var artist: String?
        var album: String?
        var durationSeconds: Double?
        var artworkURL: String?
    }

    var status: Status
    var entry: Entry?
    /// `playbackTime`: seconds into the entry; NaN when unknown.
    var positionSeconds: Double
    var shuffle: Bool
    var repeatMode: RepeatOption
}

enum AppleMusicMapping {
    /// The one device Apple Music plays on. It has no volume Jellyfesh may change (MusicKit has
    /// no volume API), so the volume control hides.
    static func device(name: String) -> ServiceDevice {
        ServiceDevice(id: AppleMusicIDs.device, name: name, kind: .computer, active: true, volumePercent: nil)
    }

    /// The core's report for MusicKit's state; nil when nothing is queued and nothing plays.
    static func report(_ snapshot: AppleMusicPlayerSnapshot, context: ExternalMedia?,
                       deviceName: String) -> ExternalPlayerReport? {
        guard snapshot.entry != nil || snapshot.status != .stopped else { return nil }
        let playing = snapshot.status == .playing || snapshot.status == .seeking
        let item = snapshot.entry.map { entry in
            ExternalMedia(kind: .track, id: entry.id, title: entry.title, artist: entry.artist, album: entry.album,
                          durationMs: milliseconds(entry.durationSeconds), artworkURL: entry.artworkURL)
        }
        return ExternalPlayerReport(item: item, context: context, playing: playing,
                                    positionMs: milliseconds(snapshot.positionSeconds) ?? 0,
                                    shuffle: snapshot.shuffle, repeatMode: snapshot.repeatMode,
                                    device: device(name: deviceName))
    }

    /// Seconds as whole milliseconds; nil for NaN, infinite or negative times.
    static func milliseconds(_ seconds: Double?) -> UInt64? {
        guard let seconds, seconds.isFinite, seconds >= 0 else { return nil }
        return UInt64((seconds * 1_000).rounded())
    }
}

/// What the Apple Music player asks of MusicKit, on the main actor.
@MainActor
protocol AppleMusicControls: AnyObject {
    /// Fetches what `request` names from the catalogue, queues it and plays.
    func play(_ request: AppleMusicPlayRequest) async throws
    func resume() async throws
    func pause()
    func next() async throws
    func previous() async throws
    func seek(toSeconds seconds: Double)
    func setShuffle(_ on: Bool)
    func setRepeat(_ mode: RepeatOption)
    func snapshot() -> AppleMusicPlayerSnapshot
}

/// Apple Music for the core (`AppPlayer`): one device, this Mac.
final class AppleMusicPlayer: AppPlayer, @unchecked Sendable {
    /// Longest wait for MusicKit to answer a call; playing fetches from the catalogue first.
    static let callTimeout: TimeInterval = 8
    static let playTimeout: TimeInterval = 15

    private let controls: AppleMusicControls
    private let deviceName: String
    private let lock = NSLock()
    /// The album or playlist the session asked for, reported until something else plays.
    private var context: ExternalMedia?

    /// Called on the main actor (MusicKit's player belongs to it).
    init(controls: AppleMusicControls, deviceName: String) {
        self.controls = controls
        self.deviceName = deviceName
    }

    func devices() throws -> [ServiceDevice] {
        [AppleMusicMapping.device(name: deviceName)]
    }

    func state() throws -> ExternalPlayerReport? {
        let snapshot = try onMain(timeout: Self.callTimeout) { $0.snapshot() }
        return AppleMusicMapping.report(snapshot, context: currentContext, deviceName: deviceName)
    }

    func play(context: ExternalMedia?, tracks: [ExternalMedia], start: Int) throws {
        guard let request = AppleMusicPlayRequest.from(context: context, tracks: tracks, start: start) else {
            throw AppPlayerError(reason: String(localized: "Apple Music cannot play this."))
        }
        try onMain(timeout: Self.playTimeout) { try await $0.play(request) }
        lock.withLock { self.context = context }
    }

    func resume() throws {
        try onMain(timeout: Self.callTimeout) { try await $0.resume() }
    }

    func pause() throws {
        try onMain(timeout: Self.callTimeout) { $0.pause() }
    }

    func next() throws {
        try onMain(timeout: Self.callTimeout) { try await $0.next() }
    }

    func previous() throws {
        try onMain(timeout: Self.callTimeout) { try await $0.previous() }
    }

    func seek(positionMs: UInt64) throws {
        let seconds = Double(positionMs) / 1_000
        try onMain(timeout: Self.callTimeout) { $0.seek(toSeconds: seconds) }
    }

    func setVolume(percent: UInt32) throws {
        throw AppPlayerError(reason: String(localized: "Apple Music's volume cannot be changed from Jellyfesh."))
    }

    func setShuffle(_ on: Bool) throws {
        try onMain(timeout: Self.callTimeout) { $0.setShuffle(on) }
    }

    func setRepeat(_ mode: RepeatOption) throws {
        try onMain(timeout: Self.callTimeout) { $0.setRepeat(mode) }
    }

    func transfer(device: String) throws {
        guard device == AppleMusicIDs.device else {
            throw AppPlayerError(reason: String(localized: "Apple Music plays only on this Mac."))
        }
    }

    private var currentContext: ExternalMedia? {
        lock.withLock { context }
    }

    /// Runs `work` on the main actor and waits for it on this (the core's) thread. Refuses on the
    /// main thread, which would wait for itself.
    @discardableResult
    private func onMain<T>(timeout: TimeInterval,
                           _ work: @escaping @MainActor (AppleMusicControls) async throws -> T) throws -> T {
        guard !Thread.isMainThread else {
            throw AppPlayerError(reason: String(localized: "Apple Music did not answer."))
        }
        let answer = Answer<T>()
        let controls = self.controls
        Task { @MainActor in
            do {
                answer.set(.success(try await work(controls)))
            } catch {
                answer.set(.failure(error))
            }
        }
        guard let result = answer.wait(timeout: timeout) else {
            throw AppPlayerError(reason: String(localized: "Apple Music did not answer."))
        }
        switch result {
        case .success(let value):
            return value
        case .failure(let error as AppPlayerError):
            throw error
        case .failure(let error):
            throw AppPlayerError(reason: AppleMusicText.failure(error))
        }
    }
}

/// One answer handed from the main actor to a waiting thread.
private final class Answer<T>: @unchecked Sendable {
    private let lock = NSLock()
    private let ready = DispatchSemaphore(value: 0)
    private var result: Result<T, Error>?

    func set(_ value: Result<T, Error>) {
        lock.withLock { result = value }
        ready.signal()
    }

    /// The answer, or nil when it did not come in time.
    func wait(timeout: TimeInterval) -> Result<T, Error>? {
        guard ready.wait(timeout: .now() + timeout) == .success else { return nil }
        return lock.withLock { result }
    }
}

enum AppleMusicText {
    /// A sentence for a MusicKit error: its own description when it has one.
    static func failure(_ error: Error) -> String {
        let text = (error as? LocalizedError)?.errorDescription?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let text, !text.isEmpty { return text }
        return String(localized: "Apple Music could not do that.")
    }
}
