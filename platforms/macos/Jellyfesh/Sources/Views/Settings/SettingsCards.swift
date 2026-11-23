// Settings drawn as Jellyfesh Web draws them (ADR-0023, docs/ui.md "Settings"; Luminous): each
// group a card of the Mist tint with `r-2xl` corners and `shadow-1` (`surfaceCard`), its heading
// inside; rows of a label with a line of help and the control at the end, at least 60 pt high,
// hairlines between them. A section's cards scroll under the page's title and tabs.

import SwiftUI

/// A section's groups, one under another, in Settings' column.
struct SettingsCardsColumn<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s5) {
                content
            }
            // The page insets grouped forms by their own margin; cards take it here, so their edges
            // line up with the title.
            .padding(.horizontal, SettingsPage.formInset)
            .padding(.top, Tokens.Space.s5)
            .padding(.bottom, Tokens.Space.s7)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// A group: its heading (with an optional control at its end, such as Install from File…) and an
/// optional note, then its rows, in a Mist card.
struct SettingsCard<Content: View, Accessory: View>: View {
    var title: LocalizedStringKey?
    var note: Text?
    @ViewBuilder var accessory: Accessory
    @ViewBuilder var content: Content

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s1) {
            if let title {
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s3) {
                    Text(title)
                        .typography(Tokens.TypeRoles.section)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 0)
                    accessory
                }
            }
            if let note {
                note
                    .lookFont(size: 13)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
            VStack(alignment: .leading, spacing: 0) {
                content
            }
        }
        .padding(.top, Tokens.Space.s5)
        .padding(.horizontal, Tokens.Space.s6)
        .padding(.bottom, Tokens.Space.s2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .surfaceCard()
    }
}

extension SettingsCard where Accessory == EmptyView {
    init(title: LocalizedStringKey? = nil, note: Text? = nil, @ViewBuilder content: () -> Content) {
        self.init(title: title, note: note, accessory: { EmptyView() }, content: content)
    }
}

/// A row: its label and a line of help, and its control at the end (or under them, `stacked`).
struct SettingsRow<Control: View>: View {
    var title: Text
    var help: Text?
    /// The control goes under the label, the width of the card (sliders, pictures, lists).
    var stacked = false
    @ViewBuilder var control: Control

    @Environment(\.palette) private var palette

    var body: some View {
        Group {
            if stacked {
                VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                    label
                    control
                }
            } else {
                HStack(alignment: .center, spacing: Tokens.Space.s3) {
                    label
                        .frame(maxWidth: .infinity, alignment: .leading)
                    control
                }
            }
        }
        .padding(.vertical, Tokens.Space.s3)
        .frame(minHeight: 60)
    }

    private var label: some View {
        VStack(alignment: .leading, spacing: 2) {
            title
                .lookFont(size: 15, weight: .semibold)
                .foregroundStyle(palette[Tokens.Ink.ink90])
            if let help {
                help
                    .lookFont(size: 13)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

extension SettingsRow {
    init(_ title: LocalizedStringKey, help: LocalizedStringKey? = nil, stacked: Bool = false,
         @ViewBuilder control: () -> Control) {
        self.init(title: Text(title), help: help.map { Text($0) }, stacked: stacked, control: control)
    }
}

/// A switch row: the label and its help, and the switch at the end.
struct SettingsToggleRow: View {
    var title: LocalizedStringKey
    var help: LocalizedStringKey?
    @Binding var isOn: Bool

    init(_ title: LocalizedStringKey, help: LocalizedStringKey? = nil, isOn: Binding<Bool>) {
        self.title = title
        self.help = help
        _isOn = isOn
    }

    var body: some View {
        SettingsRow(title, help: help) {
            Toggle(isOn: $isOn) {
                Text(title)
            }
            .toggleStyle(.switch)
            .labelsHidden()
        }
        .accessibilityElement(children: .combine)
    }
}

/// The hairline between two rows of a card.
struct SettingsDivider: View {
    @Environment(\.palette) private var palette

    var body: some View {
        Rectangle()
            .fill(palette[Tokens.Line.lineSoft])
            .frame(height: 1)
            .accessibilityHidden(true)
    }
}
