// AI Lyrics (ADR-0019, docs/ui.md "AI Lyrics, Customize and Pro"): the `AiLyrics` slice and its
// commands. The core decides what is needed first (`need`), runs the jobs and keeps the made
// lyrics; this keeps what it reported, asks before audio leaves the device, and sends the person's
// choices.

import Foundation
import Observation

@MainActor
@Observable
final class AiLyricsStore {
    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter

    private(set) var state: AiLyricsState = .idle
    /// "Send This Song to {origin}?" is being asked, for the song it would make lyrics for.
    var consentQuestion: AudioConsentQuestion?
    /// Why the last command failed (a job that could not be queued, a download that could not
    /// start); shown where the person acted.
    private(set) var failure: String?

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
    }

    func apply(_ state: AiLyricsState) {
        self.state = state
    }

    /// Create Synced Lyrics (or Time These Lyrics, Make Again, Try Again) is offered.
    var isOffered: Bool { state.available }

    /// The newest job for the song playing (its queue entry), if any.
    func job(forEntry uid: UInt64?) -> LyricsMakingJob? {
        guard let uid else { return nil }
        return state.jobs.last { $0.uid == uid }
    }

    /// The newest job for a library song, if any.
    func job(forLibrarySong id: Int64) -> LyricsMakingJob? {
        state.jobs.last { $0.libraryID == id }
    }

    /// Makes lyrics for `song`, or says what has to happen first: the Pro sheet (`pro`), the
    /// question before audio is sent (`consent`). A model to download or an API key to add is
    /// shown where the person is (the Lyrics pane, Settings) and makes nothing here.
    func make(_ song: SongReference, pro: ProStore) {
        failure = nil
        switch state.need {
        case .pro:
            pro.require()
        case .consent(let origin):
            guard let plugin = state.plugin else { return }
            consentQuestion = AudioConsentQuestion(plugin: plugin, origin: origin, song: song)
        case .nothing:
            send(.makeLyrics(song))
        case .signIn, .model, .unavailable:
            // The Lyrics pane and Settings show the step; MakeLyrics would fail with it.
            send(.makeLyrics(song))
        }
    }

    /// Allow, in "Send This Song to {origin}?": remembers the origin for the plugin, then makes
    /// the lyrics asked for.
    func allow(_ question: AudioConsentQuestion) {
        consentQuestion = nil
        send(.allowAudioUpload(plugin: question.plugin, origin: question.origin))
        send(.makeLyrics(question.song))
    }

    func cancel(job: UInt64) { send(.cancelLyrics(job: job)) }
    func deleteMadeLyrics(_ song: SongReference) { send(.deleteMadeLyrics(song)) }
    func stopAllowing(_ consent: AudioUploadConsent) {
        send(.revokeAudioUpload(plugin: consent.plugin, origin: consent.origin))
    }

    func download(model id: String) { send(.downloadSpeechModel(id: id)) }
    func cancelDownload(model id: String) { send(.cancelSpeechModelDownload(id: id)) }
    func delete(model id: String) { send(.deleteSpeechModel(id: id)) }

    private func send(_ command: CoreCommand) {
        let id = client.send(command)
        tracker.watch(id) { [weak self] message in
            self?.failure = message
            if !isNotAvailableMessage(message) {
                self?.toasts.show(message)
            }
        }
    }
}

/// "Send This Song to {origin}?": asked once per plugin and origin before the first online job.
struct AudioConsentQuestion: Identifiable, Equatable {
    var plugin: String
    var origin: String
    /// The song to make lyrics for once allowed.
    var song: SongReference

    var id: String { "\(plugin)\u{1F}\(origin)" }
}

/// What the Lyrics pane offers for the song playing (docs/ui.md "AI Lyrics, Customize and Pro").
enum LyricsMakingStep: Equatable {
    /// Nothing: AI Lyrics is off or not installed, or its job is done.
    case none
    /// Create Synced Lyrics (Time These Lyrics over plain lyrics). Pro or the question about
    /// sending audio come when it is chosen.
    case create
    /// The job's stage, with its progress when it can be measured, and Cancel.
    case running(job: UInt64, stage: String, progress: Double?)
    /// "Couldn't make lyrics: {message}" and Try Again.
    case failed(message: String)
    /// "Download the {Model} model ({size})…" and Download.
    case downloadModel(id: String, text: String)
    /// "Downloading {Model}… 35%" and Cancel.
    case downloadingModel(id: String, text: String, fraction: Double)
    /// "Add your API key to make lyrics online." and Open AI Lyrics Settings.
    case signIn
    /// Why lyrics cannot be made.
    case unavailable(reason: String)

    /// A job runs or failed: shown even over lyrics the song has.
    var isJob: Bool {
        switch self {
        case .running, .failed: return true
        default: return false
        }
    }
}

/// AI Lyrics' words (docs/ui.md).
enum AiLyricsText {
    /// What the Lyrics pane offers: the song's job first (running, or failed), then, while AI
    /// Lyrics is offered, what making lyrics needs.
    static func step(job: LyricsMakingJob?, state: AiLyricsState) -> LyricsMakingStep {
        if let job {
            if job.step.isActive {
                return .running(job: job.id, stage: stage(job.step) ?? "", progress: progress(job.step))
            }
            if case .failed(let message) = job.step {
                return .failed(message: message)
            }
        }
        guard state.available else { return .none }
        switch state.need {
        case .nothing, .pro, .consent:
            return .create
        case .signIn:
            return .signIn
        case .model(let id, let name, let bytes):
            if let model = state.models.first(where: { $0.id == id }),
               case .downloading(let fraction) = model.progress {
                return .downloadingModel(id: id, text: downloading(name: name, fraction: fraction),
                                         fraction: clamp(Double(fraction)))
            }
            return .downloadModel(id: id, text: downloadModel(name: name, bytes: bytes))
        case .unavailable(let reason):
            let trimmed = reason.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? .none : .unavailable(reason: trimmed)
        }
    }

    /// A job's stage while it runs: "Reading Audio…", "Sending to {origin}…", "Recognising… 42%",
    /// "Timing Words…".
    static func stage(_ step: LyricsJobStep) -> String? {
        switch step {
        case .waiting: return String(localized: "Waiting…")
        case .readingAudio: return String(localized: "Reading Audio…")
        case .sending(let origin): return String(localized: "Sending to \(origin)…")
        case .recognising(let fraction):
            return String(localized: "Recognising… \(percent(fraction))")
        case .timingWords: return String(localized: "Timing Words…")
        case .done, .failed, .cancelled: return nil
        }
    }

    /// The job's progress for a bar, 0–1; nil when it cannot be measured.
    static func progress(_ step: LyricsJobStep) -> Double? {
        if case .recognising(let fraction) = step { return clamp(Double(fraction)) }
        return nil
    }

    /// "Couldn't make lyrics: {message}".
    static func failed(_ message: String) -> String {
        String(localized: "Couldn't make lyrics: \(message)")
    }

    /// "Made by AI Lyrics · {engine}".
    static func madeBy(engine: String) -> String {
        String(localized: "Made by AI Lyrics · \(engine)")
    }

    /// "Download the {Model} model ({size}) to make lyrics on this device."
    static func downloadModel(name: String, bytes: UInt64) -> String {
        let size = ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
        return String(localized: "Download the \(name) model (\(size)) to make lyrics on this device.")
    }

    /// "Downloading {Model}… 35%".
    static func downloading(name: String, fraction: Float) -> String {
        String(localized: "Downloading \(name)… \(percent(fraction))")
    }

    /// A model's row in Settings: "Base · 142 MB".
    static func modelLine(_ model: SpeechModelInfo) -> String {
        let size = ByteCountFormatter.string(fromByteCount: Int64(clamping: model.bytes), countStyle: .file)
        return "\(model.name) · \(size)"
    }

    /// "42%", in the person's locale.
    static func percent(_ fraction: Float) -> String {
        clamp(Double(fraction)).formatted(.percent.precision(.fractionLength(0)))
    }

    private static func clamp(_ value: Double) -> Double {
        value.isFinite ? min(max(value, 0), 1) : 0
    }
}
