// Volume in the seek bar's style (v0.0.4): the same straight line between two speaker glyphs, ink-90
// over ink-30, with the hover thumb; the quiet glyph mutes. Dragging sends at most one command per
// 50 ms and the final value at the end (PlaybackStore).

import SwiftUI

struct VolumeSlider: View {
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @FocusState private var focused: Bool
    @State private var dragging = false
    @State private var hovering = false

    private static let step: Float = 0.05

    var body: some View {
        let volume = Double(playback.muted ? 0 : playback.displayVolume)
        let state: SeekBarState = dragging ? .drag : (hovering || focused ? .hover : .rest)
        let thickness = SeekGeometry.thickness(state)
        HStack(spacing: Tokens.Space.s3) {
            Button {
                playback.toggleMute()
            } label: {
                Image(systemName: playback.muted ? "speaker.slash.fill" : "speaker.fill")
                    .frame(width: 18)
            }
            .buttonStyle(.plain)
            .foregroundStyle(palette[Tokens.Ink.ink50])
            .accessibilityLabel(playback.muted ? Text("Unmute") : Text("Mute"))
            .help(playback.muted ? Text("Unmute") : Text("Mute"))

            GeometryReader { geometry in
                let width = geometry.size.width
                let midY = geometry.size.height / 2
                let played = SeekGeometry.playedLength(fraction: volume, width: width)
                ZStack(alignment: .topLeading) {
                    Capsule()
                        .fill(palette[Tokens.Ink.ink30])
                        .frame(width: SeekGeometry.trackLength(width: width), height: thickness)
                        .position(x: width / 2, y: midY)
                    if played > 0 {
                        Capsule()
                            .fill(palette[Tokens.Ink.ink90])
                            .frame(width: max(played, thickness), height: thickness)
                            .position(x: SeekGeometry.inset + max(played, thickness) / 2, y: midY)
                    }
                    if SeekGeometry.showsThumb(state) {
                        RingedThumb()
                            .position(x: SeekGeometry.thumbCentre(fraction: volume, width: width), y: midY)
                            .transition(.scale(scale: 0.6).combined(with: .opacity))
                    }
                }
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { value in
                            dragging = true
                            playback.setVolume(Float(SeekGeometry.fraction(atX: value.location.x, width: width)), final: false)
                        }
                        .onEnded { value in
                            dragging = false
                            playback.setVolume(Float(SeekGeometry.fraction(atX: value.location.x, width: width)), final: true)
                        }
                )
            }
            .frame(height: 24)
            .animation(Motion.unless(!effects.animates, Motion.hover), value: state)
            .onHover { hovering = $0 }
            .focusable()
            .focused($focused)
            .onKeyPress(.leftArrow) {
                playback.nudgeVolume(by: -Self.step)
                return .handled
            }
            .onKeyPress(.rightArrow) {
                playback.nudgeVolume(by: Self.step)
                return .handled
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text("Volume"))
            .accessibilityValue(Text(volume, format: .percent.precision(.fractionLength(0))))
            .accessibilityAdjustableAction { direction in
                switch direction {
                case .increment: playback.nudgeVolume(by: Self.step)
                case .decrement: playback.nudgeVolume(by: -Self.step)
                @unknown default: break
                }
            }

            Image(systemName: "speaker.wave.2.fill")
                .frame(width: 18)
                .foregroundStyle(palette[Tokens.Ink.ink50])
                .accessibilityHidden(true)
        }
        .font(.system(size: 13))
    }
}
