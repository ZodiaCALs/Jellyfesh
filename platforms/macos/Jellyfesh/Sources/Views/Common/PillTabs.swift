// Jellyfesh Web's pill tabs (ADR-0023): Settings' sections and, in narrow windows, the Library
// tab's places. A quiet capsule track (`line.fill`) with the chosen tab raised on Cloud with
// `shadow-1`; the tabs share the row while they fit, and the row scrolls sideways when they do not.

import SwiftUI

struct PillTabs<Value: Hashable>: View {
    struct Option: Identifiable {
        var value: Value
        var title: Text
        var id: Value { value }

        init(value: Value, title: LocalizedStringKey) {
            self.value = value
            self.title = Text(title)
        }

        /// A name shown as it is (a source's).
        init(value: Value, verbatim: String) {
            self.value = value
            self.title = Text(verbatim: verbatim)
        }
    }

    @Binding var selection: Value
    var options: [Option]
    /// What the tabs choose, for VoiceOver.
    var label: LocalizedStringKey

    var body: some View {
        ViewThatFits(in: .horizontal) {
            PillTabsRow(selection: $selection, options: options, stretch: true)
            ScrollView(.horizontal) {
                PillTabsRow(selection: $selection, options: options, stretch: false)
            }
            .scrollIndicators(.hidden)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text(label))
    }
}

/// One row of pill tabs: stretched to share the width, or at their own widths to scroll.
private struct PillTabsRow<Value: Hashable>: View {
    @Binding var selection: Value
    var options: [PillTabs<Value>.Option]
    var stretch: Bool

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s1) {
            ForEach(options) { option in
                PillTab(title: option.title, selected: option.value == selection, stretch: stretch) {
                    selection = option.value
                }
            }
        }
        .padding(Tokens.Space.s1)
        .background(Capsule().fill(palette[Tokens.Line.fill]))
    }
}

private struct PillTab: View {
    var title: Text
    var selected: Bool
    var stretch: Bool
    var action: () -> Void

    @Environment(\.palette) private var palette
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            title
                .lookFont(size: 14, weight: .semibold)
                .lineLimit(1)
                .fixedSize()
                .foregroundStyle(palette[selected || hovering ? Tokens.Ink.ink90 : Tokens.Ink.ink70])
                .padding(.horizontal, stretch ? Tokens.Space.s3 : Tokens.Space.s4)
                .frame(maxWidth: stretch ? .infinity : nil, minHeight: 36)
                .background {
                    if selected {
                        Capsule()
                            .fill(palette[Tokens.Glass.cloud])
                            .elevation(Tokens.Elevation.shadow1)
                    }
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }
}
