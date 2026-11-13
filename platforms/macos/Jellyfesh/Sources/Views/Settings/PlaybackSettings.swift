// Playback: where the sound goes, crossfade, ReplayGain and the equalizer. The core applies them
// (docs/core-api.md, "Playback output"); sliders send while they move, at most every 100 ms, and
// once more when let go.

import SwiftUI

struct PlaybackSettings: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(PlaybackStore.self) private var playback
    @State private var crossfadeDraft: Double?
    @State private var preampDraft: Double?

    var body: some View {
        let current = settings.current
        Form {
            Section("Output") {
                Picker(selection: Binding<String?>(get: { playback.devices.selected }, set: { playback.selectOutput($0) })) {
                    Text("System Default").tag(String?.none)
                    ForEach(playback.devices.devices) { device in
                        Text(device.name).tag(String?.some(device.id))
                    }
                } label: {
                    Text("Output Device")
                    Text("Where the sound goes.")
                }
                Toggle(isOn: Binding(get: { current.resumeOnLaunch }, set: { settings.setResumeOnLaunch($0) })) {
                    Text("Resume Playback")
                    Text("Start playing where you left off when Jellyfesh opens.")
                }
            }

            Section("Crossfade") {
                Toggle(isOn: Binding(get: { current.crossfade.enabled }, set: { value in
                    var fade = current.crossfade
                    fade.enabled = value
                    settings.setCrossfade(fade)
                })) {
                    Text("Crossfade")
                    Text("Blend the end of a song into the next.")
                }
                if current.crossfade.enabled {
                    let seconds = crossfadeDraft ?? Double(current.crossfade.durationMs) / 1000
                    LabeledContent("Length") {
                        HStack {
                            Slider(value: Binding(get: { seconds }, set: { crossfadeDraft = $0 }), in: 1...12, step: 1,
                                   onEditingChanged: { editing in
                                       guard !editing, let value = crossfadeDraft else { return }
                                       var fade = settings.current.crossfade
                                       fade.durationMs = UInt32(value * 1000)
                                       settings.setCrossfade(fade)
                                       crossfadeDraft = nil
                                   })
                            .frame(maxWidth: 240)
                            Text("\(Int(seconds)) s")
                                .monospacedDigit()
                                .frame(width: 40, alignment: .trailing)
                        }
                    }
                    Toggle(isOn: Binding(get: { current.crossfade.albumContinuity }, set: { value in
                        var fade = current.crossfade
                        fade.albumContinuity = value
                        settings.setCrossfade(fade)
                    })) {
                        Text("Album Continuity")
                        Text("Songs of the same album join without a fade.")
                    }
                }
            }

            Section {
                Picker("Volume Levelling", selection: Binding(get: { current.replayGain }, set: { settings.setReplayGain($0) })) {
                    Text("Off").tag(ReplayGainOption.off)
                    Text("Track").tag(ReplayGainOption.track)
                    Text("Album").tag(ReplayGainOption.album)
                }
                .pickerStyle(.segmented)
                if current.replayGain != .off {
                    let db = preampDraft ?? Double(current.preampDb)
                    LabeledContent("Preamp") {
                        HStack {
                            Slider(value: Binding(get: { db }, set: { preampDraft = $0 }), in: -15...15, step: 0.5,
                                   onEditingChanged: { editing in
                                       guard !editing, let value = preampDraft else { return }
                                       settings.setPreamp(Float(value))
                                       preampDraft = nil
                                   })
                            .frame(maxWidth: 240)
                            Text(DecibelText.format(db))
                                .monospacedDigit()
                                .frame(width: 64, alignment: .trailing)
                        }
                    }
                }
            } header: {
                Text("ReplayGain")
            } footer: {
                Text("ReplayGain evens out loudness using the tags in your files.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            EqualizerSection()
        }
        .formStyle(.grouped)
    }
}

/// The equalizer: on or off, a preamp and the core's bands (ten one-octave bands by default).
private struct EqualizerSection: View {
    @Environment(SettingsStore.self) private var settings
    @State private var draft: EqualizerConfig?
    @State private var sendTask: Task<Void, Never>?

    /// The gain the sliders allow, in dB (as the Linux app).
    private static let range: ClosedRange<Double> = -12...12

    var body: some View {
        let eq = draft ?? settings.current.eq
        Section {
            Toggle("Equalizer", isOn: Binding(get: { eq.enabled }, set: { value in
                var next = settings.current.eq
                next.enabled = value
                settings.setEqualizer(next)
            }))
            if !eq.bands.isEmpty {
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
            }
        } header: {
            Text("Equalizer")
        }
    }

    private func bandRow(label: String, value: Double, change: @escaping (Double) -> Void,
                         done: @escaping () -> Void) -> some View {
        LabeledContent {
            HStack {
                Slider(value: Binding(get: { value }, set: change), in: Self.range, step: 0.5,
                       onEditingChanged: { editing in if !editing { done() } })
                    .frame(maxWidth: 260)
                Text(DecibelText.format(value))
                    .monospacedDigit()
                    .frame(width: 64, alignment: .trailing)
            }
        } label: {
            Text(label)
                .monospacedDigit()
        }
        .accessibilityElement(children: .combine)
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

/// `+3.0 dB`, `−1.5 dB`.
enum DecibelText {
    static func format(_ db: Double) -> String {
        let value = (db * 10).rounded() / 10
        let sign = value > 0 ? "+" : (value < 0 ? "−" : "")
        return sign + String(format: "%.1f dB", abs(value))
    }
}
