// The look (ADR-0020, docs/ui.md "The Look slice"): the core's resolved values as the numbers the
// views draw with: radii × the corner scale, glass that the Transparency setting and Reduce
// Transparency still overrule, the aurora's strength, the platform's own faces (SF, SF Rounded,
// New York, SF Mono) at the text scale, spacing for the density, the lyrics' weight, glow and
// highlight, and the order of the sidebar and Home. With `customized` false every value is the
// Luminous default and the app draws exactly as in v0.0.6. The rules are pure, for tests.

import SwiftUI

struct LookStyle: Equatable {
    var state: LookState

    static let luminous = LookStyle(state: .luminous)

    var customized: Bool { state.customized }

    // MARK: Shapes, glass and the field

    /// Radii ×, 0–2 (1 is Luminous).
    var cornerScale: CGFloat { CGFloat(Self.clamp(state.cornerScale, 0, 2, otherwise: 1)) }

    /// A radius in this look.
    func radius(_ base: CGFloat) -> CGFloat { base * cornerScale }

    /// How much glass shows what is behind it, 0 (solid panels) to 1 (Luminous).
    var glass: Double { Double(Self.clamp(state.glass, 0, 1, otherwise: 1)) }

    /// The Transparency the glass is drawn with: the setting's, times the look's glass, so the
    /// setting and Reduce Transparency still win (ADR-0020).
    func transparency(setting: Double) -> Double {
        min(max(setting, 0), 1) * glass
    }

    /// The field's aurora strength (`field.aurora-strength` while not customized).
    var aurora: Double {
        guard customized else { return Tokens.Field.auroraStrength }
        return Double(Self.clamp(state.aurora, 0, 1, otherwise: Float(Tokens.Field.auroraStrength)))
    }

    // MARK: Type and spacing

    /// The platform's own face for the typeface: SF, SF Rounded, New York or SF Mono.
    var fontDesign: Font.Design {
        switch state.typeface {
        case .system: return .default
        case .rounded: return .rounded
        case .serif: return .serif
        case .mono: return .monospaced
        }
    }

    /// Every type size ×, 0.85–1.30.
    var textScale: CGFloat { CGFloat(Self.clamp(state.textScale, 0.85, 1.3, otherwise: 1)) }

    /// A type size in this look.
    func size(_ base: CGFloat) -> CGFloat { base * textScale }

    /// A system font in this look's face, at this look's scale.
    func font(size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: self.size(size), weight: weight, design: fontDesign)
    }

    /// Spacing × 0.8, 1 or 1.2 for the density.
    var densityScale: CGFloat {
        switch state.density {
        case .compact: return 0.8
        case .standard: return 1
        case .roomy: return 1.2
        }
    }

    /// A spacing in this look.
    func spacing(_ base: CGFloat) -> CGFloat { base * densityScale }

    // MARK: Lyrics

    /// The lyrics' weight: the token's while not customized.
    func lyricsWeight(token: Int) -> Font.Weight {
        guard customized else { return Font.Weight(cssWeight: token) }
        switch state.lyricsWeight {
        case .regular: return .regular
        case .medium: return .medium
        case .semibold: return .semibold
        case .bold: return .bold
        case .heavy: return .heavy
        }
    }

    /// The colour of sung words: the lyrics' sung ink for Artwork (v0.0.6), the accent, or the
    /// person's colour.
    func sungInk(_ palette: Palette) -> Color {
        guard customized else { return palette.ink(Tokens.Lyrics.inkSung) }
        switch state.lyricsHighlight {
        case .artwork: return palette.ink(Tokens.Lyrics.inkSung)
        case .accent: return palette[Tokens.Accent.accentText]
        case .custom(let rgb): return Color(rgba: ((rgb & 0xFF_FFFF) << 8) | 0xFF)
        }
    }

    /// The soft light around a sung line: its radius for a font size (none while not customized).
    func glowRadius(forSize size: CGFloat) -> CGFloat {
        guard customized else { return 0 }
        return size * 0.35 * CGFloat(Self.clamp(state.lyricsGlow, 0, 1, otherwise: 0))
    }

    /// The glow's colour: the highlight, half seen.
    func glowColor(_ palette: Palette) -> Color {
        customized ? sungInk(palette).opacity(0.5) : .clear
    }

    // MARK: Places

    /// The sidebar's places, in order: the look's, each once; Luminous's when none of the
    /// library's places would be left (the core keeps one, so this only guards a bad answer).
    var sidebar: [LookPlace] {
        var seen = Set<LookPlace>()
        let places = state.sidebar.filter { seen.insert($0).inserted }
        let library: Set<LookPlace> = [.home, .albums, .artists, .songs, .playlists, .favourites]
        return places.contains(where: library.contains) ? places : LookPlace.allCases
    }

    /// Home's sections, in order, each once.
    var home: [LookHomeSection] {
        var seen = Set<LookHomeSection>()
        return state.home.filter { seen.insert($0).inserted }
    }

    private static func clamp(_ value: Float, _ low: Float, _ high: Float, otherwise: Float) -> Float {
        value.isFinite ? min(max(value, low), high) : otherwise
    }
}

private struct LookKey: EnvironmentKey {
    static let defaultValue = LookStyle.luminous
}

extension EnvironmentValues {
    var look: LookStyle {
        get { self[LookKey.self] }
        set { self[LookKey.self] = newValue }
    }
}

/// A system font in the look's face and scale.
private struct LookFont: ViewModifier {
    var size: CGFloat
    var weight: Font.Weight
    @Environment(\.look) private var look

    func body(content: Content) -> some View {
        content.font(look.font(size: size, weight: weight))
    }
}

/// The soft light around sung lyrics, when the look has one.
private struct LyricsGlow: ViewModifier {
    var color: Color
    var radius: CGFloat

    @ViewBuilder func body(content: Content) -> some View {
        if radius > 0 {
            content.shadow(color: color, radius: radius)
        } else {
            content
        }
    }
}

/// A grouped surface (Luminous's lists and cards) with the look's corners.
private struct SurfaceCard: ViewModifier {
    @Environment(\.look) private var look

    func body(content: Content) -> some View {
        content.surface(in: RoundedRectangle(cornerRadius: look.radius(surfaceRadius), style: .continuous))
    }
}

extension View {
    /// `font(.system(size:weight:))` in the look's face and at its text scale.
    func lookFont(size: CGFloat, weight: Font.Weight = .regular) -> some View {
        modifier(LookFont(size: size, weight: weight))
    }

    /// A grouped surface with Luminous's corner radius, scaled by the look.
    func surfaceCard() -> some View {
        modifier(SurfaceCard())
    }

    /// The look's glow around sung lyrics drawn at `size` points (none while not customized).
    func lyricsGlow(_ look: LookStyle, palette: Palette, size: CGFloat) -> some View {
        modifier(LyricsGlow(color: look.glowColor(palette), radius: look.glowRadius(forSize: size)))
    }
}
