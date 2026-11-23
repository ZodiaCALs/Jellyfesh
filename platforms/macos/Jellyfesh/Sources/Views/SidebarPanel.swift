// The floating sidebar panel (ADR-0023, Jellyfesh Web's), in windows wider than 760 pt: the
// wordmark, then Home, Albums, Artists, Songs, Playlists, Favourites and Downloads, a line, Search
// and Sources, and at its foot Open Files… and Settings; while the core scans, the scan's progress
// above them. A panel of Clear glass with `r-2xl` corners and `shadow-1`, 12 pt from the window's
// edges (Luminous; Liquid Glass on macOS 26, solid Cloud when transparency is reduced). The places
// are the web's rows: 42 pt high, the chosen one in accent text on `accent-soft`. A customized look
// lists the places it shows in its order (ADR-0020).

import AppKit
import SwiftUI

struct SidebarPanel: View {
    @Environment(AppModel.self) private var model
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Wordmark()
                .padding(.horizontal, 10)
                .padding(.bottom, Tokens.Space.s6)
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 3) {
                    ForEach(SidebarItem.panelEntries(for: look), id: \.self) { entry in
                        switch entry {
                        case .place(let item):
                            PanelItem(title: item.title, symbol: item.symbol, selected: router.selection == item) {
                                show(item)
                            }
                        case .separator:
                            Rectangle()
                                .fill(palette[Tokens.Line.lineSoft])
                                .frame(height: 1)
                                .padding(.horizontal, Tokens.Space.s3)
                                .padding(.vertical, Tokens.Space.s2)
                                .accessibilityHidden(true)
                        }
                    }
                }
            }
            .scrollIndicators(.never)
            ScanStatus()
            VStack(alignment: .leading, spacing: 2) {
                PanelItem(title: "Open Files…", symbol: "doc.badge.plus", selected: false) {
                    FilePanels.chooseAudioFiles { urls in model.open(urls) }
                }
                .disabled(model.startupError != nil)
                PanelItem(title: SidebarItem.settings.title, symbol: SidebarItem.settings.symbol,
                          selected: router.selection == .settings) {
                    show(.settings)
                }
            }
            .padding(.top, Tokens.Space.s2)
        }
        .padding(.vertical, Tokens.Space.s5)
        .padding(.horizontal, Tokens.Space.s3)
        .frame(width: ShellLayout.sidebarWidth)
        .frame(maxHeight: .infinity, alignment: .top)
        .modifier(PanelGlass())
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Library sections"))
    }

    private func show(_ item: SidebarItem) {
        router.showNowPlaying(false, reduceMotion: true)
        router.select(item)
    }
}

/// The app's icon and name at the top of the panel. The icon is the one chosen in Settings ›
/// Appearance › App Icon (Automatic: Steel in a dark window, Ice in a light one).
private struct Wordmark: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 11) {
            Image(AppIconChoice.wordmarkThumbnail(for: settings.current.appIcon, dark: palette.isDark))
                .resizable()
                .interpolation(.high)
                .frame(width: 32, height: 32)
                .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                .accessibilityHidden(true)
            Text(verbatim: "Jellyfesh")
                .lookFont(size: 16, weight: .bold)
                .tracking(-0.16)
                .foregroundStyle(palette[Tokens.Ink.ink90])
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// A row of the panel: the place's glyph and name; on hover a soft fill, when chosen accent text on
/// `accent-soft`, semibold.
private struct PanelItem: View {
    var title: LocalizedStringKey
    var symbol: String
    var selected: Bool
    var action: () -> Void

    @Environment(\.palette) private var palette
    @Environment(\.look) private var look
    @Environment(\.isEnabled) private var isEnabled
    @State private var hovering = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: look.radius(Tokens.Radius.rLg), style: .continuous)
        Button(action: action) {
            HStack(spacing: Tokens.Space.s3) {
                Image(systemName: symbol)
                    .font(.system(size: 16, weight: .regular))
                    .frame(width: 22)
                Text(title)
                    .lookFont(size: 14.5, weight: selected ? .semibold : .medium)
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(foreground)
            .padding(.horizontal, Tokens.Space.s3)
            .frame(minHeight: 42)
            .background(shape.fill(background))
            .contentShape(shape)
        }
        .buttonStyle(PressScaleStyle(scale: 0.97))
        .onHover { hovering = $0 }
        .opacity(isEnabled ? 1 : 0.4)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }

    private var foreground: Color {
        if selected { return palette[Tokens.Accent.accentText] }
        return palette[hovering ? Tokens.Ink.ink90 : Tokens.Ink.ink70]
    }

    private var background: Color {
        if selected { return palette[Tokens.Accent.accentSoft] }
        return hovering ? palette[Tokens.Line.fillHover] : .clear
    }
}

/// The panel's glass: Clear with `shadow-1` (Luminous), Liquid Glass on macOS 26 (ADR-0007), solid
/// Cloud when transparency is reduced.
private struct PanelGlass: ViewModifier {
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look

    @ViewBuilder func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: look.radius(Tokens.Radius.r2xl), style: .continuous)
        if effects.solidGlass {
            content
                .glass(.clear, in: shape)
                .elevation(Tokens.Elevation.shadow1)
        } else {
            liquidOrClear(content, shape: shape)
        }
    }

    #if compiler(>=6.2)
    @ViewBuilder private func liquidOrClear(_ content: Content, shape: RoundedRectangle) -> some View {
        if #available(macOS 26.0, *) {
            GlassEffectContainer {
                content.glassEffect(.regular, in: shape)
            }
        } else {
            content
                .glass(.clear, in: shape)
                .elevation(Tokens.Elevation.shadow1)
        }
    }
    #else
    private func liquidOrClear(_ content: Content, shape: RoundedRectangle) -> some View {
        content
            .glass(.clear, in: shape)
            .elevation(Tokens.Elevation.shadow1)
    }
    #endif
}

/// Scan progress above the panel's foot while the core scans.
private struct ScanStatus: View {
    @Environment(LibraryStore.self) private var library
    @Environment(\.palette) private var palette

    var body: some View {
        if library.scanning {
            HStack(spacing: Tokens.Space.s2) {
                ProgressView()
                    .controlSize(.small)
                VStack(alignment: .leading, spacing: 0) {
                    Text("Scanning Library")
                        .lookFont(size: 11, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                    if case .progress(let seen, _, _)? = library.scan {
                        Text("Files checked: \(seen)")
                            .lookFont(size: 11)
                            .foregroundStyle(palette[Tokens.Ink.ink70])
                            .monospacedDigit()
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Tokens.Space.s3)
            .padding(.vertical, Tokens.Space.s2)
            .accessibilityElement(children: .combine)
        }
    }
}
