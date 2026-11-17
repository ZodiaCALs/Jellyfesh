// MusicKit for Apple Music (ADR-0010, ADR-0018 item 5): the account checks, catalogue search and
// `ApplicationMusicPlayer` (macOS 14). Only this file imports MusicKit. Nothing here runs before
// the person turns Apple Music on (AppleMusicStore asks first), and nothing from MusicKit is
// stored: results live in memory while they are shown. MusicKit's `Song` is written in full
// (`MusicKit.Song`): the app's own `Song`, a library track, hides it in this module.

import Foundation
import MusicKit
import Security

/// The account checks and the catalogue, on the main actor.
@MainActor
final class MusicKitService: AppleMusicService {
    /// Whether this copy of the app is signed so that MusicKit can work at all: MusicKit needs the
    /// `com.apple.application-identifier` entitlement, which only a provisioning profile grants
    /// (Apple DTS, developer.apple.com/forums/thread/711950). Ad-hoc builds lack it. Read from the
    /// code signature, without touching MusicKit.
    nonisolated static func hasApplicationIdentifier() -> Bool {
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        let value = SecTaskCopyValueForEntitlement(task, "com.apple.application-identifier" as CFString, nil)
        return value != nil
    }

    let isSigned: Bool

    init() {
        isSigned = Self.hasApplicationIdentifier()
    }

    func requestAuthorization() async -> Bool {
        await MusicAuthorization.request() == .authorized
    }

    func isAuthorized() -> Bool {
        MusicAuthorization.currentStatus == .authorized
    }

    /// Asks for a developer token: without the MusicKit App Service on the app's identifier this
    /// fails even after the person allowed access.
    func canFetchTokens() async -> Bool {
        do {
            let token = try await MusicDataRequest.tokenProvider.developerToken(options: [])
            return !token.isEmpty
        } catch {
            return false
        }
    }

    func canPlayCatalog() async -> Bool {
        do {
            return try await MusicSubscription.current.canPlayCatalogContent
        } catch {
            return false
        }
    }

    func search(_ term: String) async throws -> AppleMusicResults {
        var request = MusicCatalogSearchRequest(term: term, types: [MusicKit.Song.self, Album.self])
        request.limit = 25
        let response = try await request.response()
        let songs = response.songs.map { song in
            AppleMusicSong(id: song.id.rawValue, title: song.title, artist: song.artistName, album: song.albumTitle,
                           durationMs: AppleMusicMapping.milliseconds(song.duration),
                           artworkURL: song.artwork?.url(width: 300, height: 300)?.absoluteString,
                           link: song.url?.absoluteString)
        }
        let albums = response.albums.map { album in
            AppleMusicAlbum(id: album.id.rawValue, title: album.title, artist: album.artistName,
                            artworkURL: album.artwork?.url(width: 300, height: 300)?.absoluteString,
                            link: album.url?.absoluteString)
        }
        return AppleMusicResults(songs: Array(songs), albums: Array(albums))
    }

    func makeControls() -> AppleMusicControls {
        MusicKitControls()
    }
}

/// `ApplicationMusicPlayer`: it plays for this app only and leaves the Music app's state alone.
@MainActor
final class MusicKitControls: AppleMusicControls {
    private var player: ApplicationMusicPlayer { ApplicationMusicPlayer.shared }

    func play(_ request: AppleMusicPlayRequest) async throws {
        switch request {
        case .songs(let ids, let start):
            let songs = try await Self.songs(ids)
            guard !songs.isEmpty else { throw Self.notFound }
            let first = songs.indices.contains(start) ? songs[start] : songs[0]
            player.queue = ApplicationMusicPlayer.Queue(for: songs, startingAt: first)
        case .album(let id, let start):
            let response = try await MusicCatalogResourceRequest<Album>(matching: \.id, equalTo: MusicItemID(id))
                .response()
            guard let album = response.items.first else { throw Self.notFound }
            if start > 0, let detailed = try? await album.with([.tracks]), let tracks = detailed.tracks {
                let list = Array(tracks)
                if list.indices.contains(start) {
                    player.queue = ApplicationMusicPlayer.Queue(album: detailed, startingAt: list[start])
                } else {
                    player.queue = ApplicationMusicPlayer.Queue(for: [album])
                }
            } else {
                player.queue = ApplicationMusicPlayer.Queue(for: [album])
            }
        case .playlist(let id, let start):
            let response = try await MusicCatalogResourceRequest<Playlist>(matching: \.id, equalTo: MusicItemID(id))
                .response()
            guard let playlist = response.items.first else { throw Self.notFound }
            if start > 0, let detailed = try? await playlist.with([.entries]), let entries = detailed.entries {
                let list = Array(entries)
                if list.indices.contains(start) {
                    player.queue = ApplicationMusicPlayer.Queue(playlist: detailed, startingAt: list[start])
                } else {
                    player.queue = ApplicationMusicPlayer.Queue(for: [playlist])
                }
            } else {
                player.queue = ApplicationMusicPlayer.Queue(for: [playlist])
            }
        }
        try await player.play()
    }

    func resume() async throws {
        try await player.play()
    }

    func pause() {
        player.pause()
    }

    func next() async throws {
        try await player.skipToNextEntry()
    }

    func previous() async throws {
        try await player.skipToPreviousEntry()
    }

    func seek(toSeconds seconds: Double) {
        player.playbackTime = max(seconds, 0)
    }

    func setShuffle(_ on: Bool) {
        player.state.shuffleMode = on ? MusicPlayer.ShuffleMode.songs : MusicPlayer.ShuffleMode.off
    }

    func setRepeat(_ mode: RepeatOption) {
        // Spelled out: `.none` alone would be the optional's nil, not "repeat nothing".
        switch mode {
        case .off: player.state.repeatMode = MusicPlayer.RepeatMode.none
        case .all: player.state.repeatMode = MusicPlayer.RepeatMode.all
        case .one: player.state.repeatMode = MusicPlayer.RepeatMode.one
        }
    }

    func snapshot() -> AppleMusicPlayerSnapshot {
        let state = player.state
        let status: AppleMusicPlayerSnapshot.Status
        switch state.playbackStatus {
        case .playing: status = .playing
        case .paused: status = .paused
        case .stopped: status = .stopped
        case .interrupted: status = .interrupted
        case .seekingForward, .seekingBackward: status = .seeking
        @unknown default: status = .paused
        }
        let repeatMode: RepeatOption
        if state.repeatMode == MusicPlayer.RepeatMode.all {
            repeatMode = .all
        } else if state.repeatMode == MusicPlayer.RepeatMode.one {
            repeatMode = .one
        } else {
            repeatMode = .off
        }
        var entry: AppleMusicPlayerSnapshot.Entry?
        if let current = player.queue.currentEntry {
            var song: MusicKit.Song?
            if case .some(.song(let item)) = current.item {
                song = item
            }
            let title = current.title.isEmpty ? (song?.title ?? "") : current.title
            entry = AppleMusicPlayerSnapshot.Entry(
                id: song?.id.rawValue ?? current.id,
                title: title,
                artist: song?.artistName ?? current.subtitle,
                album: song?.albumTitle,
                durationSeconds: song?.duration,
                artworkURL: (current.artwork ?? song?.artwork)?.url(width: 600, height: 600)?.absoluteString
            )
        }
        return AppleMusicPlayerSnapshot(
            status: status,
            entry: entry,
            positionSeconds: player.playbackTime,
            shuffle: state.shuffleMode == MusicPlayer.ShuffleMode.songs,
            repeatMode: repeatMode
        )
    }

    /// Catalogue songs by id, in the order asked.
    private static func songs(_ ids: [String]) async throws -> [MusicKit.Song] {
        let wanted = Array(ids.prefix(200))
        let response = try await MusicCatalogResourceRequest<MusicKit.Song>(matching: \.id,
                                                                    memberOf: wanted.map { MusicItemID($0) })
            .response()
        var byID: [String: MusicKit.Song] = [:]
        for song in response.items {
            byID[song.id.rawValue] = song
        }
        return wanted.compactMap { byID[$0] }
    }

    private static var notFound: AppPlayerError {
        AppPlayerError(reason: String(localized: "Apple Music could not find this."))
    }
}
