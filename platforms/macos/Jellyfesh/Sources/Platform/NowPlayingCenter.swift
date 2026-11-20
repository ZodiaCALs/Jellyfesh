// The system's media session (docs/architecture.md §11): Control Center, the menu bar's Now
// Playing, media keys and headphone buttons. The info is set when a slice changes, never per
// frame: the system extrapolates the elapsed time from the rate. macOS needs `playbackState` on
// every start and stop (ADR-0007).

import AppKit
import MediaPlayer

@MainActor
final class NowPlayingCenter {
    private let playback: PlaybackStore
    private let artwork: ArtworkLoader
    private var artworkFor: ArtworkLocation?
    private var artworkImage: NSImage?
    private var artworkTask: Task<Void, Never>?
    private var registered = false

    init(playback: PlaybackStore, artwork: ArtworkLoader) {
        self.playback = playback
        self.artwork = artwork
    }

    /// Hooks up media keys and the system's transport controls. The handlers may run on any
    /// thread; each hands its command to the main actor.
    func start() {
        guard !registered else { return }
        registered = true
        let playback = self.playback
        let center = MPRemoteCommandCenter.shared()
        center.playCommand.addTarget { _ in
            Task { @MainActor in playback.play() }
            return .success
        }
        center.pauseCommand.addTarget { _ in
            Task { @MainActor in playback.pause() }
            return .success
        }
        center.togglePlayPauseCommand.addTarget { _ in
            Task { @MainActor in playback.togglePlayPause() }
            return .success
        }
        center.stopCommand.addTarget { _ in
            Task { @MainActor in playback.stop() }
            return .success
        }
        center.nextTrackCommand.addTarget { _ in
            Task { @MainActor in playback.next() }
            return .success
        }
        center.previousTrackCommand.addTarget { _ in
            Task { @MainActor in playback.previous() }
            return .success
        }
        center.changePlaybackPositionCommand.addTarget { event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            let target = Int64(event.positionTime * 1_000_000)
            Task { @MainActor in playback.seek(toUs: target) }
            return .success
        }
        center.changeRepeatModeCommand.addTarget { event in
            guard let event = event as? MPChangeRepeatModeCommandEvent else { return .commandFailed }
            let mode: RepeatOption
            switch event.repeatType {
            case .one: mode = .one
            case .all: mode = .all
            default: mode = .off
            }
            Task { @MainActor in playback.setRepeat(mode) }
            return .success
        }
        center.changeShuffleModeCommand.addTarget { event in
            guard let event = event as? MPChangeShuffleModeCommandEvent else { return .commandFailed }
            let on = event.shuffleType != .off
            Task { @MainActor in playback.setShuffle(on) }
            return .success
        }
        center.skipForwardCommand.isEnabled = false
        center.skipBackwardCommand.isEnabled = false
        center.seekForwardCommand.isEnabled = false
        center.seekBackwardCommand.isEnabled = false
    }

    /// Sends what is playing and where, after a playback, queue, now-playing or remote change. A
    /// remote session shows its track; the media keys reach it through the core.
    func update() {
        let info = MPNowPlayingInfoCenter.default()
        guard let now = playback.playing else {
            info.nowPlayingInfo = nil
            info.playbackState = .stopped
            artworkFor = nil
            artworkImage = nil
            return
        }
        let remote = now.isRemote
        var values: [String: Any] = [
            MPMediaItemPropertyTitle: now.title,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: Double(playback.positionUs()) / 1_000_000,
            MPNowPlayingInfoPropertyPlaybackRate: playback.isPlaying && (remote || !playback.buffering) ? 1.0 : 0.0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if !remote {
            values[MPNowPlayingInfoPropertyPlaybackQueueCount] = Int(playback.queue.length)
            if let position = playback.queue.position {
                values[MPNowPlayingInfoPropertyPlaybackQueueIndex] = Int(position)
            }
        }
        if let artist = now.artist { values[MPMediaItemPropertyArtist] = artist }
        if let album = now.album { values[MPMediaItemPropertyAlbumTitle] = album }
        if let duration = playback.durationUs {
            values[MPMediaItemPropertyPlaybackDuration] = Double(duration) / 1_000_000
        }
        // Artwork files only (the engine's tracks): networking stays in the core.
        let artwork = remote ? nil : playback.nowPlaying?.artwork
        if artwork == artworkFor, let image = artworkImage {
            values[MPMediaItemPropertyArtwork] = Self.mediaArtwork(image)
        } else {
            loadArtwork(artwork)
        }
        info.nowPlayingInfo = values
        let status = remote ? playback.remote.status : playback.status
        switch status {
        case .playing: info.playbackState = .playing
        case .paused: info.playbackState = .paused
        case .stopped, .ended: info.playbackState = .stopped
        }
        let repeatType: MPRepeatType
        switch playback.repeatMode {
        case .off: repeatType = .off
        case .one: repeatType = .one
        case .all: repeatType = .all
        }
        let commands = MPRemoteCommandCenter.shared()
        commands.changeRepeatModeCommand.currentRepeatType = repeatType
        commands.changeShuffleModeCommand.currentShuffleType = playback.shuffle ? .items : .off
    }

    /// A file, or a source's picture through the core's cache (`source_artwork`) until the core
    /// publishes the song again with the file it fetched.
    private func loadArtwork(_ location: ArtworkLocation?) {
        artworkTask?.cancel()
        artworkFor = location
        artworkImage = nil
        guard let source = ArtworkSource(hash: nil, location: location) else { return }
        artworkTask = Task { [weak self] in
            guard let self else { return }
            let image = await self.artwork.image(source, pixels: 512)
            guard !Task.isCancelled, self.artworkFor == location else { return }
            self.artworkImage = image
            if image != nil { self.update() }
        }
    }

    /// The artwork for the system. Made outside the main actor: the system calls the request
    /// handler on a queue of its own.
    private nonisolated static func mediaArtwork(_ image: NSImage) -> MPMediaItemArtwork {
        MPMediaItemArtwork(boundsSize: image.size) { _ in image }
    }
}
