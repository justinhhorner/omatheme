import Foundation

public struct DownloadProgress: Sendable, Equatable {
    /// The file being downloaded: the wallpapers in order, then the screenshot.
    public var fileIndex: Int
    public var bytesReceived: Int64
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

    private static func load(_ dir: URL) -> InstalledTheme? {
        guard var theme = JSONFile.read(InstalledTheme.self, from: dir.appending(path: manifestFile)) else { return nil }
        theme.directory = dir
        return theme
    }

    /// Downloads every wallpaper (plus the screenshot) into a staging folder and swaps it in
    /// atomically, so a cancelled or failed download never leaves a half-installed theme.
    @discardableResult
    public func install(
        _ details: ThemeDetails,
        progress: (@Sendable (DownloadProgress) -> Void)? = nil
    ) async throws -> InstalledTheme {
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

        @Sendable func report(_ index: Int, bytes: Int64) {
            progress?(DownloadProgress(fileIndex: index, bytesReceived: bytes))
        }

        for (index, wallpaper) in details.wallpapers.enumerated() {
            let name = Self.uniqueFileName(wallpaper.fileName, used: &usedNames)
            report(index, bytes: 0)
            try Task.checkCancellation()
            try await downloader.download(from: wallpaper.downloadURL, to: stagingWallpapers.appending(path: name)) { bytes in
                report(index, bytes: bytes)
            }
            wallpaperNames.append(name)
        }

        // The screenshot is a nice-to-have for offline browsing; don't fail the install over it.
        var screenshotFile: String?
        if let screenshotURL = details.entry.screenshotURL {
            let ext = screenshotURL.pathExtension.lowercased()
            let candidate = "screenshot." + (ext.isEmpty ? "webp" : ext)
            report(details.wallpapers.count, bytes: 0)
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
            // One-step swap: if it fails, the previous download is still there.
            _ = try fm.replaceItemAt(finalDir, withItemAt: staging)
        } else {
            try fm.moveItem(at: staging, to: finalDir)
        }

        theme.directory = finalDir
        return theme
    }

    public func remove(_ slug: String) throws {
        try FileManager.default.removeItemIfPresent(at: paths.themeDir(slug))
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
