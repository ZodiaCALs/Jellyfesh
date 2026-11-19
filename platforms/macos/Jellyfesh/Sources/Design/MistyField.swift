// The lit field behind every view: the sky, the aurora and a fine grain (Misty MAT-02), at the
// theme hue. Luminous (v0.0.6, docs/ui.md) turns the aurora up to `field.aurora-strength` with four
// larger glows (LuminousField), on every screen, light and dark; the colours crossfade with the
// theme on every song change. A customized look sets the aurora's strength (ADR-0020). Only Now
// Playing in the Artwork style draws the baked backdrop over it.

import SwiftUI

struct MistyField: View {
    @Environment(\.palette) private var palette
    @Environment(\.look) private var look

    var body: some View {
        ZStack {
            palette[Tokens.Field.sky]
            ForEach(LuminousField.glows.indices, id: \.self) { index in
                glow(LuminousField.glows[index])
            }
            Grain(opacity: palette.alpha(Tokens.Field.grain))
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }

    private func glow(_ glow: LuminousField.Glow) -> some View {
        let color = palette[glow.color]
        return EllipticalGradient(
            stops: [
                .init(color: color.opacity(glow.alpha(strength: look.aurora)), location: 0),
                .init(color: color.opacity(0), location: glow.fadeEnd),
            ],
            center: UnitPoint(x: glow.x, y: glow.y),
            startRadiusFraction: 0,
            endRadiusFraction: glow.radius
        )
    }
}

/// A static noise texture, tiled at the field's grain opacity. Drawn once; it never animates.
private struct Grain: View {
    var opacity: Double

    var body: some View {
        if let image = GrainTexture.image {
            Image(decorative: image, scale: 2)
                .resizable(resizingMode: .tile)
                .opacity(opacity)
        }
    }
}

private enum GrainTexture {
    static let image: CGImage? = {
        let side = 128
        var generator = SystemRandomNumberGenerator()
        var bytes = [UInt8](repeating: 0, count: side * side)
        for index in bytes.indices {
            bytes[index] = UInt8.random(in: 0...255, using: &generator)
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData) else { return nil }
        return CGImage(
            width: side,
            height: side,
            bitsPerComponent: 8,
            bitsPerPixel: 8,
            bytesPerRow: side,
            space: CGColorSpaceCreateDeviceGray(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.none.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )
    }()
}
