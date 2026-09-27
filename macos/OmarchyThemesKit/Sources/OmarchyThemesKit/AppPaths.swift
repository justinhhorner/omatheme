import Foundation

/// On-disk layout for everything the app stores locally. The app isn't sandboxed, so this is the
/// real `~/Library/Application Support/OmarchyThemes`.
public struct AppPaths: Sendable, Equatable {
    public let root: URL

    public init(root: URL) {
        self.root = root.standardizedFileURL
    }

    public static func `default`() -> AppPaths {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return AppPaths(root: support.appending(path: "OmarchyThemes", directoryHint: .isDirectory))
    }

    /// HTTP/API response cache (catalog, GitHub trees, ETags).
    public var cacheDir: URL { root.appending(path: "cache", directoryHint: .isDirectory) }

    /// Downloaded themes, one folder per theme slug.
    public var themesDir: URL { root.appending(path: "themes", directoryHint: .isDirectory) }

    public var settingsFile: URL { root.appending(path: "settings.json") }

    /// Snapshot of the user's desktop taken before the first Apply, for "Restore".
    public var snapshotFile: URL { root.appending(path: "original-desktop.json") }

    /// Private copies of the original wallpapers, used by Restore if the originals are gone.
    public var originalDesktopDir: URL { root.appending(path: "original-desktop", directoryHint: .isDirectory) }

    /// Folder for one downloaded theme. Rejects slugs that could escape `themesDir`.
    public func themeDir(_ slug: String) throws -> URL {
        guard Self.isValidSlug(slug) else { throw InvalidSlugError(slug: slug) }
        return themesDir.appending(path: slug, directoryHint: .isDirectory)
    }

    public func ensureCreated() throws {
        try FileManager.default.createDirectory(at: cacheDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: themesDir, withIntermediateDirectories: true)
    }

    static func isValidSlug(_ slug: String?) -> Bool {
        guard let slug, !slug.isEmpty, slug.count <= 100, slug != ".", slug != ".." else { return false }
        return slug.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" || $0 == ".") }
    }
}

public struct InvalidSlugError: LocalizedError, Sendable {
    public let slug: String
    public var errorDescription: String? { "Invalid theme slug '\(slug)'." }
}
