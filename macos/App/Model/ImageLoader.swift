import AppKit
import ImageIO
import SwiftUI

/// Loads screenshots and wallpapers as downsampled thumbnails. Remote images go through a disk
/// URLCache (the gallery shows 140+ screenshots); decoded thumbnails are kept in memory.
final class ImageLoader: Sendable {
    private let session: URLSession
    private let memory = ThumbnailCache()

    init(cacheDirectory: URL) {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = URLCache(memoryCapacity: 16 << 20, diskCapacity: 256 << 20, directory: cacheDirectory)
        configuration.requestCachePolicy = .returnCacheDataElseLoad
        configuration.httpAdditionalHeaders = ["User-Agent": "OmarchyThemes/0.1"]
        session = URLSession(configuration: configuration)
    }

    /// A thumbnail no larger than `maxPixelSize` on its long edge, or nil if it can't be loaded.
    func thumbnail(for url: URL, maxPixelSize: Int) async -> CGImage? {
        let key = "\(maxPixelSize)|\(url.absoluteString)" as NSString
        if let cached = memory.object(forKey: key) { return cached.image }

        let source: CGImageSource?
        if url.isFileURL {
            source = CGImageSourceCreateWithURL(url as CFURL, nil)
        } else {
            guard let (data, response) = try? await session.data(from: url),
                  (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? false
            else { return nil }
            source = CGImageSourceCreateWithData(data as CFData, nil)
        }

        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        ]
        guard let source, let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        memory.setObject(Thumbnail(image), forKey: key)
        return image
    }

    func clearMemory() {
        memory.removeAllObjects()
    }

    private final class Thumbnail {
        let image: CGImage
        init(_ image: CGImage) { self.image = image }
    }

    /// NSCache is thread-safe.
    private final class ThumbnailCache: NSCache<NSString, Thumbnail>, @unchecked Sendable {
        override init() {
            super.init()
            countLimit = 400
        }
    }
}

/// An image from disk or the web, shown as a thumbnail with a placeholder while it loads.
struct ThumbnailImage: View {
    @Environment(AppModel.self) private var model
    let url: URL?
    var maxPixelSize = 800
    var contentMode: ContentMode = .fill
    /// The grey placeholder box behind the image; off for large previews shown on a backdrop.
    var showsBackground = true

    @State private var image: CGImage?
    @State private var failed = false

    var body: some View {
        // The image is an overlay so it never sizes the view: callers set the frame or aspect
        // ratio, and a .fill image is cropped to it.
        Rectangle()
            .fill(showsBackground ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear))
            .overlay {
                if let image {
                    Image(decorative: image, scale: 1)
                        .resizable()
                        .aspectRatio(contentMode: contentMode)
                        .transition(.opacity)
                } else if failed || url == nil {
                    Image(systemName: "photo")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                }
            }
            .clipped()
        .task(id: url) {
            image = nil
            failed = false
            guard let url else { return }
            let loaded = await model.images.thumbnail(for: url, maxPixelSize: maxPixelSize)
            withAnimation(.easeOut(duration: 0.15)) {
                image = loaded
                failed = loaded == nil
            }
        }
    }
}
