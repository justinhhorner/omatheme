import Foundation
import ImageIO
import OmarchyThemesKit
import Testing
@testable import OmarchyThemesMac

/// Opt-in checks against the real omarchy.org, GitHub and this Mac, all read-only:
///
///     OMATHEME_LIVE=1 swift test --filter LiveChecks
///
/// They resolve a handful of themes (the unauthenticated GitHub API allows 60 calls an hour)
/// and never change the desktop.
@Suite(.enabled(if: ProcessInfo.processInfo.environment["OMATHEME_LIVE"] == "1"), .serialized)
struct LiveChecks {
    let temp = TempFolder()

    var cache: HTTPCache {
        HTTPCache(transport: URLSessionTransport(session: HTTPSessions.make()), directory: temp.url.appending(path: "cache"))
    }

    @Test func catalogAndAFewThemesResolve() async throws {
        let catalog = try await CatalogService(cache: cache).refresh()
        print("Catalog: \(catalog.entries.count) themes")
        #expect(catalog.entries.count > 100)
        #expect(Set(catalog.entries.map(\.slug)).count == catalog.entries.count)

        let resolver = ThemeResolver(github: GitHubClient(cache: cache, token: ProcessInfo.processInfo.environment["GITHUB_TOKEN"]))
        let wanted = ["aetheria", "vulkanite", "tokyo-night", "catppuccin-latte"]
        for slug in wanted {
            guard let entry = catalog.entries.first(where: { $0.slug == slug }) else {
                print("\(slug): not in catalog")
                continue
            }
            let details = try await resolver.resolve(entry)
            let palette = details.palette.map { "\($0.source) bg \($0.background) accent \($0.accent) \($0.swatches.count) swatches" }
                ?? "no palette: \(details.paletteError ?? "?")"
            print("\(slug): \(Self.summary(details)), \(palette)")
            #expect(details.canApply)
        }
    }

    @Test func omarchysDefaultThemesAreListedAndResolve() async throws {
        let github = GitHubClient(cache: cache, token: ProcessInfo.processInfo.environment["GITHUB_TOKEN"])
        let catalog = try await CatalogService(cache: cache, github: github).refresh()
        let defaults = catalog.entries.filter(\.isDefaultTheme)
        print("Default themes (\(defaults.count)): \(defaults.map(\.name).joined(separator: ", "))")
        #expect(catalog.defaultThemesError == nil)
        #expect(defaults.count >= 10)
        #expect(defaults.allSatisfy { $0.screenshotURL != nil })

        for slug in ["omarchy.tokyo-night", "omarchy.catppuccin-latte"] {
            let entry = try #require(defaults.first { $0.slug == slug })
            let details = try await ThemeResolver(github: github).resolve(entry)
            print("\(entry.name): \(Self.summary(details)), accent \(details.palette?.accent.hex ?? "none")")
            #expect(details.palette != nil)
            #expect(!details.wallpapers.isEmpty)
        }
    }

    @Test func downloaderReportsProgressAndProducesADecodableImage() async throws {
        let catalog = try await CatalogService(cache: cache).refresh()
        let entry = try #require(catalog.entries.first { $0.slug == "vulkanite" } ?? catalog.entries.first)
        let details = try await ThemeResolver(github: GitHubClient(cache: cache)).resolve(entry)
        let wallpaper = try #require(details.wallpapers.first)

        let progress = ProgressLog()
        let destination = temp.url.appending(path: wallpaper.fileName)
        try await URLSessionDownloader(session: HTTPSessions.make())
            .download(from: wallpaper.downloadURL, to: destination, progress: progress.record)

        let size = try #require(try destination.resourceValues(forKeys: [.fileSizeKey]).fileSize)
        print("Downloaded \(wallpaper.fileName): \(size) bytes, \(progress.count) progress reports, last \(progress.last ?? -1)")
        #expect(Int64(size) == wallpaper.size)
        #expect(progress.count > 0)
        #expect(progress.last == Int64(size))

        let converted = try WallpaperImageConverter().ensureSupportedFormat(destination)
        let source = try #require(CGImageSourceCreateWithURL(converted as CFURL, nil))
        let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
        print("Usable as wallpaper: \(converted.lastPathComponent) \(image.width)x\(image.height)")
    }

    @Test func readsTheCurrentDesktopWithoutChangingIt() async {
        let screens = await NSWorkspaceWallpaperAPI().currentWallpapers()
        for screen in screens {
            print("Screen \(screen.screenID): \(screen.imageURL?.path ?? "none") \(screen.options)")
        }
        #expect(!screens.isEmpty)
    }

    /// "dark, 3 wallpapers [1.png, 2.png, 3.png]" (at most three names).
    static func summary(_ details: ThemeDetails) -> String {
        let names = details.wallpapers.prefix(3).map(\.fileName).joined(separator: ", ")
        return "\(details.mode), \(details.wallpapers.count) wallpapers [\(names)]"
    }
}

final class ProgressLog: @unchecked Sendable {
    private let lock = NSLock()
    private var values: [Int64] = []

    var count: Int { lock.withLock { values.count } }
    var last: Int64? { lock.withLock { values.last } }

    var record: @Sendable (Int64) -> Void {
        { value in self.lock.withLock { self.values.append(value) } }
    }
}
