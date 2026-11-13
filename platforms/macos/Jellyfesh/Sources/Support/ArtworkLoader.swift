// Artwork off the main actor (docs/architecture.md: images cross the boundary as file paths).
// Library artwork comes as the core's cached thumbnail for a content hash (64 to 1024 px); the
// file is decoded and downsampled on a background task and kept in memory under a byte budget.
// Source artwork URLs are not fetched here: networking stays in the core, so those rows show the
// placeholder until the core hands out a path.

import AppKit
import ImageIO

/// Artwork a view can load: a library content hash, a file, or a source's URL.
enum ArtworkSource: Hashable, Sendable {
    case hash(String)
    case file(String)
    case url(String)

    init?(hash: String?, location: ArtworkLocation? = nil) {
        if let hash {
            self = .hash(hash)
            return
        }
        switch location {
        case .file(let path)?: self = .file(path)
        case .url(let url)?: self = .url(url)
        case nil: return nil
        }
    }
}

@MainActor
final class ArtworkLoader {
    /// Thumbnail sizes the core makes (`artwork_thumbnail`).
    nonisolated static let thumbnailSizes = [64, 128, 256, 512, 1024]

    private let client: CoreClient
    private let cache = NSCache<NSString, NSImage>()
    private var inFlight: [String: Task<NSImage?, Never>] = [:]

    init(client: CoreClient) {
        self.client = client
        cache.totalCostLimit = 64 * 1024 * 1024
    }

    /// The smallest thumbnail size that covers `pixels` device pixels.
    nonisolated static func thumbnailSize(for pixels: Int) -> Int {
        thumbnailSizes.first { $0 >= pixels } ?? 1024
    }

    /// The decoded image if it is already in memory: rows scrolled back into view draw at once.
    func cached(_ source: ArtworkSource, pixels: Int) -> NSImage? {
        cache.object(forKey: Self.key(source, Self.thumbnailSize(for: pixels)) as NSString)
    }

    /// The image for `source` at about `pixels` device pixels, or nil when there is none.
    func image(_ source: ArtworkSource, pixels: Int) async -> NSImage? {
        let size = Self.thumbnailSize(for: pixels)
        let key = Self.key(source, size)
        if let hit = cache.object(forKey: key as NSString) { return hit }
        if let running = inFlight[key] { return await running.value }
        let client = self.client
        let task = Task<NSImage?, Never> {
            let path: String?
            switch source {
            case .hash(let hash):
                path = try? await client.artworkThumbnail(hash: hash, sizePx: size)
            case .file(let file):
                path = file
            case .url:
                path = nil
            }
            guard let path else { return nil }
            return await Self.decode(path: path, maxPixels: size)
        }
        inFlight[key] = task
        let image = await task.value
        inFlight[key] = nil
        if let image {
            cache.setObject(image, forKey: key as NSString, cost: size * size * 4)
        }
        return image
    }

    private nonisolated static func key(_ source: ArtworkSource, _ size: Int) -> String {
        "\(source)@\(size)"
    }

    /// Decodes and downsamples an image file on a background task.
    nonisolated static func decode(path: String, maxPixels: Int) async -> NSImage? {
        await Task.detached(priority: .utility) { () -> NSImage? in
            let url = URL(fileURLWithPath: path)
            guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
            let options: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: maxPixels,
            ]
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
                return nil
            }
            return NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
        }.value
    }
}
