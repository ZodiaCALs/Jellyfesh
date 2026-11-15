// The person's settings as the core keeps them (`Settings` slice). Every change goes back as a
// whole modified copy (`SetSettings`); the core applies playback, library and source settings and
// the app applies the appearance ones (docs/core-api.md).

import Foundation
import Observation

@MainActor
@Observable
final class SettingsStore {
    let client: CoreClient
    private let tracker: CommandTracker
    private let toasts: ToastCenter

    /// The equalizer presets, in the core's order (`eq_presets()`).
    let equalizerPresets: [EqualizerPreset]
    /// The settings as the core last reported them.
    private(set) var settings: AppSettings = .defaults
    /// The copy sent last, shown until the core reports it back. Every change builds on it, so
    /// two quick changes never undo each other.
    private(set) var draft: AppSettings?

    @ObservationIgnored private var draftExpiry: Task<Void, Never>?
    /// Called with what controls show whenever it changes: the app applies the appearance and the
    /// Dock icon at once, without waiting for the core's answer.
    @ObservationIgnored var onChange: ((AppSettings) -> Void)?

    init(client: CoreClient, tracker: CommandTracker, toasts: ToastCenter) {
        self.client = client
        self.tracker = tracker
        self.toasts = toasts
        equalizerPresets = client.equalizerPresets()
    }

    /// What controls show: the change in flight, else the core's settings.
    var current: AppSettings { draft ?? settings }

    func apply(_ value: AppSettings) {
        settings = value
        if draft == value {
            clearDraft()
        }
        onChange?(current)
    }

    /// Changes the settings and sends the whole copy to the core.
    func update(_ change: (inout AppSettings) -> Void) {
        var next = current
        change(&next)
        guard next != current else { return }
        draft = next
        onChange?(next)
        let id = client.send(.setSettings(next))
        tracker.watch(id) { [weak self] message in
            self?.clearDraft()
            self?.toasts.show(message)
        }
        // The core may adjust a value (a crossfade length out of range): after a while the
        // settings it reports win over the draft.
        draftExpiry?.cancel()
        draftExpiry = Task { [weak self] in
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }
            self?.clearDraft()
        }
    }

    private func clearDraft() {
        let shown = current
        draft = nil
        draftExpiry?.cancel()
        draftExpiry = nil
        if current != shown {
            onChange?(current)
        }
    }

    // MARK: Appearance

    func setAppearance(_ value: AppearanceOption) { update { $0.appearance = value } }
    func setStyle(_ value: BackdropOption) { update { $0.style = value } }
    func setAppIcon(_ id: String) { update { $0.appIcon = id } }
    func setHueSource(_ value: HueSourceOption) { update { $0.hueSource = value } }
    func setReduceEffects(_ value: Bool) { update { $0.reduceEffects = value } }
    /// 0 (solid) to 1 (clearest); 0.5 keeps the glass tokens' alphas.
    func setTransparency(_ value: Float) { update { $0.transparency = value.isFinite ? min(max(value, 0), 1) : 0.5 } }

    // MARK: Lyrics

    func setShowTranslation(_ value: Bool) { update { $0.showTranslation = value } }
    func setShowRomanization(_ value: Bool) { update { $0.showRomanization = value } }
    func setLyricsSize(_ value: LyricsSizeOption) { update { $0.lyricsSize = value } }
    func setLyricsAlign(_ value: LyricsAlignOption) { update { $0.lyricsAlign = value } }

    // MARK: Playback

    func setCrossfade(_ value: CrossfadeConfig) { update { $0.crossfade = value } }
    func setReplayGain(_ mode: ReplayGainOption) { update { $0.replayGain = mode } }
    func setPreamp(_ db: Float) { update { $0.preampDb = min(max(db, -15), 15) } }
    func setEqualizer(_ eq: EqualizerConfig) { update { $0.eq = eq } }
    func setResumeOnLaunch(_ value: Bool) { update { $0.resumeOnLaunch = value } }

    /// Applies a preset as the core defines it: its bands, switched on, with a 0 dB preamp.
    func applyEqualizerPreset(_ preset: EqualizerPreset) { setEqualizer(preset.settings) }

    /// The preset with these bands, or nil once a band has moved (the Preset picker shows Custom).
    /// The preamp and the on/off switch are not part of the choice.
    func equalizerPreset(matching eq: EqualizerConfig) -> EqualizerPreset? {
        equalizerPresets.first { Self.sameBands($0.settings.bands, eq.bands) }
    }

    /// Bands that sound the same: shape, frequency, width and gain, within what the sliders show.
    nonisolated static func sameBands(_ a: [EqualizerBand], _ b: [EqualizerBand]) -> Bool {
        guard a.count == b.count else { return false }
        for (x, y) in zip(a, b) {
            let sameShape = x.kind == y.kind && x.enabled == y.enabled
            let sameFrequency: Bool = abs(x.freqHz - y.freqHz) < 0.5
            let sameWidth: Bool = abs(x.q - y.q) < 0.01
            let sameGain: Bool = abs(x.gainDb - y.gainDb) < 0.05
            if !(sameShape && sameFrequency && sameWidth && sameGain) { return false }
        }
        return true
    }

    // MARK: Library and sources

    func setWatchFolders(_ value: Bool) { update { $0.watchFolders = value } }
    func setAllowCommunityPlugins(_ value: Bool) { update { $0.allowCommunityPlugins = value } }
}

/// The ten app icons (ADR-0017), after Automatic, in the picker's order.
enum AppIconChoice: String, CaseIterable, Identifiable {
    case automatic, ice, steel, original
    case navyTide = "navy-tide"
    case royal
    case tealDeep = "teal-deep"
    case seaGlass = "sea-glass"
    case electric, abyss, lagoon

    var id: String { rawValue }

    /// The option's name in the picker. Icon names stay as designed in every language except where
    /// a translation reads more naturally (Localizable.xcstrings).
    var title: String {
        switch self {
        case .automatic: return String(localized: "Automatic")
        case .ice: return String(localized: "Ice")
        case .steel: return String(localized: "Steel")
        case .original: return String(localized: "Original")
        case .navyTide: return String(localized: "Navy Tide")
        case .royal: return String(localized: "Royal")
        case .tealDeep: return String(localized: "Teal Deep")
        case .seaGlass: return String(localized: "Sea Glass")
        case .electric: return String(localized: "Electric")
        case .abyss: return String(localized: "Abyss")
        case .lagoon: return String(localized: "Lagoon")
        }
    }

    /// The picker thumbnail in the asset catalog.
    var thumbnailName: String {
        self == .automatic ? "IconThumb-ice" : "IconThumb-\(rawValue)"
    }
}
