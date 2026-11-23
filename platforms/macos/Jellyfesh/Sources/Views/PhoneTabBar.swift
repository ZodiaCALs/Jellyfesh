// The phone layout (ADR-0023) for windows up to 760 pt wide: Jellyfesh Web's tabs Home, Library,
// Search, Sources and Settings in a floating Veil capsule at the bottom of the window (68 pt high,
// 14 pt from its sides, 24 pt above its bottom; Liquid Glass on macOS 26), the chosen tab's glyph on
// an `accent-soft` pill. The Library tab shows the library's place shown last, and its pages start
// with the pill tabs that switch between Albums, Artists, Songs, Playlists, Favourites and
// Downloads (LibrarySwitcher, in PageHeader). A customized look orders the tabs (ADR-0020).

import SwiftUI

struct PhoneTabBar: View {
    @Environment(Router.self) private var router
    @Environment(\.look) private var look

    var body: some View {
        FloatingGlass(cornerRadius: ShellLayout.tabsHeight / 2) {
            HStack(spacing: 0) {
                ForEach(PhoneTab.tabs(for: look), id: \.self) { tab in
                    PhoneTabButton(tab: tab, selected: router.tab == tab) {
                        router.showNowPlaying(false, reduceMotion: true)
                        router.selectTab(tab)
                    }
                }
            }
            .padding(.horizontal, 6)
            .frame(height: ShellLayout.tabsHeight)
            .frame(maxWidth: .infinity)
        }
        .elevation(Tokens.Elevation.shadow2)
        .padding(.horizontal, ShellLayout.tabsSideMargin)
        .padding(.bottom, ShellLayout.tabsBottomMargin)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Library sections"))
    }
}

private struct PhoneTabButton: View {
    var tab: PhoneTab
    var selected: Bool
    var action: () -> Void

    @Environment(\.palette) private var palette
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            VStack(spacing: 3) {
                Image(systemName: tab.symbol)
                    .font(.system(size: 17, weight: .regular))
                    .frame(width: 54, height: 30)
                    .background(Capsule().fill(pill))
                Text(tab.title)
                    .lookFont(size: 11, weight: .semibold)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .foregroundStyle(palette[selected ? Tokens.Accent.accentText : Tokens.Ink.ink70])
            .frame(maxWidth: .infinity, minHeight: 56)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressScaleStyle(scale: 0.97))
        .onHover { hovering = $0 }
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }

    private var pill: Color {
        if selected { return palette[Tokens.Accent.accentSoft] }
        return hovering ? palette[Tokens.Line.fillHover] : .clear
    }
}

/// The Library tab's places as pill tabs, at the top of its pages in the phone layout.
struct LibrarySwitcher: View {
    @Environment(Router.self) private var router

    var body: some View {
        PillTabs(
            selection: Binding(get: { router.selection }, set: { router.select($0) }),
            options: SidebarItem.libraryPlaces.map { PillTabs<SidebarItem>.Option(value: $0, title: $0.title) },
            label: "Library"
        )
    }
}

private struct ShowsLibrarySwitcherKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// The page is one of the Library tab's, in the phone layout: its header starts with the
    /// switcher between them.
    var showsLibrarySwitcher: Bool {
        get { self[ShowsLibrarySwitcherKey.self] }
        set { self[ShowsLibrarySwitcherKey.self] = newValue }
    }
}
