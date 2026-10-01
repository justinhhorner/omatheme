import AppKit
import ImageIO
import os
import OmarchyThemesKit
import SwiftUI

/// Loads screenshots and wallpapers as downsampled thumbnails. Remote images go through a disk
/// URLCache (the gallery shows 140+ screenshots); decoded thumbnails are kept in memory.
///
/// The disk cache lives in a "generation" folder inside `cacheDirectory`. A URLCache can't be
/// emptied reliably (`removeAllCachedResponses` leaves its disk store behind) and its files mustn't
/// be deleted while it's in use, so `clear()` starts a new generation and deletes the old one.
final class ImageLoader: Sendable {
    private let directory: URL
    private let generation: OSAllocatedUnfairLock<Generation>
    private let memory = ThumbnailCache()

    private struct Generation {
        let folder: URL
        let session: URLSession
    }

    init(cacheDirectory: URL) {
        directory = cacheDirectory
        // The newest generation's thumbnails carry over between launches; anything else here
        // (an older generation, or the single cache earlier versions kept) is left over.
        let keep = Self.newestGeneration(in: cacheDirectory)
        Self.deleteContents(of: cacheDirectory, except: keep)
        generation = OSAllocatedUnfairLock(initialState: Self.makeGeneration(keep ?? Self.newFolder(in: cacheDirectory)))
    }

    private var session: URLSession { generation.withLock { $0.session } }

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

    /// Forgets every thumbnail, in memory and on disk: later loads use a new, empty generation, and
    /// the old one's folder is deleted once nothing new can be stored in it.
    func clear() {
        memory.removeAllObjects()
        let fresh = Self.makeGeneration(Self.newFolder(in: directory))
        let old = generation.withLock { current in
            defer { current = fresh }
            return current
        }
        old.session.finishTasksAndInvalidate()
        try? FileManager.default.removeItem(at: old.folder)
    }

    private static func makeGeneration(_ folder: URL) -> Generation {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = URLCache(memoryCapacity: 16 << 20, diskCapacity: 256 << 20, directory: folder)
        configuration.requestCachePolicy = .returnCacheDataElseLoad
        configuration.httpAdditionalHeaders = ["User-Agent": AppInfo.userAgent]
        return Generation(folder: folder, session: URLSession(configuration: configuration))
    }

    private static func newFolder(in directory: URL) -> URL {
        directory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
    }

    private static func newestGeneration(in directory: URL) -> URL? {
        let keys: Set<URLResourceKey> = [.isDirectoryKey, .creationDateKey]
        let items = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: Array(keys))) ?? []
        return items
            .compactMap { url -> (URL, Date)? in
                // Generations are UUID-named; an older version's URLCache has its own subfolders here.
                guard UUID(uuidString: url.lastPathComponent) != nil,
                      let values = try? url.resourceValues(forKeys: keys), values.isDirectory == true
                else { return nil }
                return (url, values.creationDate ?? .distantPast)
            }
            .max { $0.1 < $1.1 }?.0
    }

    private static func deleteContents(of directory: URL, except keep: URL?) {
        let items = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        for item in items where item.standardizedFileURL.path != keep?.standardizedFileURL.path {
            try? FileManager.default.removeItem(at: item)
        }
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

extension EnvironmentValues {
    /// Set by `appEnvironment(_:)`; without one, thumbnails show their placeholder.
    @Entry var imageLoader: ImageLoader?
}

/// An image from disk or the web, shown as a thumbnail with a placeholder while it loads.
struct ThumbnailImage: View {
    @Environment(\.imageLoader) private var loader
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
            .task(id: url) { await load() }
    }

    private func load() async {
        image = nil
        failed = false
        guard let url, let loader else { return }
        let loaded = await loader.thumbnail(for: url, maxPixelSize: maxPixelSize)
        withAnimation(.easeOut(duration: 0.15)) {
            image = loaded
            failed = loaded == nil
        }
    }
}
