import Foundation
import OmarchyThemesKit

/// `DesktopBackend` for macOS. Only the wallpaper can be changed: macOS has no public API for
/// an app to set system dark mode or the accent color (only AppleScript/System Events or private
/// defaults, which this app doesn't use), so those are reported as unsupported.
public final class MacDesktopBackend: DesktopBackend {
    public let capabilities: DesktopCapabilities = [.wallpaper]
    public let supportedFits: [WallpaperFit] = [.fill, .fit, .stretch, .center]

    private let wallpapers: any WallpaperAPI
    private let converter: WallpaperImageConverter
    private let snapshotAssetsDir: URL
    private let now: @Sendable () -> Date

    static let screensKey = "screens"
    static let urlPrefix = "screen.url."
    static let optionsPrefix = "screen.options."
    static let copyPrefix = "screen.copy."

    /// Folders the OS owns; their pictures are always there, so they aren't copied.
    private static let systemPictureFolders = ["/System/", "/Library/Desktop Pictures/"]

    public init(
        wallpapers: any WallpaperAPI = NSWorkspaceWallpaperAPI(),
        converter: WallpaperImageConverter = WallpaperImageConverter(),
        snapshotAssetsDir: URL,
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.wallpapers = wallpapers
        self.converter = converter
        self.snapshotAssetsDir = snapshotAssetsDir
        self.now = now
    }

    public func capture() async throws -> DesktopSnapshot {
        let screens = await wallpapers.currentWallpapers()
        guard !screens.isEmpty else { throw NoScreensError() }

        var values: [String: String] = [Self.screensKey: screens.map(\.screenID).joined(separator: ",")]
        let encoder = JSONFile.makeEncoder()

        // Keep a private copy of each original wallpaper: the user may delete or move the file
        // later, and Restore should still work. The copies go to a staging folder that replaces the
        // previous ones only once they're all made, so a failed capture doesn't lose them.
        let fm = FileManager.default
        let staging = snapshotAssetsDir.deletingLastPathComponent()
            .appending(path: snapshotAssetsDir.lastPathComponent + ".new", directoryHint: .isDirectory)
        try fm.removeItemIfPresent(at: staging)
        do {
            var copies: [URL: URL] = [:]
            for (index, screen) in screens.enumerated() {
                values[Self.urlPrefix + screen.screenID] = screen.imageURL?.path ?? ""
                values[Self.optionsPrefix + screen.screenID] = String(decoding: try encoder.encode(screen.options), as: UTF8.self)

                guard let url = screen.imageURL, Self.needsCopy(url) else { continue }
                let copy: URL
                if let existing = copies[url] {
                    copy = existing
                } else {
                    let fileName = "\(index)" + (url.pathExtension.isEmpty ? "" : ".\(url.pathExtension)")
                    try fm.createDirectory(at: staging, withIntermediateDirectories: true)
                    try fm.copyItem(at: url, to: staging.appending(path: fileName))
                    copy = snapshotAssetsDir.appending(path: fileName)
                    copies[url] = copy
                }
                values[Self.copyPrefix + screen.screenID] = copy.path
            }

            try fm.removeItemIfPresent(at: snapshotAssetsDir)
            if fm.fileExists(atPath: staging.path) {
                try fm.moveItem(at: staging, to: snapshotAssetsDir)
            }
        } catch {
            try? fm.removeItemIfPresent(at: staging)
            throw error
        }

        return DesktopSnapshot(takenAt: now(), values: values)
    }

    private static func needsCopy(_ url: URL) -> Bool {
        var isDirectory: ObjCBool = false
        guard url.isFileURL,
              FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory),
              !isDirectory.boolValue
        else { return false }
        return !systemPictureFolders.contains { url.path.hasPrefix($0) }
    }

    public func restore(_ snapshot: DesktopSnapshot) async throws {
        let recorded = snapshot.values[Self.screensKey]?.split(separator: ",").map(String.init) ?? []
        guard let fallbackID = recorded.first else { return }
        let decoder = JSONFile.makeDecoder()

        // Throwing keeps the snapshot (ThemeApplier only forgets it after a successful restore), so
        // with no display connected right now the original desktop isn't lost.
        let current = await wallpapers.currentWallpapers()
        guard !current.isEmpty else { throw NoScreensError() }

        var attempted = 0
        var failures: [any Error] = []
        for screen in current {
            // A display connected after the snapshot gets the main screen's original picture.
            let id = recorded.contains(screen.screenID) ? screen.screenID : fallbackID
            guard let path = snapshot.values[Self.urlPrefix + id], !path.isEmpty else { continue }

            var url = URL(filePath: path)
            if !FileManager.default.fileExists(atPath: path),
               let copy = snapshot.values[Self.copyPrefix + id], FileManager.default.fileExists(atPath: copy) {
                url = URL(filePath: copy)
            }
            let options = snapshot.values[Self.optionsPrefix + id]
                .flatMap { try? decoder.decode(WallpaperOptions.self, from: Data($0.utf8)) } ?? WallpaperOptions()

            attempted += 1
            do {
                try await wallpapers.setWallpaper(url, options: options, screenID: screen.screenID)
            } catch {
                failures.append(error)
            }
        }
        // Fail only if nothing could be put back (displays without a saved picture are skipped).
        if let first = failures.first, failures.count == attempted {
            throw first
        }
    }

    /// Sets the wallpaper on every display, carrying on past a display that fails so the others
    /// still change; the error then says how many did.
    public func setWallpaper(_ image: URL, fit: WallpaperFit, fillColor: RgbColor?) async throws {
        let usable = try converter.ensureSupportedFormat(image)
        try Task.checkCancellation()
        let options = WallpaperOptions(fit: fit, fillColor: fillColor)
        let screens = await wallpapers.currentWallpapers()
        guard !screens.isEmpty else { throw NoScreensError() }

        var failures: [any Error] = []
        for screen in screens {
            do {
                try await wallpapers.setWallpaper(usable, options: options, screenID: screen.screenID)
            } catch {
                failures.append(error)
            }
        }
        guard let first = failures.first else { return }
        if failures.count == screens.count { throw first }
        throw PartialWallpaperError(changed: screens.count - failures.count, of: screens.count, underlying: first)
    }

    public func setAppearanceMode(_ mode: AppearanceMode) async throws {
        throw UnsupportedOnMacError()
    }

    public func setAccentColor(_ accent: RgbColor) async throws {
        throw UnsupportedOnMacError()
    }
}

public struct NoScreensError: LocalizedError, Sendable {
    public var errorDescription: String? { "No displays were found." }
}

/// Some displays got the new wallpaper and some didn't.
public struct PartialWallpaperError: LocalizedError, Sendable {
    public let changed: Int
    public let total: Int
    public let underlying: any Error

    public init(changed: Int, of total: Int, underlying: any Error) {
        self.changed = changed
        self.total = total
        self.underlying = underlying
    }

    public var errorDescription: String? {
        "It was set on \(changed) of \(total) displays. \(underlying.localizedDescription)"
    }
}

public struct UnsupportedOnMacError: LocalizedError, Sendable {
    public var errorDescription: String? { "macOS doesn't let apps change this setting." }
}
