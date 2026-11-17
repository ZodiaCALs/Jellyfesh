// Control style D, "Blend" (docs/design-controls.md, design/canvas/Controls-D.dc.html): every
// button and bar shape of the player lives in this file and SeekBar.swift, so another style is a
// change here only (the choice is still open; it becomes a `controls.style` token group).
//
// Misty rules kept: one accent fill per view (the play button, COL-04); accent-soft only behind
// accent glyphs (COL-06); 44 pt targets (ACC-12); Spring only for presses (MOT-05).

import SwiftUI

/// Round glyph button: rest, hover (soft fill), pressed (92 %), disabled.
struct IconButtonStyle: ButtonStyle {
    var size: CGFloat = 44
    var glyph: CGFloat = 18
    var prominent = false

    func makeBody(configuration: Configuration) -> some View {
        IconButtonBody(configuration: configuration, size: size, glyph: glyph, prominent: prominent, isOn: nil)
    }
}

/// A toggle (shuffle, repeat, lyrics): accent glyph on an accent-soft circle while on.
struct ToggleButtonStyle: ButtonStyle {
    var isOn: Bool
    var size: CGFloat = 44
    var glyph: CGFloat = 18

    func makeBody(configuration: Configuration) -> some View {
        IconButtonBody(configuration: configuration, size: size, glyph: glyph, prominent: false, isOn: isOn)
    }
}

private struct IconButtonBody: View {
    let configuration: ButtonStyleConfiguration
    let size: CGFloat
    let glyph: CGFloat
    let prominent: Bool
    let isOn: Bool?
    @State private var hovering = false
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.effects) private var effects

    var body: some View {
        configuration.label
            .labelStyle(.iconOnly)
            .font(.system(size: glyph, weight: .regular))
            .foregroundStyle(foreground)
            .frame(width: size, height: size)
            .background(Circle().fill(background))
            .contentShape(Circle())
            .scaleEffect(configuration.isPressed ? 0.92 : 1)
            .animation(Motion.unless(effects.reduceMotion, Motion.press), value: configuration.isPressed)
            .animation(Motion.unless(effects.reduceMotion, Motion.hover), value: hovering)
            .onHover { hovering = $0 }
            .opacity(isEnabled ? 1 : 0.4)
    }

    private var foreground: Color {
        if isOn == true { return palette[Tokens.Accent.accentText] }
        if prominent || hovering || configuration.isPressed { return palette[Tokens.Ink.ink90] }
        return palette[Tokens.Ink.ink70]
    }

    private var background: Color {
        if isOn == true { return palette[Tokens.Accent.accentSoft] }
        if configuration.isPressed { return palette[Tokens.Ink.ink10] }
        if hovering { return palette[Tokens.Ink.ink06] }
        return .clear
    }
}

/// The one accent-filled action on Now Playing: play and pause. Hover adds a soft halo; a press
/// darkens and scales to 92 %.
struct PlayButtonStyle: ButtonStyle {
    var size: CGFloat = 60

    func makeBody(configuration: Configuration) -> some View {
        PlayButtonBody(configuration: configuration, size: size)
    }
}

private struct PlayButtonBody: View {
    let configuration: ButtonStyleConfiguration
    let size: CGFloat
    @State private var hovering = false
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        configuration.label
            .labelStyle(.iconOnly)
            .font(.system(size: size * 0.4, weight: .semibold))
            .foregroundStyle(palette[Tokens.Accent.onAccent])
            .frame(width: size, height: size)
            .background(
                Circle().fill(palette[configuration.isPressed
                    ? Tokens.Accent.accentPress
                    : (hovering ? Tokens.Accent.accentHover : Tokens.Accent.accent)])
            )
            .background(
                Circle()
                    .fill(palette[Tokens.Accent.accentSoft])
                    .padding(hovering && !configuration.isPressed ? -6 : 0)
            )
            .contentShape(Circle())
            .scaleEffect(configuration.isPressed ? 0.92 : 1)
            .animation(Motion.unless(effects.reduceMotion, Motion.press), value: configuration.isPressed)
            .animation(Motion.unless(effects.reduceMotion, Motion.hover), value: hovering)
            .onHover { hovering = $0 }
    }
}

/// A pill with a glyph and a label (the Speakers chip of the Now Playing bar).
struct ChipButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        ChipBody(configuration: configuration)
    }
}

private struct ChipBody: View {
    let configuration: ButtonStyleConfiguration
    @State private var hovering = false
    @Environment(\.palette) private var palette

    var body: some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(palette[Tokens.Ink.ink90])
            .padding(.leading, 10)
            .padding(.trailing, 14)
            .frame(height: 36)
            .background(Capsule().fill(palette[hovering ? Tokens.Ink.ink10 : Tokens.Ink.ink06]))
            .overlay(Capsule().strokeBorder(palette[Tokens.Line.lineSoft], lineWidth: 1))
            .contentShape(Capsule())
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .onHover { hovering = $0 }
    }
}

/// The chip's shape for a label that is not a button (the Speakers menu).
struct ChipLabel: ViewModifier {
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(palette[Tokens.Ink.ink90])
            .padding(.leading, 10)
            .padding(.trailing, 14)
            .frame(height: 36)
            .background(Capsule().fill(palette[Tokens.Ink.ink06]))
            .overlay(Capsule().strokeBorder(palette[Tokens.Line.lineSoft], lineWidth: 1))
            .contentShape(Capsule())
    }
}

extension View {
    func chipLabel() -> some View {
        modifier(ChipLabel())
    }
}

/// Misty's segmented control: a quiet track with the chosen segment raised on Cloud.
struct SegmentedControl<Value: Hashable>: View {
    struct Option: Identifiable {
        var value: Value
        var title: LocalizedStringKey
        var id: Value { value }
    }

    @Binding var selection: Value
    var options: [Option]
    var label: LocalizedStringKey
    @Namespace private var namespace
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        HStack(spacing: 0) {
            ForEach(options) { option in
                let selected = option.value == selection
                Button {
                    withAnimation(Motion.unless(effects.reduceMotion, Motion.move)) {
                        selection = option.value
                    }
                } label: {
                    Text(option.title)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(palette[selected ? Tokens.Ink.ink90 : Tokens.Ink.ink70])
                        .padding(.horizontal, 16)
                        .frame(height: 30)
                        .background {
                            if selected {
                                RoundedRectangle(cornerRadius: 7, style: .continuous)
                                    .fill(palette[Tokens.Glass.cloud])
                                    .elevation(Tokens.Elevation.shadow1)
                                    .matchedGeometryEffect(id: "selection", in: namespace)
                            }
                        }
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(3)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(palette[Tokens.Ink.ink06]))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(palette[Tokens.Line.lineSoft], lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text(label))
    }
}

/// A section heading on library pages (`type-roles.section`).
struct SectionTitle: View {
    var title: LocalizedStringKey
    @Environment(\.palette) private var palette

    init(_ title: LocalizedStringKey) {
        self.title = title
    }

    var body: some View {
        Text(title)
            .typography(Tokens.TypeRoles.section)
            .foregroundStyle(palette[Tokens.Ink.ink90])
            .accessibilityAddTraits(.isHeader)
    }
}
