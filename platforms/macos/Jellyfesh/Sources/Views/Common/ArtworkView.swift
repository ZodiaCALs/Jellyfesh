// Artwork in rows, cards and Now Playing: drawn from memory at once when it is cached, otherwise
// loaded off the main actor by ArtworkLoader while a quiet placeholder shows.

import SwiftUI

private struct ArtworkLoaderKey: EnvironmentKey {
    static let defaultValue: ArtworkLoader? = nil
}

extension EnvironmentValues {
    var artworkLoader: ArtworkLoader? {
        get { self[ArtworkLoaderKey.self] }
        set { self[ArtworkLoaderKey.self] = newValue }
    }
}

struct ArtworkView: View {
    var source: ArtworkSource?
    /// The side of the square in points; nil fills the offered width (grids), loading artwork
    /// for `flexibleSize` points.
    var size: CGFloat?
    var cornerRadius: CGFloat = Tokens.Artwork.radiusList
    /// A glyph for the placeholder.
    var placeholder: String = "music.note"

    @Environment(\.artworkLoader) private var loader
    @Environment(\.displayScale) private var displayScale
    @Environment(\.palette) private var palette
    @State private var loaded: NSImage?
    @State private var loadedFor: ArtworkSource?

    /// Points the artwork is loaded for when it fills its cell.
    static let flexibleSize: CGFloat = 220

    private var pixels: Int { Int(((size ?? Self.flexibleSize) * max(displayScale, 1)).rounded(.up)) }

    var body: some View {
        let image = currentImage
        // The rectangle takes the size; the image fills it and is clipped to the corners.
        Rectangle()
            .fill(palette[Tokens.Ink.ink06])
            .overlay {
                if let image {
                    Image(nsImage: image)
                        .resizable()
                        .interpolation(.high)
                        .scaledToFill()
                } else {
                    Image(systemName: placeholder)
                        .font(.system(size: max(10, (size ?? Self.flexibleSize) * 0.28), weight: .regular))
                        .foregroundStyle(palette[Tokens.Ink.ink30])
                }
            }
            .frame(width: size, height: size)
            .aspectRatio(1, contentMode: .fit)
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .accessibilityHidden(true)
            .task(id: TaskKey(source: source, pixels: pixels)) {
                await load()
            }
    }

    private var currentImage: NSImage? {
        if let loaded, loadedFor == source { return loaded }
        guard let source, let loader else { return nil }
        return loader.cached(source, pixels: pixels)
    }

    private struct TaskKey: Equatable {
        var source: ArtworkSource?
        var pixels: Int
    }

    private func load() async {
        guard let source, let loader else {
            loaded = nil
            loadedFor = nil
            return
        }
        if loadedFor == source, loaded != nil { return }
        let image = await loader.image(source, pixels: pixels)
        guard !Task.isCancelled else { return }
        loaded = image
        loadedFor = source
    }
}
