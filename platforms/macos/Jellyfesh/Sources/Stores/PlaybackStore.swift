// Playback, the queue, what is playing and the output devices, as the core reports them, and the
// intents views send. Nothing here decides what plays: every change is a command.
//
// While the Remote slice has a source (ADR-0018), the player shows the remote session's track
// instead of the engine's: its title, its position (from its own anchor), its shuffle, repeat and
// volume. The transport commands stay the usual ones; the core routes them to the session.

import Foundation
import Observation

/// What the player bar, the mini player and Now Playing show: the engine's track, or the remote
/// session's while one runs.
struct PlayingItem: Equatable {
    var title: String
    var artist: String?
    var album: String?
    var artwork: ArtworkSource?
    /// Opens the track in its service (Open in {Service}).
    var link: String?
    /// The source whose service `link` opens.
    var source: String?
    /// The engine's queue entry; nil for a remote session's track.
    var uid: UInt64?
    /// What the engine plays; nil for a remote session's track.
    var track: TrackReference?
    /// A remote session: "Playing on {Device}", and the error it last had.
    var remote: Remote?

    struct Remote: Equatable {
        /// The service's name (Open in {Service}, the Lyrics and Queue panes).
        var service: String
        /// The playing device's name, when the service says.
        var device: String?
        /// What went wrong last talking to the service, for a quieter line.
        var error: String?
    }

    var isRemote: Bool { remote != nil }
}

/// One row of the Devices picker.
struct RemoteDeviceRow: Identifiable, Equatable {
    var device: ServiceDevice
    /// The device playing carries the check.
    var checked: Bool

    var id: String { device.id }
}

@MainActor
@Observable
final class PlaybackStore {
    let client: CoreClient
    private let tracker: CommandTracker
    /// The position to draw. Not observed: views read it per frame inside time-driven schedules
    /// and key those schedules by `anchorVersion`.
    let clock = PlaybackClock()
    /// The remote session's position, from its own anchor.
    let remoteClock = PlaybackClock()

    private(set) var status: PlaybackStatus = .stopped
    /// Bumped on every anchor, so time-driven views restart their schedules.
    private(set) var anchorVersion = 0
    private(set) var volume: Float = 1
    private(set) var muted = false
    private(set) var nowPlaying: NowPlayingInfo?
    private(set) var queue: QueueState = .empty
    private(set) var devices: DevicesState = .empty
    private(set) var buffering = false
    /// The remote session (`Remote` slice); `remote.source` is nil without one.
    private(set) var remote: RemoteSession = .idle
    /// The volume while the person drags the slider, until the core reports it.
    private(set) var draggedVolume: Float?
    /// Where a seek is going, until the core's next anchor arrives.
    private(set) var pendingSeekUs: Int64?

    @ObservationIgnored private var volumeSend: Task<Void, Never>?
    @ObservationIgnored private var lastVolumeSent: Float?
    @ObservationIgnored private var lastRemoteVolumeSent: UInt32?
    @ObservationIgnored private var seekGeneration: UInt64?
    @ObservationIgnored private var remoteSeekGeneration: UInt64?
    @ObservationIgnored private var remoteSeekExpiry: Task<Void, Never>?
    /// The version of the ended session whose error was shown, so it shows once.
    @ObservationIgnored private var shownEndingVersion: UInt64?
    /// A `SetMuted` was sent: read the muted state again when the playback slice it republishes
    /// arrives.
    @ObservationIgnored private var muteChangePending = false

    init(client: CoreClient, tracker: CommandTracker) {
        self.client = client
        self.tracker = tracker
    }

    // MARK: Slices

    func apply(_ state: PlaybackState) {
        status = state.status
        volume = state.volume
        clock.update(state.anchor, nowNs: client.monotonicNowNs())
        if let generation = seekGeneration, generation != state.anchor.trackGeneration {
            pendingSeekUs = nil
            seekGeneration = nil
        }
        if !isRemote, let dragged = draggedVolume, volumeSend == nil, abs(dragged - state.volume) < 0.001 {
            draggedVolume = nil
        }
        anchorVersion &+= 1
        if muteChangePending {
            muteChangePending = false
            Task { [weak self] in
                guard let self else { return }
                self.apply(muted: await self.client.isMuted())
            }
        }
    }

    func apply(nowPlaying info: NowPlayingInfo?) {
        if info?.uid != nowPlaying?.uid {
            buffering = false
        }
        nowPlaying = info
    }

    func apply(queue state: QueueState) {
        queue = state
    }

    func apply(devices state: DevicesState) {
        devices = state
    }

    func apply(muted value: Bool) {
        muted = value
    }

    func apply(bufferingUID uid: UInt64, buffering value: Bool) {
        guard uid == nowPlaying?.uid else { return }
        buffering = value
    }

    /// The `Remote` slice. Returns the error to show once when a session ended with one (the
    /// slice without a source carries why); errors while a session runs show in its quieter line.
    @discardableResult
    func apply(remote state: RemoteSession) -> String? {
        let wasRemote = isRemote
        remote = state
        remoteClock.update(state.anchor, nowNs: client.monotonicNowNs())
        if let generation = remoteSeekGeneration, generation != state.anchor.trackGeneration {
            clearRemoteSeek()
        }
        if wasRemote != isRemote {
            // The volume shown belongs to another player now.
            draggedVolume = nil
            if pendingSeekUs != nil {
                pendingSeekUs = nil
                seekGeneration = nil
                clearRemoteSeek()
            }
        } else if isRemote, let dragged = draggedVolume, volumeSend == nil,
                  let percent = state.device?.volumePercent,
                  abs(dragged * 100 - Float(percent)) < 0.5 {
            draggedVolume = nil
        }
        anchorVersion &+= 1
        guard state.source == nil, let error = state.error?.trimmingCharacters(in: .whitespacesAndNewlines),
              !error.isEmpty, state.version != shownEndingVersion
        else { return nil }
        shownEndingVersion = state.version
        return error
    }

    // MARK: Reading

    /// A remote session runs: the player shows and controls the service's player.
    var isRemote: Bool { remote.source != nil }

    var isPlaying: Bool { isRemote ? remote.status == .playing : status == .playing }

    /// Something to show in the player: the engine's track or a remote session.
    var hasTrack: Bool { nowPlaying != nil || isRemote }

    /// The service of the remote session, by name.
    var remoteService: String {
        let name = remote.sourceName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return name.isEmpty ? (remote.source ?? "") : name
    }

    /// What the player shows now.
    var playing: PlayingItem? {
        if isRemote {
            let item = remote.item
            let device = remote.device?.name.trimmingCharacters(in: .whitespacesAndNewlines)
            let error = remote.error?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = item?.title.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            return PlayingItem(
                title: title.isEmpty ? remoteService : title,
                artist: item?.subtitle,
                album: item?.album,
                artwork: item?.artwork.flatMap { ArtworkSource(hash: nil, location: $0) },
                link: item?.link,
                source: remote.source,
                uid: nil,
                track: nil,
                remote: PlayingItem.Remote(service: remoteService, device: device?.nilIfEmpty, error: error?.nilIfEmpty)
            )
        }
        guard let now = nowPlaying else { return nil }
        var source: String?
        if case .source(let song) = now.track { source = song.plugin }
        return PlayingItem(title: now.title, artist: now.artist, album: now.album,
                           artwork: now.artwork.flatMap { ArtworkSource(hash: nil, location: $0) },
                           link: now.link, source: source, uid: now.uid, track: now.track, remote: nil)
    }

    /// The quality badge and its menu for what plays (ADR-0026 §3): the core's levels and note,
    /// with `chosen`, the Streaming Quality setting. Nil with nothing playing.
    func qualityMenu(chosen: StreamingQualityOption) -> QualityMenuContent? {
        if isRemote {
            return QualityMenuContent(playing: nil, offered: [], chosen: chosen,
                                      note: remote.qualityNote ?? .setInService(remoteService))
        }
        guard let now = nowPlaying else { return nil }
        return QualityMenuContent(playing: now.quality, offered: now.qualities, chosen: chosen, note: now.qualityNote)
    }

    /// Shuffle as the player shows it: the remote session's, else the queue's.
    var shuffle: Bool { isRemote ? remote.shuffle : queue.shuffle }

    /// Repeat as the player shows it: the remote session's, else the queue's.
    var repeatMode: RepeatOption { isRemote ? remote.repeatMode : queue.repeatMode }

    /// The volume control shows: always for the engine, for a remote device only when it has one.
    var showsVolume: Bool { !isRemote || remote.device?.volumePercent != nil }

    /// Mute is the engine's; a remote device has only its volume.
    var canMute: Bool { !isRemote }

    var displayVolume: Float {
        if let draggedVolume { return draggedVolume }
        if isRemote { return Float(min(remote.device?.volumePercent ?? 0, 100)) / 100 }
        return volume
    }

    /// The Devices picker: every device the service lists, in its order, the playing one checked;
    /// the playing device first when the list does not have it yet (it arrives with Refresh).
    var remoteDeviceRows: [RemoteDeviceRow] { Self.deviceRows(remote) }

    nonisolated static func deviceRows(_ session: RemoteSession) -> [RemoteDeviceRow] {
        let playing = session.device
        let named = session.devices.filter { !$0.name.trimmingCharacters(in: .whitespaces).isEmpty }
        var rows = named.map { device in
            RemoteDeviceRow(device: device, checked: playing.map { $0.id == device.id } ?? device.active)
        }
        if let playing, !playing.name.trimmingCharacters(in: .whitespaces).isEmpty,
           !rows.contains(where: { $0.device.id == playing.id }) {
            rows.insert(RemoteDeviceRow(device: playing, checked: true), at: 0)
        }
        return rows
    }

    /// The clock of what the player shows.
    private var activeClock: PlaybackClock { isRemote ? remoteClock : clock }

    /// The current track's duration: the anchor's, or the track's own before the anchor has one.
    var durationUs: Int64? {
        _ = anchorVersion
        if let duration = activeClock.durationUs, duration > 0 { return duration }
        if isRemote {
            return remote.item?.durationMs.map { Int64(clamping: $0) * 1_000 }
        }
        return nowPlaying?.durationMs.map { Int64(clamping: $0) * 1_000 }
    }

    /// The position to draw now. Cheap; call it where it is drawn, not to store it.
    func positionUs() -> Int64 {
        if let pending = pendingSeekUs { return pending }
        return activeClock.positionUs(nowNs: client.monotonicNowNs())
    }

    /// Where a schedule starts. Reading it makes the view depend on the anchor version, so every
    /// new anchor starts a new schedule.
    func scheduleStart() -> PlaybackSnapshotForSchedule {
        _ = anchorVersion
        let now = client.monotonicNowNs()
        if let pending = pendingSeekUs {
            return PlaybackSnapshotForSchedule(positionUs: pending, anchorPositionUs: pending, running: false,
                                               durationUs: durationUs)
        }
        let active = activeClock
        return PlaybackSnapshotForSchedule(
            positionUs: active.positionUs(nowNs: now),
            anchorPositionUs: active.anchorPositionUs(nowNs: now),
            running: active.isRunning,
            rate: Double(active.rate),
            durationUs: durationUs
        )
    }

    /// How much of the track has played, 0 to 1, at the drawn position.
    func progressFraction() -> Double {
        guard let duration = durationUs, duration > 0 else { return 0 }
        return min(max(Double(positionUs()) / Double(duration), 0), 1)
    }

    // MARK: Transport

    func togglePlayPause() { client.send(.togglePlayPause) }
    func play() { client.send(.play) }
    func pause() { client.send(.pause) }
    func stop() { client.send(.stop) }
    func next() { client.send(.next) }
    func previous() { client.send(.previous) }

    func seek(toUs target: Int64) {
        let clamped = max(0, durationUs.map { min(target, $0) } ?? target)
        pendingSeekUs = clamped
        if isRemote {
            remoteSeekGeneration = remote.anchor.trackGeneration
            // A seek the service refused brings no new anchor: show its position again soon.
            remoteSeekExpiry?.cancel()
            remoteSeekExpiry = Task { [weak self] in
                try? await Task.sleep(for: .seconds(3))
                guard let self, !Task.isCancelled, self.remoteSeekGeneration != nil else { return }
                self.clearRemoteSeek()
            }
        } else {
            seekGeneration = clock.anchor.trackGeneration
        }
        client.send(.seek(positionMs: UInt64(clamped / 1_000)))
    }

    func seek(byUs delta: Int64) {
        seek(toUs: positionUs() + delta)
    }

    private func clearRemoteSeek() {
        remoteSeekGeneration = nil
        remoteSeekExpiry?.cancel()
        remoteSeekExpiry = nil
        if seekGeneration == nil {
            pendingSeekUs = nil
        }
    }

    /// Sets the volume while dragging (at most every 50 ms) and once more at the end: the engine's,
    /// or the remote device's while a session runs.
    func setVolume(_ value: Float, final: Bool) {
        guard showsVolume else { return }
        let clamped = value.isFinite ? max(0, min(1, value)) : 0
        draggedVolume = clamped
        if final {
            volumeSend?.cancel()
            volumeSend = nil
            sendVolume(clamped)
            return
        }
        guard volumeSend == nil else { return }
        volumeSend = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(50))
            guard let self, !Task.isCancelled else { return }
            self.volumeSend = nil
            if let volume = self.draggedVolume { self.sendVolume(volume) }
        }
    }

    func nudgeVolume(by delta: Float) {
        guard showsVolume else { return }
        setVolume(displayVolume + delta, final: true)
    }

    private func sendVolume(_ value: Float) {
        if isRemote {
            let percent = Self.volumePercent(value)
            guard percent != lastRemoteVolumeSent || remote.device?.volumePercent != percent else { return }
            lastRemoteVolumeSent = percent
            client.send(.remote(.setVolume(percent: percent)))
            return
        }
        guard value != lastVolumeSent else { return }
        lastVolumeSent = value
        client.send(.setVolume(value))
        if muted && value > 0 {
            setMuted(false)
        }
    }

    /// A slider position (0 to 1) as a remote device's volume, in whole percent.
    nonisolated static func volumePercent(_ value: Float) -> UInt32 {
        guard value.isFinite else { return 0 }
        return UInt32((min(max(value, 0), 1) * 100).rounded())
    }

    func toggleMute() {
        guard canMute else { return }
        setMuted(!muted)
    }

    func setMuted(_ value: Bool) {
        muteChangePending = true
        client.send(.setMuted(value))
    }

    func toggleShuffle() { client.send(.setShuffle(!shuffle)) }
    func setShuffle(_ on: Bool) { client.send(.setShuffle(on)) }
    func cycleRepeat() { client.send(.setRepeat(repeatMode.next)) }
    func setRepeat(_ mode: RepeatOption) { client.send(.setRepeat(mode)) }

    // MARK: Remote session

    /// Asks the service for its devices and state now (the Devices picker opens).
    func refreshRemote() { client.send(.remote(.refresh)) }

    /// Moves the remote session's playback to another device.
    func transferRemote(to device: String) { client.send(.remote(.transfer(device: device))) }

    /// Stop Controlling: the service's player carries on by itself.
    func endRemote() { client.send(.remote(.end)) }

    // MARK: Queue

    func play(_ collection: CollectionReference, from index: Int = 0, shuffle: Bool = false,
              onFailure: ((String) -> Void)? = nil) {
        let id = client.send(.playCollection(collection, start: UInt32(clamping: max(0, index)), shuffle: shuffle))
        if let onFailure { tracker.watch(id, onFailure: onFailure) }
    }

    func play(tracks: [TrackReference], from index: Int = 0) {
        guard !tracks.isEmpty else { return }
        client.send(.playTracks(tracks, start: UInt32(clamping: max(0, min(index, tracks.count - 1)))))
    }

    func enqueue(_ tracks: [TrackReference], next: Bool) {
        guard !tracks.isEmpty else { return }
        client.send(.enqueue(tracks, playNext: next))
    }

    func enqueue(_ collection: CollectionReference, next: Bool) {
        client.send(.enqueueCollection(collection, playNext: next))
    }

    func jump(to uid: UInt64) { client.send(.jumpTo(uid: uid)) }
    func remove(_ uid: UInt64) { client.send(.removeFromQueue(uid: uid)) }
    func clearQueue() { client.send(.clearQueue) }

    /// Moves an Up Next entry. `from` and `to` are SwiftUI's `onMove` indices in the shown list
    /// (`to` counts the gap before which the entry lands); the core takes the final index in the
    /// whole queue, where Up Next starts after the current entry.
    func moveUpNext(from: Int, to: Int, in entries: [QueueEntry]) {
        guard entries.indices.contains(from) else { return }
        let destination = to > from ? to - 1 : to
        guard destination != from else { return }
        let base = queue.position.map { Int($0) + 1 } ?? 0
        client.send(.moveInQueue(uid: entries[from].uid, to: UInt32(clamping: base + destination)))
    }

    // MARK: Output

    func selectOutput(_ id: String?) { client.send(.setOutputDevice(id)) }
    func refreshDevices() { client.send(.refreshDevices) }
}
