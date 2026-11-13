// Theme colours (Misty PLT-03): every colour comes from the core's Theme slice, resolved from one
// hue; the generated token constants are the names to look them up by (`Tokens.Ink.ink90`).

import SwiftUI

/// The theme colours of one appearance.
struct Palette: Equatable {
    var colors: [String: UInt32]
    var isDark: Bool

    static let fallbackLight = Palette(colors: FallbackTheme.light, isDark: false)
    static let fallbackDark = Palette(colors: FallbackTheme.dark, isDark: true)

    subscript(_ key: ThemeColorKey) -> Color {
        color(named: key.name)
    }

    /// A colour by its theme name (`ink.ink-90`).
    func color(named name: String) -> Color {
        guard let rgba = colors[name] else { return .primary }
        return Color(rgba: rgba)
    }

    /// An ink role the lyric tokens name without their group (`Tokens.Lyrics.inkActive` is `ink-90`).
    func ink(_ role: String) -> Color {
        color(named: "ink.\(role)")
    }

    /// A shadow token's layers for this appearance.
    func layers(_ shadow: Shadow) -> [ShadowLayer] {
        isDark ? shadow.dark : shadow.light
    }

    /// A glass grade's tint alpha for this appearance.
    func alpha(_ themed: Themed) -> Double {
        isDark ? themed.dark : themed.light
    }

    /// The alpha a theme colour carries (a glass tint's token alpha), 0 to 1.
    func alpha(of key: ThemeColorKey) -> Double {
        guard let rgba = colors[key.name] else { return 1 }
        return Double(rgba & 0xFF) / 255
    }

    /// A theme colour with its alpha replaced (the transparency setting's glass tints).
    func color(_ key: ThemeColorKey, alpha: Double) -> Color {
        guard let rgba = colors[key.name] else { return .primary }
        let a = UInt32((min(max(alpha, 0), 1) * 255).rounded())
        return Color(rgba: (rgba & 0xFFFF_FF00) | a)
    }
}

extension Color {
    /// 8-bit sRGB with alpha packed as 0xRRGGBBAA, as the core sends theme colours.
    init(rgba: UInt32) {
        self.init(
            .sRGB,
            red: Double((rgba >> 24) & 0xFF) / 255,
            green: Double((rgba >> 16) & 0xFF) / 255,
            blue: Double((rgba >> 8) & 0xFF) / 255,
            opacity: Double(rgba & 0xFF) / 255
        )
    }
}

/// What the person and the system allow: motion, translucency and live drawing.
struct Effects: Equatable {
    /// The system's Reduce Motion: motion completes instantly (Misty MOT-12), no lyric sweep.
    var reduceMotion = false
    /// Every glass grade becomes solid Cloud: Reduce Transparency, Low Power Mode or the app's
    /// Reduce effects (Misty MAT-08, ADR-0007).
    var solidGlass = false
    /// The app's Reduce effects setting: fewer translucent layers and shorter motion.
    var reduceEffects = false
    /// The window can be seen. When it cannot, nothing time-driven is drawn (Still mode).
    var windowVisible = true
    /// The Transparency setting, 0 (solid) to 1 (clearest); 0.5 keeps the glass tokens' alphas.
    var transparency: Double = 0.5

    /// The Now Playing backdrop crossfades instead of swapping.
    var crossfadesBackdrop: Bool { !reduceMotion && !reduceEffects }
    /// The karaoke sweep runs; otherwise the whole active line lights at once.
    var sweepsLyrics: Bool { !reduceMotion }
    /// Motion runs: no short fades in its place (the system's Reduce Motion or Reduce effects).
    var animates: Bool { !reduceMotion && !reduceEffects }
}

private struct PaletteKey: EnvironmentKey {
    static let defaultValue = Palette.fallbackLight
}

private struct EffectsKey: EnvironmentKey {
    static let defaultValue = Effects()
}

extension EnvironmentValues {
    var palette: Palette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }

    var effects: Effects {
        get { self[EffectsKey.self] }
        set { self[EffectsKey.self] = newValue }
    }
}
