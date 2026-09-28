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
        wallpapers: [String] = [], screenshotFile: String? = nil, downloadedAt: Date = Date(), directory: URL = URL(filePath: "/")
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

public struct DownloadProgress: Sendable, Equatable {
    public var fileIndex: Int
    public var fileCount: Int
    public var fileName: String
    public var bytesReceived: Int64
    public var totalBytes: Int64?
}

public struct NothingToDownloadError: LocalizedError, Sendable {
    public let themeName: String
    public var errorDescription: String? {
        "\(themeName) has neither wallpapers nor a readable palette, so there's nothing to download."
    }
}

/// Downloaded themes under `~/Library/Application Support/OmarchyThemes/themes/<slug>`.
public struct ThemeStore: Sendable {
    static let manifestFile = "theme.json"
    static let wallpapersFolder = "wallpapers"

    private let paths: AppPaths
    private let downloader: any Downloader
    private let now: @Sendable () -> Date

    public init(paths: AppPaths, downloader: any Downloader, now: @escaping @Sendable () -> Date = { Date() }) {
        self.paths = paths
        self.downloader = downloader
        self.now = now
    }

    public func list() -> [InstalledTheme] {
        let dirs = (try? FileManager.default.contentsOfDirectory(at: paths.themesDir, includingPropertiesForKeys: nil)) ?? []
        return dirs
            .filter { !$0.lastPathComponent.hasPrefix(".") }
            .compactMap(Self.load)
            .sorted { $0.downloadedAt > $1.downloadedAt }
    }

    public func get(_ slug: String) -> InstalledTheme? {
        (try? paths.themeDir(slug)).flatMap(Self.load)
    }

    public func isInstalled(_ slug: String) -> Bool {
        get(slug) != nil
    }

    private static func load(_ dir: URL) -> InstalledTheme? {
        guard var theme = JSONFile.read(InstalledTheme.self, from: dir.appending(path: manifestFile)) else { return nil }
        theme.directory = dir
        return theme
    }

    /// Downloads every wallpaper (plus the screenshot) into a staging folder and swaps it in
    /// atomically, so a cancelled or failed download never leaves a half-installed theme.
    @discardableResult
    public func install(_ details: ThemeDetails, progress: (@Sendable (DownloadProgress) -> Void)? = nil) async throws -> InstalledTheme {
        guard details.canApply else { throw NothingToDownloadError(themeName: details.entry.name) }

        let fm = FileManager.default
        let slug = details.entry.slug
        let finalDir = try paths.themeDir(slug)
        let staging = paths.themesDir.appending(path: ".staging-\(slug)-\(UUID().uuidString)", directoryHint: .isDirectory)
        let stagingWallpapers = staging.appending(path: Self.wallpapersFolder, directoryHint: .isDirectory)
        try fm.createDirectory(at: stagingWallpapers, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: staging) }

        var wallpaperNames: [String] = []
        var usedNames: Set<String> = []
        let fileCount = details.wallpapers.count + (details.entry.screenshotURL == nil ? 0 : 1)

        for (index, wallpaper) in details.wallpapers.enumerated() {
            let name = Self.uniqueFileName(wallpaper.fileName, used: &usedNames)
            progress?(DownloadProgress(fileIndex: index, fileCount: fileCount, fileName: wallpaper.fileName, bytesReceived: 0, totalBytes: wallpaper.size))
            try Task.checkCancellation()
            try await downloader.download(from: wallpaper.downloadURL, to: stagingWallpapers.appending(path: name)) { bytes in
                progress?(DownloadProgress(fileIndex: index, fileCount: fileCount, fileName: wallpaper.fileName, bytesReceived: bytes, totalBytes: wallpaper.size))
            }
            wallpaperNames.append(name)
        }

        // The screenshot is a nice-to-have for offline browsing; don't fail the install over it.
        var screenshotFile: String?
        if let screenshotURL = details.entry.screenshotURL {
            let ext = screenshotURL.pathExtension.lowercased()
            let candidate = "screenshot." + (ext.isEmpty ? "webp" : ext)
            progress?(DownloadProgress(fileIndex: fileCount - 1, fileCount: fileCount, fileName: candidate, bytesReceived: 0, totalBytes: nil))
            do {
                try await downloader.download(from: screenshotURL, to: staging.appending(path: candidate), progress: nil)
                screenshotFile = candidate
            } catch {
                if Task.isCancelled { throw CancellationError() }
            }
        }
        try Task.checkCancellation()

        var theme = InstalledTheme(
            slug: slug,
            name: details.entry.name,
            repoURL: details.entry.repoURL,
            palette: details.palette,
            mode: details.mode,
            wallpapers: wallpaperNames,
            screenshotFile: screenshotFile,
            downloadedAt: now())
        try JSONFile.write(theme, to: staging.appending(path: Self.manifestFile))

        if fm.fileExists(atPath: finalDir.path) {
            try fm.removeItem(at: finalDir)
        }
        try fm.moveItem(at: staging, to: finalDir)

        theme.directory = finalDir
        return theme
    }

    public func remove(_ slug: String) throws {
        let dir = try paths.themeDir(slug)
        guard FileManager.default.fileExists(atPath: dir.path) else { return }
        try FileManager.default.removeItem(at: dir)
    }

    /// Deletes leftovers from interrupted downloads (e.g. the app was quit mid-download).
    public func cleanUpStaging() {
        let dirs = (try? FileManager.default.contentsOfDirectory(at: paths.themesDir, includingPropertiesForKeys: nil)) ?? []
        for dir in dirs where dir.lastPathComponent.hasPrefix(".staging-") {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    private static func uniqueFileName(_ fileName: String, used: inout Set<String>) -> String {
        var safe = String(fileName.map { $0 == "/" || $0 == ":" || $0 == "\0" ? "_" : $0 })
        if safe.trimmingCharacters(in: .whitespaces).isEmpty || safe.hasPrefix(".") {
            safe = "wallpaper" + safe
        }
        let stem = (safe as NSString).deletingPathExtension
        let ext = (safe as NSString).pathExtension
        var candidate = safe
        var n = 2
        // APFS is case-insensitive by default.
        while !used.insert(candidate.lowercased()).inserted {
            candidate = ext.isEmpty ? "\(stem)-\(n)" : "\(stem)-\(n).\(ext)"
            n += 1
        }
        return candidate
    }
}
