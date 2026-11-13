// The theme the core resolves from the artwork hue (Misty PLT-03): every colour for light and dark
// appearance and the baked Now Playing backdrops. Until the first snapshot the hue-250 theme is
// used. Theme colours crossfade on dur-4 with the Fluid curve; under reduced motion, a short fade.

import SwiftUI
import Observation

@MainActor
@Observable
final class ThemeStore {
    private(set) var theme: ThemeState?
    /// Set by the window from the system's Reduce Motion and the app's Reduce effects.
    @ObservationIgnored var reduceMotion = false

    func apply(_ value: ThemeState?) {
        guard value != theme else { return }
        withAnimation(reduceMotion ? Motion.shortFade : Motion.themeChange) {
            theme = value
        }
    }

    /// The colours of one appearance.
    func palette(dark: Bool) -> Palette {
        guard let theme else { return dark ? .fallbackDark : .fallbackLight }
        let colors = dark ? theme.dark.colors : theme.light.colors
        // A theme without colours (an older core) still draws with the fallback.
        guard !colors.isEmpty else { return dark ? .fallbackDark : .fallbackLight }
        return Palette(colors: colors, isDark: dark)
    }

    func palette(for scheme: ColorScheme) -> Palette {
        palette(dark: scheme == .dark)
    }

    /// The baked backdrop for an appearance: blurred artwork tone-mapped so the ink holds 4.5:1.
    func backdrop(dark: Bool) -> String? {
        dark ? theme?.backdropDark : theme?.backdropLight
    }

    /// The hue the theme was resolved at.
    var hue: Float { theme?.hue ?? Float(Tokens.defaultHue) }
}
