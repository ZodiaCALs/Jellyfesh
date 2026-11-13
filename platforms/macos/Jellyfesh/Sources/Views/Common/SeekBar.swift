// The seek bar of v0.0.4 (docs/design-controls.md, "Seek bar and sliders: a straight line"):
// elapsed and remaining times at both ends, a straight line with round caps, the played part in
// ink-90 over ink-30, no thumb at rest. Hovering or focusing thickens the line to 6 pt and shows a
// 14 pt ringed thumb (after Media Player); dragging thickens it to 8 pt and the times follow the
// thumb. Thickness and thumb change over dur-1 Fluid; progress itself never animates. It redraws
// when the position moves by a pixel or a second ticks over (ProgressSchedule), never per frame,
// and not at all while paused or out of sight. Geometry: SeekGeometry (Support/DesignMath.swift).

import SwiftUI

struct SeekBar: View {
    /// The thumb is the one Liquid Glass accent (deviation D2, Artwork style only).
    var liquidThumb = false
    /// The line's rest colour: ink-30 on glass and backdrops (`line-strong` on plain surfaces).
    var rest: ThemeColorKey = Tokens.Ink.ink30

    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Environment(\.displayScale) private var displayScale
    @State private var trackWidth: CGFloat = 0
    @State private var dragFraction: Double?
    @State private var hovering = false
    @FocusState private var focused: Bool

    var body: some View {
        let start = playback.scheduleStart()
        let duration = playback.durationUs
        let step = barStepUs(durationUs: duration, lengthPoints: SeekGeometry.trackLength(width: trackWidth),
                             scale: displayScale)
        TimelineView(ProgressSchedule(start: start, stepUs: step, seconds: true, active: effects.windowVisible)) { _ in
            bar(position: playback.positionUs(), duration: duration)
        }
    }

    private var state: SeekBarState {
        if dragFraction != nil { return .drag }
        return hovering || focused ? .hover : .rest
    }

    private func bar(position: Int64, duration: Int64?) -> some View {
        let total = max(duration ?? 0, 0)
        let fraction = dragFraction ?? (total > 0 ? min(max(Double(position) / Double(total), 0), 1) : 0)
        let shown = dragFraction.map { Int64($0 * Double(total)) } ?? position
        let elapsed = TimeText.from(microseconds: shown)
        return HStack(spacing: Tokens.Space.s3) {
            Text(elapsed)
                .frame(minWidth: 40, alignment: .trailing)
            track(fraction: fraction, total: total)
            Text(total > 0 ? TimeText.remaining(positionUs: shown, durationUs: total) : "–:––")
                .frame(minWidth: 44, alignment: .leading)
        }
        .font(.system(size: 12, weight: .semibold))
        .monospacedDigit()
        .foregroundStyle(palette[Tokens.Ink.ink70])
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text("Position"))
        .accessibilityValue(Text(total > 0
            ? String(localized: "\(elapsed) of \(TimeText.from(microseconds: total))")
            : elapsed))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: seek(.forward, total: total)
            case .decrement: seek(.back, total: total)
            @unknown default: break
            }
        }
    }

    private func track(fraction: Double, total: Int64) -> some View {
        let thickness = SeekGeometry.thickness(state)
        let showThumb = total > 0 && SeekGeometry.showsThumb(state)
        let ink = palette[Tokens.Ink.ink90]
        let restColor = palette[rest]
        return GeometryReader { geometry in
            let width = geometry.size.width
            let midY = geometry.size.height / 2
            let played = SeekGeometry.playedLength(fraction: fraction, width: width)
            ZStack(alignment: .topLeading) {
                Capsule()
                    .fill(restColor)
                    .frame(width: SeekGeometry.trackLength(width: width), height: thickness)
                    .position(x: width / 2, y: midY)
                if played > 0 {
                    Capsule()
                        .fill(ink)
                        .frame(width: max(played, thickness), height: thickness)
                        .position(x: SeekGeometry.inset + max(played, thickness) / 2, y: midY)
                }
                if showThumb {
                    RingedThumb(liquid: liquidThumb)
                        .position(x: SeekGeometry.thumbCentre(fraction: fraction, width: width), y: midY)
                        .transition(.scale(scale: 0.6).combined(with: .opacity))
                }
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        dragFraction = SeekGeometry.fraction(atX: value.location.x, width: width)
                    }
                    .onEnded { value in
                        let target = SeekGeometry.fraction(atX: value.location.x, width: width)
                        dragFraction = nil
                        if total > 0 {
                            playback.seek(toUs: Int64(target * Double(total)))
                        }
                    }
            )
        }
        .frame(height: 24)
        .background {
            GeometryReader { geometry in
                Color.clear
                    .onAppear { trackWidth = geometry.size.width }
                    .onChange(of: geometry.size.width) { _, width in trackWidth = width }
            }
        }
        .animation(Motion.unless(!effects.animates, Motion.hover), value: state)
        .onHover { hovering = $0 }
        .focusable()
        .focused($focused)
        .onKeyPress(.leftArrow) { seek(.back, total: total) }
        .onKeyPress(.rightArrow) { seek(.forward, total: total) }
        .onKeyPress(.pageDown) { seek(.pageBack, total: total) }
        .onKeyPress(.pageUp) { seek(.pageForward, total: total) }
        .onKeyPress(.home) { seek(.start, total: total) }
        .onKeyPress(.end) { seek(.end, total: total) }
    }

    /// Left/Right 5 s, Page Down/Page Up 30 s, Home and End: from the drawn position, within the track.
    @discardableResult
    private func seek(_ key: SeekKey, total: Int64) -> KeyPress.Result {
        guard total > 0 else { return .ignored }
        playback.seek(toUs: SeekGeometry.keyTarget(key, positionUs: playback.positionUs(), durationUs: total))
        return .handled
    }
}

/// The hover thumb (after Media Player): ink-90 with a glass-edge ring and shadow-1; Liquid Glass on
/// macOS 26 in the Artwork style.
struct RingedThumb: View {
    var size: CGFloat = SeekGeometry.thumb
    var liquid = false
    @Environment(\.palette) private var palette

    var body: some View {
        Circle()
            .fill(palette[Tokens.Ink.ink90])
            .overlay(Circle().strokeBorder(palette[Tokens.Glass.glassEdge], lineWidth: 2))
            .frame(width: size, height: size)
            .modifier(LiquidThumb(enabled: liquid))
            .elevation(Tokens.Elevation.shadow1)
            .accessibilityHidden(true)
    }
}
