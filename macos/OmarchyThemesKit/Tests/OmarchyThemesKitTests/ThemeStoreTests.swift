import Foundation
import Testing
@testable import OmarchyThemesKit
import OmarchyThemesTestSupport

struct ThemeStoreTests {
    let dir = TempDir()
    let downloader = FakeDownloader()
    let clock = ManualClock(Date(timeIntervalSince1970: 1_790_380_800)) // 2026-09-26 UTC
    let store: ThemeStore

    init() {
        store = ThemeStore(paths: dir.paths, downloader: downloader, now: clock.function)
    }

    static let tokyoPalette = Palette(
        background: RgbColor("#1a1b26"), foreground: RgbColor("#a9b1d6"), accent: RgbColor("#7aa2f7"), source: .colorsToml)

    static func details(_ slug: String = "tokyo", wallpapers: [String] = [], palette: Bool = true) -> ThemeDetails {
        let repo = RepoRef(owner: "o", name: slug)
        return ThemeDetails(
            entry: CatalogEntry(slug: slug, name: "Tokyo", repoURL: URL(string: "https://github.com/o/\(slug)")!,
                                screenshotURL: URL(string: "https://omarchy.org/assets/themes/\(slug).webp")),
            repo: repo,
            palette: palette ? tokyoPalette : nil,
            paletteError: nil,
            wallpapers: wallpapers.map { name in
                let path = "backgrounds/\(name)"
                return WallpaperRef(path: path, size: 100, downloadURL: GitHubClient.rawURL(repo, path))
            },
            mode: .dark)
    }

    private var themesDirContents: [String] {
        (try? FileManager.default.contentsOfDirectory(atPath: dir.paths.themesDir.path)) ?? []
    }

    @Test func installDownloadsWallpapersAndScreenshotAndWritesManifest() async throws {
        let reports = Recorder<DownloadProgress>()

        let installed = try await store.install(Self.details(wallpapers: ["1.png", "2.jpg"]), progress: reports.append)

        #expect(installed.wallpapers == ["1.png", "2.jpg"])
        #expect(FileManager.default.fileExists(atPath: installed.wallpaperURL("1.png").path))
        #expect(FileManager.default.fileExists(atPath: try #require(installed.screenshotURL).path))
        #expect(installed.downloadedAt == clock.now)

        let reloaded = try #require(store.get("tokyo"))
        #expect(reloaded.directory == installed.directory)
        #expect(reloaded.slug == "tokyo")
        #expect(reloaded.name == "Tokyo")
        #expect(reloaded.repoURL.absoluteString == "https://github.com/o/tokyo")
        #expect(reloaded.mode == .dark)
        #expect(reloaded.downloadedAt == clock.now)
        #expect(reloaded.wallpapers == installed.wallpapers)
        #expect(reloaded.screenshotFile == installed.screenshotFile)
        #expect(reloaded.palette?.accent == RgbColor("#7aa2f7"))
        #expect(store.list().count == 1)
        #expect(reports.values.contains { $0.fileIndex == 1 && $0.fileCount == 3 && $0.fileName == "2.jpg" })
    }

    @Test func failedDownloadLeavesNoPartialThemeBehind() async throws {
        let details = Self.details(wallpapers: ["1.png", "2.png"])
        downloader.fail(details.wallpapers[1].downloadURL)

        await #expect(throws: URLError.self) { try await store.install(details) }

        #expect(store.get("tokyo") == nil)
        #expect(themesDirContents.isEmpty)
    }

    @Test func cancelledDownloadLeavesNoPartialThemeBehind() async throws {
        let store = self.store
        let task = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            return try await store.install(Self.details(wallpapers: ["1.png"]))
        }

        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(themesDirContents.isEmpty)
    }

    @Test func screenshotFailureDoesNotFailInstall() async throws {
        let details = Self.details(wallpapers: ["1.png"])
        downloader.fail(try #require(details.entry.screenshotURL))

        let installed = try await store.install(details)

        #expect(installed.screenshotFile == nil)
        #expect(store.get("tokyo") != nil)
    }

    @Test func reinstallReplacesThePreviousCopy() async throws {
        _ = try await store.install(Self.details(wallpapers: ["old.png"]))
        let installed = try await store.install(Self.details(wallpapers: ["new.png"]))

        #expect(installed.wallpapers == ["new.png"])
        #expect(!FileManager.default.fileExists(atPath: installed.wallpaperURL("old.png").path))
        #expect(store.get("tokyo")?.wallpapers == ["new.png"])
        #expect(themesDirContents == ["tokyo"]) // no staging or backup folder left behind
    }

    @Test func duplicateWallpaperNamesAreMadeUnique() async throws {
        var details = Self.details(wallpapers: ["a.png"])
        var second = details.wallpapers[0]
        second.path = "backgrounds/dark/A.png"
        details.wallpapers.append(second)

        let installed = try await store.install(details)

        #expect(installed.wallpapers == ["a.png", "A-2.png"])
    }

    @Test func removeDeletesTheTheme() async throws {
        _ = try await store.install(Self.details(wallpapers: ["1.png"]))

        try store.remove("tokyo")

        #expect(store.get("tokyo") == nil)
        #expect(store.list().isEmpty)
    }

    @Test func listIsNewestFirstAndIgnoresJunk() async throws {
        _ = try await store.install(Self.details("older", wallpapers: ["1.png"]))
        clock.advance(by: 60)
        _ = try await store.install(Self.details("newer", wallpapers: ["1.png"]))
        try FileManager.default.createDirectory(at: dir.paths.themesDir.appending(path: "no-manifest"), withIntermediateDirectories: true)

        #expect(store.list().map(\.slug) == ["newer", "older"])
    }

    @Test func themeWithoutWallpapersOrPaletteCannotBeInstalled() async {
        await #expect(throws: NothingToDownloadError.self) { try await store.install(Self.details(palette: false)) }
    }

    @Test func paletteOnlyThemeInstallsWithoutWallpapers() async throws {
        let installed = try await store.install(Self.details())
        #expect(installed.wallpapers.isEmpty)
        #expect(installed.palette != nil)
    }

    @Test func cleanUpRemovesInterruptedStagingFolders() throws {
        let staging = dir.paths.themesDir.appending(path: ".staging-tokyo-123")
        try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)

        store.cleanUpStaging()

        #expect(!FileManager.default.fileExists(atPath: staging.path))
    }

    @Test func settingsRoundTripAndDefaultWhenMissingOrCorrupt() throws {
        let settings = SettingsStore(paths: dir.paths)
        #expect(!settings.load().welcomeSeen)

        try settings.update {
            $0.welcomeSeen = true
            $0.applyDefaults = ApplyOptions(accentColor: false, fit: .center)
        }
        let loaded = settings.load()
        #expect(loaded.welcomeSeen)
        #expect(!loaded.applyDefaults.accentColor)
        #expect(loaded.applyDefaults.wallpaper)
        #expect(loaded.applyDefaults.fit == .center)

        try Data("{ not json".utf8).write(to: dir.paths.settingsFile)
        #expect(settings.load() == AppSettings())
    }

    @Test func settingsWithMissingKeysUseDefaults() throws {
        try FileManager.default.createDirectory(at: dir.url, withIntermediateDirectories: true)
        try Data(#"{ "welcomeSeen": true, "applyDefaults": { "fit": "span" } }"#.utf8).write(to: dir.paths.settingsFile)

        let loaded = SettingsStore(paths: dir.paths).load()

        #expect(loaded.welcomeSeen)
        #expect(loaded.applyDefaults == ApplyOptions(fit: .span))
    }
}
