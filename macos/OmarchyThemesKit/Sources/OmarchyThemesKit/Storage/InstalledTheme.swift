import Foundation

/// A theme saved locally. Everything needed to apply it is on disk (no network).
public struct InstalledTheme: Codable, Sendable, Hashable, Identifiable {
    public var slug: String
    public var name: String
    public var repoURL: URL
    public var palette: Palette?
    public var mode: AppearanceMode

    /// Wallpaper file names inside the theme's wallpapers folder, in display order.
    public var wallpapers: [String]

    public var screenshotFile: String?
    public var downloadedAt: Date

    /// Absolute folder of this theme; set when loaded from disk.
    public var directory: URL = URL(filePath: "/")

    public init(
        slug: String, name: String, repoURL: URL, palette: Palette? = nil, mode: AppearanceMode = .dark,
        wallpapers: [String] = [], screenshotFile: String? = nil, downloadedAt: Date = Date(),
        directory: URL = URL(filePath: "/")
    ) {
        self.slug = slug
        self.name = name
        self.repoURL = repoURL
        self.palette = palette
        self.mode = mode
        self.wallpapers = wallpapers
        self.screenshotFile = screenshotFile
        self.downloadedAt = downloadedAt
        self.directory = directory
    }

    public var id: String { slug }

    public func wallpaperURL(_ fileName: String) -> URL {
        directory.appending(path: ThemeStore.wallpapersFolder, directoryHint: .isDirectory).appending(path: fileName)
    }

    public var screenshotURL: URL? { screenshotFile.map { directory.appending(path: $0) } }

    // Keys follow docs/data-format.md (shared with Windows): "repoUrl".
    private enum CodingKeys: String, CodingKey {
        case slug, name, palette, mode, wallpapers, screenshotFile, downloadedAt
        case repoURL = "repoUrl"
    }

    /// Keys written by older versions of this app.
    private enum LegacyKeys: String, CodingKey {
        case repoURL
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        slug = try c.decode(String.self, forKey: .slug)
        name = try c.decode(String.self, forKey: .name)
        if let url = try c.decodeIfPresent(URL.self, forKey: .repoURL) {
            repoURL = url
        } else {
            repoURL = try decoder.container(keyedBy: LegacyKeys.self).decode(URL.self, forKey: .repoURL)
        }
        palette = try c.decodeIfPresent(Palette.self, forKey: .palette)
        mode = try c.decodeIfPresent(AppearanceMode.self, forKey: .mode) ?? palette?.mode ?? .dark
        wallpapers = try c.decodeIfPresent([String].self, forKey: .wallpapers) ?? []
        screenshotFile = try c.decodeIfPresent(String.self, forKey: .screenshotFile)
        downloadedAt = try c.decode(Date.self, forKey: .downloadedAt)
    }
}
