// Glass per ADR-0007: Misty's grades on the nearest system material with the grade's tint on
// top; solid Cloud when transparency is reduced (MAT-08). One edge, one shadow, nothing animated
// on the glass itself (MAT-04, MAT-10). macOS 26's Liquid Glass is used where the ADR allows it.
// The Transparency setting (v0.0.4) moves each grade to a thicker or thinner material and sets
// its tint's alpha from the shared glass-alpha port (GlassLook, GlassTransparency); at 0 % the
// glass is solid Cloud.

import SwiftUI

/// Misty's glass grades, clearest first.
enum GlassLevel {
    /// Frames (the Misty artwork frame): `.ultraThinMaterial` at the default transparency.
    case clear
    /// Cards: `.thinMaterial` at the default transparency.
    case mist
    /// Bars and the mini player: `.regularMaterial` at the default transparency.
    case veil

    var tint: ThemeColorKey {
        switch self {
        case .clear: return Tokens.Glass.tintClear
        case .mist: return Tokens.Glass.tintMist
        case .veil: return Tokens.Glass.tintVeil
        }
    }

    /// The grade's usual material, as a thickness the transparency setting can move.
    var baseThickness: GlassLook.Thickness {
        switch self {
        case .clear: return .ultraThin
        case .mist: return .thin
        case .veil: return .regular
        }
    }

    /// The material at `transparency`, or nil for solid Cloud.
    func material(transparency: Double) -> Material? {
        GlassLook.thickness(base: baseThickness, transparency: transparency).map(Self.systemMaterial)
    }

    /// The tint at `transparency`: the theme's tint colour with the port's alpha.
    func tintColor(_ palette: Palette, transparency: Double) -> Color {
        let key = self.tint
        return palette.color(key, alpha: GlassTransparency.alpha(palette.alpha(of: key), transparency: transparency))
    }

    static func systemMaterial(_ thickness: GlassLook.Thickness) -> Material {
        switch thickness {
        case .ultraThin: return .ultraThinMaterial
        case .thin: return .thinMaterial
        case .regular: return .regularMaterial
        case .thick: return .thickMaterial
        case .ultraThick: return .ultraThickMaterial
        }
    }
}

private struct GlassBackground<S: InsettableShape>: ViewModifier {
    var grade: GlassLevel
    var shape: S
    var edge: Bool
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    func body(content: Content) -> some View {
        content
            .background {
                if !effects.solidGlass, let material = grade.material(transparency: effects.transparency) {
                    shape.fill(material)
                        .overlay(shape.fill(grade.tintColor(palette, transparency: effects.transparency)))
                } else {
                    shape.fill(palette[Tokens.Glass.cloud])
                }
            }
            .overlay {
                if edge {
                    shape.strokeBorder(palette[effects.solidGlass ? Tokens.Line.lineSoft : Tokens.Glass.glassEdge],
                                       lineWidth: 1)
                }
            }
    }
}

private struct Elevation: ViewModifier {
    var shadow: Shadow
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        let layers = palette.layers(shadow)
        // CSS blur is twice the Gaussian radius SwiftUI takes; spread has no SwiftUI equivalent.
        return content
            .shadow(color: layers.first.map { palette[$0.color] } ?? .clear,
                    radius: (layers.first?.blur ?? 0) / 2, x: layers.first?.x ?? 0, y: layers.first?.y ?? 0)
            .shadow(color: layers.dropFirst().first.map { palette[$0.color] } ?? .clear,
                    radius: (layers.dropFirst().first?.blur ?? 0) / 2,
                    x: layers.dropFirst().first?.x ?? 0, y: layers.dropFirst().first?.y ?? 0)
    }
}

extension View {
    /// A glass surface of `grade` in `shape`, with Misty's one edge.
    func glass<S: InsettableShape>(_ grade: GlassLevel, in shape: S, edge: Bool = true) -> some View {
        modifier(GlassBackground(grade: grade, shape: shape, edge: edge))
    }

    /// One of the elevation shadows (`Tokens.Elevation.shadow1` to `shadow3`).
    func elevation(_ shadow: Shadow) -> some View {
        modifier(Elevation(shadow: shadow))
    }
}

/// The floating mini player and player bar: Veil with rounded corners (`player.mini-radius`,
/// `r-xl`) on macOS 14 and 15; Liquid Glass inside a `GlassEffectContainer` on macOS 26
/// (ADR-0007); solid Cloud when transparency is reduced.
struct FloatingGlass<Content: View>: View {
    var cornerRadius: CGFloat
    @ViewBuilder var content: Content
    @Environment(\.effects) private var effects
    @Environment(\.look) private var look

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: look.radius(cornerRadius), style: .continuous)
    }

    var body: some View {
        if effects.solidGlass {
            content.glass(.veil, in: shape)
        } else {
            liquidOrVeil
        }
    }

    #if compiler(>=6.2)
    @ViewBuilder private var liquidOrVeil: some View {
        if #available(macOS 26.0, *) {
            GlassEffectContainer {
                content.glassEffect(.regular, in: shape)
            }
        } else {
            content.glass(.veil, in: shape)
        }
    }
    #else
    private var liquidOrVeil: some View {
        content.glass(.veil, in: shape)
    }
    #endif
}

/// The one liquid accent (D2, Artwork style only): the scrubber thumb in Liquid Glass on macOS 26,
/// a plain Veil thumb elsewhere.
struct LiquidThumb: ViewModifier {
    var enabled: Bool
    @Environment(\.effects) private var effects

    @ViewBuilder func body(content: Content) -> some View {
        if enabled && !effects.solidGlass {
            liquid(content)
        } else {
            content
        }
    }

    #if compiler(>=6.2)
    @ViewBuilder private func liquid(_ content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.glassEffect(.regular.interactive(), in: Circle())
        } else {
            content
        }
    }
    #else
    private func liquid(_ content: Content) -> some View {
        content
    }
    #endif
}

/// Luminous: the content fades out just above the floating player, so the player's glass floats
/// over the field alone (docs/ui.md, "Luminous"). Use it as a mask over the whole content area
/// (it ignores the safe area); `playerZone` is the height the player and its margin take at the
/// bottom, 0 without a player.
struct PlayerFadeMask: View {
    var playerZone: CGFloat
    var fade: CGFloat = LuminousGeometry.fadeHeight

    var body: some View {
        VStack(spacing: 0) {
            Color.black
            LinearGradient(colors: [.black, .black.opacity(0)], startPoint: .top, endPoint: .bottom)
                .frame(height: playerZone > 0 ? fade : 0)
            Color.clear
                .frame(height: playerZone)
        }
        .ignoresSafeArea()
    }
}

/// Luminous's grouped surfaces (lists, cards, notices): the Mist tint without live blur, one edge
/// and `shadow-1`, so a screen stays within the glass budget; solid Cloud when transparency is
/// reduced.
private struct SurfaceBackground<S: InsettableShape>: ViewModifier {
    var shape: S
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    func body(content: Content) -> some View {
        content
            .background {
                shape
                    .fill(effects.solidGlass
                        ? palette[Tokens.Glass.cloud]
                        : GlassLevel.mist.tintColor(palette, transparency: effects.transparency))
                    .elevation(Tokens.Elevation.shadow1)
            }
            .overlay {
                shape.strokeBorder(palette[effects.solidGlass ? Tokens.Line.lineSoft : Tokens.Glass.glassEdge],
                                   lineWidth: 1)
            }
    }
}

extension View {
    /// A grouped surface in `shape` (Luminous): the Mist tint, no live blur, `shadow-1`.
    func surface<S: InsettableShape>(in shape: S) -> some View {
        modifier(SurfaceBackground(shape: shape))
    }
}

/// The corner radius of Luminous's grouped surfaces (the owner's mock-up: 24).
let surfaceRadius: CGFloat = Tokens.Radius.r2xl

extension View {
    /// The soft scroll edge under chrome on macOS 26 (ADR-0007); elsewhere the content fades out
    /// above the floating player instead (PlayerFadeMask).
    @ViewBuilder func softScrollEdges() -> some View {
        #if compiler(>=6.2)
        if #available(macOS 26.0, *) {
            self.scrollEdgeEffectStyle(.soft, for: .all)
        } else {
            self
        }
        #else
        self
        #endif
    }
}
