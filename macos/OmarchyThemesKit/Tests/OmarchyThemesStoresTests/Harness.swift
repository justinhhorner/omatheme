import Foundation
import OmarchyThemesKit
import OmarchyThemesMac
import OmarchyThemesTestSupport
@testable import OmarchyThemesStores

/// The stores wired to fakes: a routed HTTP transport, a downloader that writes the URL into each
/// file, a desktop backend that records calls, in-memory terminals and a temp data folder.
@MainActor
final class Harness {
    static let pageURL = CatalogParser.defaultPageURL.absoluteString
    static let treeURL = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1"
    static let raw = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/"

    let dir = TempDir()
    let http = FakeTransport()
    let downloader = FakeDownloader()
    let backend = FakeDesktopBackend()
    let snapshots = InMemorySnapshotStore()
    let clock = ManualClock(Date(timeIntervalSince1970: 1_790_467_200)) // 2026-09-27 UTC
    let iterm = FakeTerminalExporter(id: "iterm2", displayName: "iTerm2")
    let terminalApp = FakeTerminalExporter(id: "terminal", displayName: "Terminal", canRemove: false)

    /// Like the macOS backend: only the wallpaper can be changed.
    init() {
        backend.capabilities = .wallpaper
    }

    /// The catalog page and Omarchy's repo tree, with Tokyo Night's palette.
    func serveCatalog() {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Self.treeURL, body: Fixture.read("omarchy-tree.json"))
            .on(Self.raw + "tokyo-night/colors.toml", body: Fixture.read("colors-named.toml"))
    }

    var services: AppServices {
        let cache = HTTPCache(transport: http, directory: dir.paths.cacheDir, now: clock.function)
        let github = GitHubClient(cache: cache)
        return AppServices(
            paths: dir.paths,
            httpCache: cache,
            catalog: CatalogService(cache: cache, github: github),
            resolver: ThemeResolver(github: github),
            themes: ThemeStore(paths: dir.paths, downloader: downloader, now: clock.function),
            settings: SettingsStore(paths: dir.paths),
            applier: ThemeApplier(backend: backend, snapshots: snapshots),
            terminals: [iterm, terminalApp])
    }

    /// A fresh set of stores over the same data folder (like relaunching the app).
    func makeStores() -> AppStores {
        AppStores(services: services, now: clock.function)
    }

    static let tokyoEntry = CatalogEntry(
        slug: "omarchy.tokyo-night", name: "Tokyo Night",
        repoURL: URL(string: "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night")!,
        screenshotURL: URL(string: raw + "tokyo-night/preview.png"))

    /// Downloads Tokyo Night (three wallpapers in the fixture tree) through the library.
    @discardableResult
    func installTokyo(in stores: AppStores) async throws -> InstalledTheme {
        serveCatalog()
        guard let theme = await stores.library.download(Self.tokyoEntry) else {
            throw FakeFailure(message: "Download failed: \(String(describing: stores.banners[.theme(Self.tokyoEntry.slug)]))")
        }
        return theme
    }
}
