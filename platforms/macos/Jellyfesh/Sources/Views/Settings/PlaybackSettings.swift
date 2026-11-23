// Playback, in the order of docs/ui.md, as the web's cards (SettingsCards): where the sound goes,
// crossfade, volume levelling (ReplayGain), the equalizer with the core's presets, and resuming at
// startup. The core applies them (docs/core-api.md, "Playback output"); sliders send while they
// move, at most every 100 ms, and once more when let go.

import SwiftUI

struct PlaybackSettings: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(PlaybackStore.self) private var playback
    @State private var crossfadeDraft: Double?
    @State private var preampDraft: Double?

    var body: some View {
        SettingsCardsColumn {
            outputCard
            crossfadeCard
            levellingCard
            EqualizerSection()
            startupCard
        }
    }

    private var outputCard: some View {
        SettingsCard(title: "Output") {
            SettingsRow("Output Device", help: "Where the sound goes.") {
                Picker("Output Device", selection: Binding<String?>(get: { playback.devices.selected },
                                                                    set: { playback.selectOutput($0) })) {
                    Text("System Default").tag(String?.none)
                    ForEach(playback.devices.devices) { device in
                        Text(device.name).tag(String?.some(device.id))
                    }
                }
                .labelsHidden()
                .fixedSize()
            }
        }
    }

    private var crossfadeCard: some View {
        let current = settings.current
        return SettingsCard(title: "Crossfade") {
            SettingsToggleRow("Crossfade", help: "Blend the end of a song into the next.", isOn: Binding(
                get: { current.crossfade.enabled },
                set: { value in
                    var fade = current.crossfade
                    fade.enabled = value
                    settings.setCrossfade(fade)
                }
            ))
            if current.crossfade.enabled {
                let seconds = crossfadeDraft ?? Double(current.crossfade.durationMs) / 1000
                SettingsDivider()
                SettingsRow("Length") {
                    HStack {
                        Slider(value: Binding(get: { seconds }, set: { crossfadeDraft = $0 }), in: 1...12, step: 1,
                               onEditingChanged: { editing in
                                   guard !editing, let value = crossfadeDraft else { return }
                                   var fade = settings.current.crossfade
                                   fade.durationMs = UInt32(value * 1000)
                                   settings.setCrossfade(fade)
                                   crossfadeDraft = nil
                               })
                        .frame(width: 220)
                        .accessibilityLabel(Text("Length"))
                        Text("\(Int(seconds)) s")
                            .monospacedDigit()
                            .frame(width: 40, alignment: .trailing)
                    }
                }
                SettingsDivider()
                SettingsToggleRow("Album Continuity", help: "Songs of the same album join without a fade.", isOn: Binding(
                    get: { current.crossfade.albumContinuity },
                    set: { value in
                        var fade = current.crossfade
                        fade.albumContinuity = value
                        settings.setCrossfade(fade)
                    }
                ))
            }
        }
    }

    private var levellingCard: some View {
        let current = settings.current
        return SettingsCard(title: "Volume Levelling",
                            note: Text("Evens out loudness using the ReplayGain tags in your files.")) {
            SettingsRow("ReplayGain") {
                Picker("ReplayGain", selection: Binding(get: { current.replayGain }, set: { settings.setReplayGain($0) })) {
                    Text("Off").tag(ReplayGainOption.off)
                    Text("Track").tag(ReplayGainOption.track)
                    Text("Album").tag(ReplayGainOption.album)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
            }
            if current.replayGain != .off {
                let db = preampDraft ?? Double(current.preampDb)
                SettingsDivider()
                SettingsRow("Preamp") {
                    HStack {
                        Slider(value: Binding(get: { db }, set: { preampDraft = $0 }), in: -15...15, step: 0.5,
                               onEditingChanged: { editing in
                                   guard !editing, let value = preampDraft else { return }
                                   settings.setPreamp(Float(value))
                                   preampDraft = nil
                               })
                        .frame(width: 220)
                        .accessibilityLabel(Text("Preamp"))
                        Text(DecibelText.format(db))
                            .monospacedDigit()
                            .frame(width: 64, alignment: .trailing)
                    }
                }
            }
        }
    }

    private var startupCard: some View {
        SettingsCard(title: "Startup") {
            SettingsToggleRow("Resume Playback", help: "Start playing where you left off when Jellyfesh opens.",
                              isOn: Binding(get: { settings.current.resumeOnLaunch },
                                            set: { settings.setResumeOnLaunch($0) }))
        }
    }
}

/// The equalizer: on or off, the core's presets, a preamp and the core's bands (ten one-octave
/// bands by default), and Reset to Flat.
private struct EqualizerSection: View {
    @Environment(SettingsStore.self) private var settings
    @State private var draft: EqualizerConfig?
    @State private var sendTask: Task<Void, Never>?

    /// The gain the sliders allow, in dB (as the Linux app).
    private static let range: ClosedRange<Double> = -12...12
    /// The Preset picker's choice while the bands match no preset.
    private static let custom = "custom"

    var body: some View {
        let eq = draft ?? settings.current.eq
        SettingsCard(title: "Equalizer") {
            SettingsToggleRow("Equalizer", isOn: Binding(get: { eq.enabled }, set: { value in
                var next = settings.current.eq
                next.enabled = value
                settings.setEqualizer(next)
            }))
            if !settings.equalizerPresets.isEmpty {
                SettingsDivider()
                SettingsRow("Preset") {
                    presetPicker(matched: settings.equalizerPreset(matching: eq))
                }
            }
            if !eq.bands.isEmpty {
                SettingsDivider()
                bandRow(label: String(localized: "Preamp"), value: Double(eq.preampDb)) { value in
                    edit { $0.preampDb = Float(value) }
                } done: { finish() }
                ForEach(Array(eq.bands.enumerated()), id: \.offset) { index, band in
                    bandRow(label: Self.frequency(band.freqHz), value: Double(band.gainDb)) { value in
                        edit { $0.bands[index].gainDb = Float((value * 10).rounded() / 10) }
                    } done: { finish() }
                }
                HStack {
                    Spacer()
                    Button("Reset to Flat") {
                        var flat = settings.current.eq
                        flat.preampDb = 0
                        for index in flat.bands.indices { flat.bands[index].gainDb = 0 }
                        settings.setEqualizer(flat)
                    }
                    .disabled(eq.preampDb == 0 && eq.bands.allSatisfy { $0.gainDb == 0 })
                }
                .padding(.vertical, Tokens.Space.s3)
            }
        }
    }

    /// Flat, Bass Boost, Vocal and Treble from the core, and Custom once a band has moved.
    private func presetPicker(matched: EqualizerPreset?) -> some View {
        Picker("Preset", selection: Binding<String>(
            get: { matched?.id ?? Self.custom },
            set: { id in
                guard let preset = settings.equalizerPresets.first(where: { $0.id == id }) else { return }
                apply(preset)
            }
        )) {
            ForEach(settings.equalizerPresets) { preset in
                Text(EqualizerPresetText.name(preset.id)).tag(preset.id)
            }
            if matched == nil {
                Text("Custom").tag(Self.custom)
            }
        }
        .labelsHidden()
        .fixedSize()
    }

    /// A preset replaces whatever a slider was still sending.
    private func apply(_ preset: EqualizerPreset) {
        sendTask?.cancel()
        sendTask = nil
        draft = nil
        settings.applyEqualizerPreset(preset)
    }

    private func bandRow(label: String, value: Double, change: @escaping (Double) -> Void,
                         done: @escaping () -> Void) -> some View {
        HStack(spacing: Tokens.Space.s3) {
            Text(label)
                .lookFont(size: 13, weight: .semibold)
                .monospacedDigit()
                .frame(width: 56, alignment: .leading)
            Slider(value: Binding(get: { value }, set: change), in: Self.range, step: 0.5,
                   onEditingChanged: { editing in if !editing { done() } })
                .accessibilityLabel(Text(label))
            Text(DecibelText.format(value))
                .monospacedDigit()
                .frame(width: 64, alignment: .trailing)
        }
        .padding(.vertical, Tokens.Space.s1)
    }

    /// Changes the draft and sends it, at most every 100 ms while a slider moves.
    private func edit(_ change: (inout EqualizerConfig) -> Void) {
        var next = draft ?? settings.current.eq
        change(&next)
        draft = next
        guard sendTask == nil else { return }
        sendTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(100))
            if let latest = draft { settings.setEqualizer(latest) }
            sendTask = nil
        }
    }

    private func finish() {
        sendTask?.cancel()
        sendTask = nil
        if let latest = draft { settings.setEqualizer(latest) }
        draft = nil
    }

    /// `31`, `250`, `1k`, `16k`.
    static func frequency(_ hertz: Float) -> String {
        if hertz >= 1000 {
            let kilo = hertz / 1000
            if abs(kilo - kilo.rounded()) < 0.05 {
                return "\(Int(kilo.rounded()))k"
            }
            return String(format: "%.1fk", Double(kilo))
        }
        return "\(Int(hertz.rounded()))"
    }
}

/// An equalizer preset's name, by the core's id ("Flat", "Bass Boost", "Vocal", "Treble"); an id
/// this build does not know reads as words.
enum EqualizerPresetText {
    static func name(_ id: String) -> String {
        switch id {
        case "flat": return String(localized: "Flat")
        case "bass_boost": return String(localized: "Bass Boost")
        case "vocal": return String(localized: "Vocal")
        case "treble": return String(localized: "Treble")
        default: return SchemaForm.humanize(id)
        }
    }
}

/// `+3.0 dB`, `−1.5 dB`.
enum DecibelText {
    static func format(_ db: Double) -> String {
        let value = (db * 10).rounded() / 10
        let sign = value > 0 ? "+" : (value < 0 ? "−" : "")
        return sign + String(format: "%.1f dB", abs(value))
    }
}
