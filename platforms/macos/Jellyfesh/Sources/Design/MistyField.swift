// Misty's lit field: the sky, three aurora glows and a fine grain (MAT-02), at the theme hue. It
// sits behind every view; only Now Playing in the Artwork style replaces it (deviation D1).

import SwiftUI

struct MistyField: View {
    @Environment(\.palette) private var palette

    var body: some View {
        ZStack {
            palette[Tokens.Field.sky]
            glow(Tokens.Field.aurora1, at: UnitPoint(x: 0.10, y: 0.08), size: 0.70)
            glow(Tokens.Field.aurora2, at: UnitPoint(x: 0.92, y: 0.92), size: 0.65)
            glow(Tokens.Field.aurora3, at: UnitPoint(x: 0.04, y: 0.98), size: 0.55)
            Grain(opacity: palette.alpha(Tokens.Field.grain))
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }

    private func glow(_ key: ThemeColorKey, at center: UnitPoint, size: CGFloat) -> some View {
        EllipticalGradient(
            colors: [palette[key].opacity(0.85), palette[key].opacity(0)],
            center: center,
            startRadiusFraction: 0,
            endRadiusFraction: size
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
