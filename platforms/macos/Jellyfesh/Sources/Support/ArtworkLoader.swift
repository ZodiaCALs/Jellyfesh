// Artwork off the main actor (docs/architecture.md: images cross the boundary as file paths).
// Library artwork comes as the core's cached thumbnail for a content hash (64 to 1024 px); a
// source's picture (a URL) as the file the core fetched through the plugin host into its artwork
// cache (docs/core-api.md, "Covers"): `source_artwork` gives the path at once when it is cached,
// otherwise an `.artwork` event brings it, and until then the placeholder shows. Files are decoded
// and downsampled on a background task and kept in memory under a byte budget.
//
// A picture the core could not get is asked for again after a back-off (30 seconds, doubling up to
// 10 minutes), at once when the core later sends it after all, and when a source signs in or out
// (`forgetFailures`); views showing a placeholder then try again (`generation`).

import AppKit
import ImageIO
import Observation

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
@Observable
final class ArtworkLoader {
    /// Thumbnail sizes the core makes (`artwork_thumbnail`).
    nonisolated static let thumbnailSizes = [64, 128, 256, 512, 1024]

    /// How long a picture the core is fetching is waited for before the placeholder stays.
    nonisolated static let sourceArtworkWait: Duration = .seconds(60)

    /// Bumped when pictures that failed may be had now (`forgetFailures`): views that show a
    /// placeholder load again.
    private(set) var generation = 0

    private let client: CoreClient
    private let now: () -> ContinuousClock.Instant
    private let cache = NSCache<NSString, NSImage>()
    @ObservationIgnored private var inFlight: [String: Task<NSImage?, Never>] = [:]
    /// Waiting for `.artwork` events, by URL and size.
    @ObservationIgnored private var waiters: [String: [UUID: CheckedContinuation<String?, Never>]] = [:]
    /// Pictures the core said cannot be had, by URL and size: not asked for again until their
    /// back-off ends.
    @ObservationIgnored private var failures: [String: Failure] = [:]
    /// `.artwork` events that came while nobody waited yet (the answer can overtake the query's).
    @ObservationIgnored private var arrived: [String: String] = [:]

    /// A picture the core could not get: how often in a row, and when it may be asked for again.
    private struct Failure {
        var count: Int
        var retryAt: ContinuousClock.Instant
    }

    init(client: CoreClient, now: @escaping () -> ContinuousClock.Instant = { ContinuousClock.now }) {
        self.client = client
        self.now = now
        cache.totalCostLimit = 64 * 1024 * 1024
    }

    /// How long a picture that failed `count` times in a row waits before it is asked for again:
    /// 30 seconds, then twice as long after each failure, at most 10 minutes.
    nonisolated static func backOff(failures count: Int) -> Duration {
        let doublings = min(max(count, 1) - 1, 5)
        return .seconds(min(30 << doublings, 600))
    }

    /// Pictures that failed may be had now (a source signed in or out): ask for them again.
    func forgetFailures() {
        guard !failures.isEmpty else { return }
        failures.removeAll()
        generation += 1
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
            case .url(let url):
                path = await self.sourceArtworkPath(url: url, size: size)
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

    /// A source picture's cached file: at once when the core has it, else when its `.artwork`
    /// event arrives (nil when it cannot be had, or after `sourceArtworkWait`).
    private func sourceArtworkPath(url: String, size: Int) async -> String? {
        let key = "\(url)@\(size)"
        guard mayAsk(key) else { return nil }
        do {
            if let path = try await client.sourceArtwork(url: url, sizePx: size) { return path }
        } catch {
            return nil
        }
        if let path = arrived.removeValue(forKey: key) { return path }
        // The core may have answered while the query was on its way.
        guard mayAsk(key) else { return nil }
        let token = UUID()
        Task { [weak self] in
            try? await Task.sleep(for: Self.sourceArtworkWait)
            self?.resume(key: key, token: token, path: nil)
        }
        return await withCheckedContinuation { continuation in
            waiters[key, default: [:]][token] = continuation
        }
    }

    /// Whether a source picture may be asked for: never failed, or its back-off is over.
    private func mayAsk(_ key: String) -> Bool {
        guard let failure = failures[key] else { return true }
        return now() >= failure.retryAt
    }

    /// The core cached a source picture, or found it cannot be had.
    func artworkFetched(_ fetched: ArtworkFetched) {
        let key = "\(fetched.url)@\(fetched.sizePx)"
        if fetched.path == nil {
            let time = now()
            if failures.count > 4_000 {
                failures = failures.filter { $0.value.retryAt > time }
                if failures.count > 4_000 { failures.removeAll() }
            }
            let count = (failures[key]?.count ?? 0) + 1
            failures[key] = Failure(count: count, retryAt: time.advanced(by: Self.backOff(failures: count)))
        } else {
            // It can be had after all.
            failures[key] = nil
        }
        guard let pending = waiters.removeValue(forKey: key) else {
            if let path = fetched.path {
                if arrived.count > 256 { arrived.removeAll() }
                arrived[key] = path
            }
            return
        }
        for continuation in pending.values {
            continuation.resume(returning: fetched.path)
        }
    }

    private func resume(key: String, token: UUID, path: String?) {
        guard let continuation = waiters[key]?.removeValue(forKey: token) else { return }
        if waiters[key]?.isEmpty == true { waiters[key] = nil }
        continuation.resume(returning: path)
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
