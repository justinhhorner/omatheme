import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Makes sure a wallpaper is in a format the desktop displays. ImageIO decodes WebP and BMP, but
/// whether the wallpaper agent accepts them isn't documented, so those are converted to PNG
/// once, next to the original (in a hidden `.converted` folder), and reused afterwards.
public struct WallpaperImageConverter: Sendable {
    /// Formats the desktop is documented to display directly.
    static let directExtensions: Set<String> = ["png", "jpg", "jpeg", "heic", "heif", "tif", "tiff", "gif"]

    public init() {}

    public func ensureSupportedFormat(_ image: URL) throws -> URL {
        if Self.directExtensions.contains(image.pathExtension.lowercased()) {
            return image
        }

        let fm = FileManager.default
        let folder = image.deletingLastPathComponent().appending(path: ".converted", directoryHint: .isDirectory)
        let output = folder.appending(path: image.deletingPathExtension().lastPathComponent + ".png")
        if let converted = modificationDate(output), let original = modificationDate(image), converted >= original {
            return output
        }

        guard let source = CGImageSourceCreateWithURL(image as CFURL, nil),
              let cgImage = CGImageSourceCreateImageAtIndex(source, 0, nil)
        else { throw ImageConversionError(fileName: image.lastPathComponent) }

        try fm.createDirectory(at: folder, withIntermediateDirectories: true)
        let temporary = folder.appending(path: ".\(UUID().uuidString).png")
        defer { try? fm.removeItem(at: temporary) }
        guard let destination = CGImageDestinationCreateWithURL(temporary as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            throw ImageConversionError(fileName: image.lastPathComponent)
        }
        CGImageDestinationAddImage(destination, cgImage, nil)
        guard CGImageDestinationFinalize(destination) else { throw ImageConversionError(fileName: image.lastPathComponent) }

        if fm.fileExists(atPath: output.path) { try fm.removeItem(at: output) }
        try fm.moveItem(at: temporary, to: output)
        return output
    }

    private func modificationDate(_ url: URL) -> Date? {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
    }
}

public struct ImageConversionError: LocalizedError, Sendable {
    public let fileName: String
    public var errorDescription: String? {
        "\(fileName) couldn't be read as an image. Try downloading the theme again."
    }
}
