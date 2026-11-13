// Playback, the queue, what is playing and the output devices, as the core reports them, and the
// intents views send. Nothing here decides what plays: every change is a command.

import Foundation
import Observation

@MainActor
@Observable
final class PlaybackStore {
    let client: CoreClient
    private let tracker: CommandTracker
    /// The position to draw. Not observed: views read it per frame inside time-driven schedules
    /// and key those schedules by `anchorVersion`.
    let clock = PlaybackClock()

    private(set) var status: PlaybackStatus = .stopped
    /// Bumped on every anchor, so time-driven views restart their schedules.
    private(set) var anchorVersion = 0
    private(set) var volume: Float = 1
    private(set) var muted = false
    private(set) var nowPlaying: NowPlayingInfo?
    private(set) var queue: QueueState = .empty
    private(set) var devices: DevicesState = .empty
    private(set) var buffering = false
    /// The volume while the person drags the slider, until the core reports it.
    private(set) var draggedVolume: Float?
    /// Where a seek is going, until the core's next anchor arrives.
    private(set) var pendingSeekUs: Int64?

    @ObservationIgnored private var volumeSend: Task<Void, Never>?
    @ObservationIgnored private var lastVolumeSent: Float?
    @ObservationIgnored private var seekGeneration: UInt64?
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
        if let dragged = draggedVolume, volumeSend == nil, abs(dragged - state.volume) < 0.001 {
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

    // MARK: Reading

    var isPlaying: Bool { status == .playing }
    var hasTrack: Bool { nowPlaying != nil }
    var displayVolume: Float { draggedVolume ?? volume }

    /// The current track's duration: the anchor's, or the tags' before the engine opens it.
    var durationUs: Int64? {
        _ = anchorVersion
        if let duration = clock.durationUs, duration > 0 { return duration }
        return nowPlaying?.durationMs.map { Int64(clamping: $0) * 1_000 }
    }

    /// The position to draw now. Cheap; call it where it is drawn, not to store it.
    func positionUs() -> Int64 {
        if let pending = pendingSeekUs { return pending }
        return clock.positionUs(nowNs: client.monotonicNowNs())
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
        return PlaybackSnapshotForSchedule(
            positionUs: clock.positionUs(nowNs: now),
            anchorPositionUs: clock.anchorPositionUs(nowNs: now),
            running: clock.isRunning,
            rate: Double(clock.rate),
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
        seekGeneration = clock.anchor.trackGeneration
        client.send(.seek(positionMs: UInt64(clamped / 1_000)))
    }

    func seek(byUs delta: Int64) {
        seek(toUs: positionUs() + delta)
    }

    /// Sets the volume while dragging (at most every 50 ms) and once more at the end.
    func setVolume(_ value: Float, final: Bool) {
        let clamped = max(0, min(1, value))
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
        setVolume(displayVolume + delta, final: true)
    }

    private func sendVolume(_ value: Float) {
        guard value != lastVolumeSent else { return }
        lastVolumeSent = value
        client.send(.setVolume(value))
        if muted && value > 0 {
            setMuted(false)
        }
    }

    func toggleMute() { setMuted(!muted) }

    func setMuted(_ value: Bool) {
        muteChangePending = true
        client.send(.setMuted(value))
    }
    func toggleShuffle() { client.send(.setShuffle(!queue.shuffle)) }
    func setShuffle(_ on: Bool) { client.send(.setShuffle(on)) }
    func cycleRepeat() { client.send(.setRepeat(queue.repeatMode.next)) }
    func setRepeat(_ mode: RepeatOption) { client.send(.setRepeat(mode)) }

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
