import Foundation
import OmarchyThemesKit
import OmarchyThemesMac

/// The services the stores are built from. `live()` wires up the real Mac; tests pass fakes
/// (an HTTP transport, downloader, desktop backend, terminal exporters) instead.
public struct AppServices: Sendable {
    public var paths: AppPaths
    /// The HTTP cache behind the catalog and GitHub lookups (for Clear Cache).
    public var httpCache: HTTPCache
    public var catalog: CatalogService
    public var resolver: ThemeResolver
    public var themes: ThemeStore
    public var settings: SettingsStore
    public var applier: ThemeApplier
    public var terminals: [any TerminalExporter]
    public var gitHubTokenIsSet: Bool
    public var isDryRun: Bool

    public init(
        paths: AppPaths, httpCache: HTTPCache, catalog: CatalogService, resolver: ThemeResolver, themes: ThemeStore,
        settings: SettingsStore, applier: ThemeApplier, terminals: [any TerminalExporter], gitHubTokenIsSet: Bool = false,
        isDryRun: Bool = false
    ) {
        self.paths = paths
        self.httpCache = httpCache
        self.catalog = catalog
        self.resolver = resolver
        self.themes = themes
        self.settings = settings
        self.applier = applier
        self.terminals = terminals
        self.gitHubTokenIsSet = gitHubTokenIsSet
        self.isDryRun = isDryRun
    }

    /// The real services. Test switches:
    /// - `OMARCHY_THEMES_DATA_DIR=<dir>` uses another data folder (fresh state for testing);
    /// - `OMARCHY_THEMES_DRY_RUN=1` swaps in a backend that changes nothing, and keeps terminal
    ///   exports inside the data folder.
    public static func live(environment: [String: String] = ProcessInfo.processInfo.environment) -> AppServices {
        let paths = environment["OMARCHY_THEMES_DATA_DIR"]
            .map { AppPaths(root: URL(filePath: $0, directoryHint: .isDirectory)) } ?? .default()
        try? paths.ensureCreated()

        let cache = HTTPCache(transport: URLSessionTransport(session: HTTPSessions.make()), directory: paths.cacheDir)
        let token = environment["GITHUB_TOKEN"]
        let github = GitHubClient(cache: cache, token: token)

        let isDryRun = environment["OMARCHY_THEMES_DRY_RUN"] == "1"
        let backend: any DesktopBackend = isDryRun
            ? DryRunDesktopBackend()
            : MacDesktopBackend(snapshotAssetsDir: paths.originalDesktopDir)

        return AppServices(
            paths: paths,
            httpCache: cache,
            catalog: CatalogService(cache: cache, github: github),
            resolver: ThemeResolver(github: github),
            themes: ThemeStore(paths: paths, downloader: URLSessionDownloader(session: HTTPSessions.make(requestTimeout: 60))),
            settings: SettingsStore(paths: paths),
            applier: ThemeApplier(backend: backend, snapshots: FileSnapshotStore(paths: paths)),
            terminals: TerminalExporters.all(in: terminalEnvironment(paths: paths, isDryRun: isDryRun)),
            gitHubTokenIsSet: token?.isEmpty == false,
            isDryRun: isDryRun)
    }

    /// In a dry run, terminal exports go to a folder in the data directory and nothing is opened,
    /// so UI checks can't touch real terminal settings either.
    private static func terminalEnvironment(paths: AppPaths, isDryRun: Bool) -> TerminalEnvironment {
        var environment = TerminalEnvironment.live(
            exportsDirectory: paths.root.appending(path: "terminal", directoryHint: .isDirectory))
        if isDryRun {
            let home = paths.root.appending(path: "dry-run-home", directoryHint: .isDirectory)
            environment.homeDirectory = home
            environment.configDirectory = home.appending(path: ".config", directoryHint: .isDirectory)
            environment.open = { file, _ in log.info("Dry run: would open \(file.path, privacy: .public)") }
        }
        return environment
    }
}

/// Used when OMARCHY_THEMES_DRY_RUN=1: behaves like the macOS backend (wallpaper only) but
/// changes nothing, so the UI can be exercised end to end without touching the real desktop.
struct DryRunDesktopBackend: DesktopBackend {
    let capabilities: DesktopCapabilities = [.wallpaper]
    let supportedFits: [WallpaperFit] = [.fill, .fit, .stretch, .center]

    func capture() async throws -> DesktopSnapshot {
        DesktopSnapshot(takenAt: Date(), values: ["dryRun": "1"])
    }

    func restore(_ snapshot: DesktopSnapshot) async throws {}

    func setWallpaper(_ image: URL, fit: WallpaperFit, fillColor: RgbColor?) async throws {
        try await Task.sleep(for: .milliseconds(300))
    }

    func setAppearanceMode(_ mode: AppearanceMode) async throws {}

    func setAccentColor(_ accent: RgbColor) async throws {}
}
