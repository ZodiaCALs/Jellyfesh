// A player the app provides (ADR-0018 item 5) as the core's `ExternalPlayer`, the UniFFI foreign
// trait (docs/ffi.md, "Players the apps provide"): it converts the generated types to the app's and
// turns every failure into `ExternalPlayerError.Failed`, whose reason the Remote slice shows.

import Foundation
import JellyfeshFFI

/// The core calls this on a remote session's own thread; each call may block until the app's
/// player answers.
final class FFIExternalPlayer: ExternalPlayer, @unchecked Sendable {
    private let player: AppPlayer

    init(_ player: AppPlayer) {
        self.player = player
    }

    func devices() throws -> [RemoteDevice] {
        try answer { try player.devices().map(\.ffi) }
    }

    func state() throws -> ExternalState? {
        try answer { try player.state()?.ffi }
    }

    func play(context: ExternalItem?, tracks: [ExternalItem], start: UInt32) throws {
        try answer {
            try player.play(context: context.map(ExternalMedia.init), tracks: tracks.map(ExternalMedia.init),
                            start: Int(start))
        }
    }

    func resume() throws {
        try answer { try player.resume() }
    }

    func pause() throws {
        try answer { try player.pause() }
    }

    func next() throws {
        try answer { try player.next() }
    }

    func previous() throws {
        try answer { try player.previous() }
    }

    func seek(positionMs: UInt64) throws {
        try answer { try player.seek(positionMs: positionMs) }
    }

    func setVolume(percent: UInt32) throws {
        try answer { try player.setVolume(percent: percent) }
    }

    func setShuffle(on: Bool) throws {
        try answer { try player.setShuffle(on) }
    }

    func setRepeat(mode: RepeatMode) throws {
        try answer { try player.setRepeat(RepeatOption(mode)) }
    }

    func transfer(device: String) throws {
        try answer { try player.transfer(device: device) }
    }

    /// The player's answer, or `ExternalPlayerError.Failed` with a sentence the person can read.
    private func answer<T>(_ call: () throws -> T) throws -> T {
        do {
            return try call()
        } catch let error as AppPlayerError {
            throw ExternalPlayerError.Failed(reason: error.reason)
        } catch {
            throw ExternalPlayerError.Failed(reason: error.localizedDescription)
        }
    }
}
