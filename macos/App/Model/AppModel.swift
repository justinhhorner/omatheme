import Foundation
import Observation
import os
import OmarchyThemesKit
import OmarchyThemesMac

/// App-wide state and the Kit services behind it. Views read it from the environment.
@MainActor
@Observable
final class AppModel {
    /// The catalog is refreshed in the background when the cached copy is older than this.
    static let autoRefreshAfter: TimeInterval = 12 * 3600

    let paths: AppPaths
    let images: ImageLoader
    let isDryRun: Bool
    let gitHubTokenIsSet: Bool

    private let catalogService: CatalogService
    private let resolver: ThemeResolver
    private let store: ThemeStore
    private let settingsStore: SettingsStore
    private let applier: ThemeApplier

    // MARK: Catalog

    private(set) var entries: [CatalogEntry] = []
    private(set) var catalogFetchedAt: Date?
    /// First load with nothing cached.
    private(set) var isLoadingCatalog = false
    private(set) var isRefreshing = false
    /// Shown in place of the gallery when there is no catalog at all.
    private(set) var catalogError: String?
    /// Shown above the gallery, e.g. "showing the cached catalog".
    var catalogNotice: String?
    /// Shown above the gallery when Omarchy's default themes couldn't be loaded.
    var defaultThemesNotice: String?
    private var catalogLoaded = false
    private var refreshTask: Task<Void, Never>?
    private var resolveTasks: [String: Task<ThemeDetails, any Error>] = [:]

    // MARK: Downloads and settings

    private(set) var installed: [InstalledTheme] = []
    private(set) var settings = AppSettings()
    private(set) var hasOriginalSnapshot = false
    private(set) var downloads: [String: DownloadState] = [:]
    private(set) var applyingSlug: String?
    private var detailsCache: [String: ThemeDetails] = [:]

    /// Result banners, keyed by where they're shown (see `BannerContext`).
    var banners: [BannerContext: Banner] = [:]

    var showWelcome = false
    var sidebarSelection: SidebarItem? = .gallery

    init(environment: [String: String] = ProcessInfo.processInfo.environment) {
        // OMARCHY_THEMES_DATA_DIR points the app at another data folder (fresh state for testing).
        paths = environment["OMARCHY_THEMES_DATA_DIR"].map { AppPaths(root: URL(filePath: $0, directoryHint: .isDirectory)) } ?? .default()
        try? paths.ensureCreated()

        let cache = HTTPCache(transport: URLSessionTransport(session: HTTPSessions.make()), directory: paths.cacheDir)
        let token = environment["OMARCHY_THEMES_GITHUB_TOKEN"] ?? environment["GITHUB_TOKEN"]
        gitHubTokenIsSet = token?.isEmpty == false
        let github = GitHubClient(cache: cache, token: token)
        catalogService = CatalogService(cache: cache, github: github)
        resolver = ThemeResolver(github: github)
        store = ThemeStore(paths: paths, downloader: URLSessionDownloader(session: HTTPSessions.make(requestTimeout: 60)))
        settingsStore = SettingsStore(paths: paths)
        images = ImageLoader(cacheDirectory: paths.cacheDir.appending(path: "images", directoryHint: .isDirectory))

        // OMARCHY_THEMES_DRY_RUN=1 swaps in a backend that changes nothing, for UI testing.
        isDryRun = environment["OMARCHY_THEMES_DRY_RUN"] == "1"
        let backend: any DesktopBackend = isDryRun
            ? DryRunDesktopBackend()
            : MacDesktopBackend(snapshotAssetsDir: paths.originalDesktopDir)
        applier = ThemeApplier(backend: backend, snapshots: FileSnapshotStore(paths: paths))

        store.cleanUpStaging()
        settings = settingsStore.load()
        installed = store.list()
        hasOriginalSnapshot = applier.hasOriginalSnapshot
        showWelcome = !settings.welcomeSeen
    }

    // MARK: Catalog

    func loadCatalogIfNeeded() async {
        guard !catalogLoaded else { return }
        catalogLoaded = true

        if let cached = catalogService.loadCached() {
            show(cached)
            // Refresh when the cache is old, or has no default themes yet (e.g. they failed to load).
            let hasDefaultThemes = cached.entries.contains(where: \.isDefaultTheme)
            if hasDefaultThemes && Date().timeIntervalSince(cached.fetchedAt) < Self.autoRefreshAfter { return }
        } else {
            isLoadingCatalog = true
        }
        await refreshCatalog()
    }

    /// Re-fetches the catalog. The work runs in a model-owned task: a view's `.task` being
    /// cancelled (e.g. the view is recreated) must not cancel the request. Concurrent callers
    /// share one refresh.
    func refreshCatalog() async {
        if let refreshTask {
            return await refreshTask.value
        }
        let task = Task { await performRefresh() }
        refreshTask = task
        await task.value
        refreshTask = nil
    }

    private func performRefresh() async {
        isRefreshing = true
        catalogError = nil
        defer {
            isRefreshing = false
            isLoadingCatalog = false
        }

        do {
            let catalog = try await catalogService.refresh()
            show(catalog)
            catalogNotice = catalog.isStale
                ? "Couldn't reach omarchy.org, so this is the catalog from \(catalog.fetchedAt.formatted(.relative(presentation: .named)))."
                : nil
            if let error = catalog.defaultThemesError, !catalog.entries.contains(where: \.isDefaultTheme) {
                log.error("Default themes failed: \(String(describing: error), privacy: .public)")
                defaultThemesNotice = "The themes that come with Omarchy couldn't be loaded from GitHub. \(error.localizedDescription)"
            } else {
                defaultThemesNotice = nil
            }
        } catch {
            log.error("Catalog refresh failed: \(String(describing: error), privacy: .public)")
            let message = error is CatalogFormatError
                ? error.localizedDescription
                : "Couldn't load themes from omarchy.org. Check your internet connection and try again."
            if entries.isEmpty {
                catalogError = message
            } else {
                catalogNotice = message
            }
        }
    }

    private func show(_ catalog: ThemeCatalog) {
        entries = catalog.entries
        catalogFetchedAt = catalog.fetchedAt
    }

    /// The catalog entry for a downloaded theme, or one rebuilt from its manifest.
    func entry(for theme: InstalledTheme) -> CatalogEntry {
        entries.first { $0.slug == theme.slug }
            ?? CatalogEntry(slug: theme.slug, name: theme.name, repoURL: theme.repoURL, screenshotURL: nil)
    }

    // MARK: Theme details

    func installedTheme(_ slug: String) -> InstalledTheme? {
        installed.first { $0.slug == slug }
    }

    func cachedDetails(_ slug: String) -> ThemeDetails? {
        detailsCache[slug]
    }

    /// Resolves a theme from GitHub, memoized for the session so revisiting a theme is instant.
    /// Like the catalog refresh, the request runs in a model-owned task shared by concurrent
    /// callers, so leaving the page doesn't waste the (rate-limited) API call.
    func details(for entry: CatalogEntry, force: Bool = false) async throws -> ThemeDetails {
        if !force, let cached = detailsCache[entry.slug] { return cached }
        if let running = resolveTasks[entry.slug] {
            return try await running.value
        }
        let task = Task { [resolver] in try await resolver.resolve(entry) }
        resolveTasks[entry.slug] = task
        defer { resolveTasks[entry.slug] = nil }
        let details = try await task.value
        detailsCache[entry.slug] = details
        return details
    }

    // MARK: Download

    /// Downloads (or re-downloads) a theme. Runs in a task owned by the model, so it carries on
    /// if the user navigates away; returns nil if it failed or was cancelled.
    @discardableResult
    func download(_ entry: CatalogEntry) async -> InstalledTheme? {
        if let running = downloads[entry.slug] {
            return await running.task?.value ?? nil
        }

        let state = DownloadState()
        downloads[entry.slug] = state
        banners[.theme(entry.slug)] = nil
        let isReinstall = installedTheme(entry.slug) != nil

        let task = Task { () -> InstalledTheme? in
            defer { downloads[entry.slug] = nil }
            do {
                // Re-resolve when re-downloading so updates to the theme are picked up.
                let details = try await details(for: entry, force: isReinstall)
                state.begin(details)
                let installed = try await store.install(details) { progress in
                    Task { @MainActor in state.update(progress) }
                }
                reloadInstalled()
                return installed
            } catch {
                if Task.isCancelled || error is CancellationError || (error as? URLError)?.code == .cancelled {
                    banners[.theme(entry.slug)] = Banner(kind: .info, title: "Download cancelled", message: "Nothing was saved.")
                } else {
                    log.error("Download of \(entry.slug, privacy: .public) failed: \(String(describing: error), privacy: .public)")
                    let message = (error as? URLError)?.code == .timedOut ? "The connection timed out. Try again." : error.localizedDescription
                    banners[.theme(entry.slug)] = Banner(kind: .error, title: "Download failed", message: message)
                }
                return nil
            }
        }
        state.task = task
        return await task.value
    }

    func cancelDownload(_ slug: String) {
        downloads[slug]?.task?.cancel()
    }

    func remove(_ theme: InstalledTheme) {
        do {
            try store.remove(theme.slug)
            banners[.theme(theme.slug)] = Banner(kind: .info, title: "Download removed", message: "\(theme.name) was removed from this Mac.")
        } catch {
            banners[.theme(theme.slug)] = Banner(kind: .error, title: "Couldn't remove \(theme.name)", message: error.localizedDescription)
        }
        reloadInstalled()
    }

    func reloadInstalled() {
        installed = store.list()
    }

    // MARK: Apply

    var capabilities: DesktopCapabilities { applier.capabilities }

    var supportedFits: [WallpaperFit] { applier.supportedFits }

    var activeSlug: String? { settings.lastAppliedSlug }

    /// The wallpaper to preselect for a downloaded theme: the one last applied, else the first.
    func preferredWallpaper(for theme: InstalledTheme) -> String? {
        if settings.lastAppliedSlug == theme.slug, let last = settings.lastAppliedWallpaper, theme.wallpapers.contains(last) {
            return last
        }
        return theme.wallpapers.first
    }

    func apply(_ theme: InstalledTheme, wallpaperFile: String?, options: ApplyOptions) async -> ApplySummary {
        guard applyingSlug == nil else {
            return ApplySummary(kind: .info, title: "Already applying a theme", message: "Wait for it to finish, then try again.")
        }
        applyingSlug = theme.slug
        defer {
            applyingSlug = nil
            hasOriginalSnapshot = applier.hasOriginalSnapshot
        }

        let request = ApplyRequest.from(theme, wallpaperFile: wallpaperFile, options: options)
        do {
            let result = try await applier.apply(request)
            if result.anyApplied {
                updateSettings {
                    $0.lastAppliedSlug = theme.slug
                    $0.lastAppliedWallpaper = request.wallpaper?.lastPathComponent
                }
            }
            return ApplySummary.describe(result, themeName: theme.name, mode: theme.mode)
        } catch {
            return ApplySummary(kind: .info, title: "Apply cancelled", message: "Nothing was changed.")
        }
    }

    /// One-click apply with the defaults from Settings.
    func applyWithDefaults(_ theme: InstalledTheme) async -> ApplySummary {
        await apply(theme, wallpaperFile: preferredWallpaper(for: theme), options: settings.applyDefaults)
    }

    func restoreOriginalDesktop() async -> Banner {
        defer { hasOriginalSnapshot = applier.hasOriginalSnapshot }
        do {
            guard try await applier.restoreOriginal() else {
                return Banner(kind: .info, title: "Nothing to restore", message: "No saved desktop was found.")
            }
            updateSettings {
                $0.lastAppliedSlug = nil
                $0.lastAppliedWallpaper = nil
            }
            return Banner(kind: .success, title: "Original desktop restored", message: "Your previous wallpaper is back.")
        } catch {
            return Banner(kind: .error, title: "Couldn't restore your desktop", message: error.localizedDescription)
        }
    }

    // MARK: Settings

    func updateSettings(_ change: (inout AppSettings) -> Void) {
        var updated = settings
        change(&updated)
        guard updated != settings else { return }
        settings = updated
        try? settingsStore.save(updated)
    }

    func dismissWelcome() {
        showWelcome = false
        updateSettings { $0.welcomeSeen = true }
    }

    func clearCache() throws {
        let fm = FileManager.default
        if fm.fileExists(atPath: paths.cacheDir.path) {
            try fm.removeItem(at: paths.cacheDir)
        }
        try fm.createDirectory(at: paths.cacheDir, withIntermediateDirectories: true)
        images.clearMemory()
        detailsCache.removeAll()
    }
}

enum SidebarItem: Hashable {
    case gallery
    case downloaded
}

enum BannerContext: Hashable {
    case gallery
    case downloaded
    case settings
    case theme(String)
}

struct Banner: Equatable, Identifiable {
    let id = UUID()
    var kind: SummaryKind
    var title: String
    var message: String

    init(kind: SummaryKind, title: String, message: String) {
        self.kind = kind
        self.title = title
        self.message = message
    }

    init(_ summary: ApplySummary) {
        self.init(kind: summary.kind, title: summary.title, message: summary.message)
    }
}

/// Progress of one theme download, shown on its detail page.
@MainActor
@Observable
final class DownloadState {
    private(set) var fraction: Double?
    private(set) var status = "Reading theme from GitHub…"
    fileprivate(set) var task: Task<InstalledTheme?, Never>?

    private var sizes: [Int64] = []
    private var totalBytes: Int64 = 0
    private var wallpaperCount = 0

    fileprivate func begin(_ details: ThemeDetails) {
        sizes = details.wallpapers.map { $0.size ?? 0 }
        totalBytes = sizes.reduce(0, +)
        wallpaperCount = details.wallpapers.count
        fraction = totalBytes == 0 ? nil : 0
    }

    fileprivate func update(_ progress: DownloadProgress) {
        guard progress.fileIndex < wallpaperCount else {
            status = "Finishing up…"
            return
        }
        let done = sizes.prefix(progress.fileIndex).reduce(0, +) + progress.bytesReceived
        if totalBytes > 0 {
            fraction = min(1, Double(done) / Double(totalBytes))
        }
        status = "Downloading wallpaper \(progress.fileIndex + 1) of \(wallpaperCount) · "
            + "\(done.formatted(.byteCount(style: .file))) of \(totalBytes.formatted(.byteCount(style: .file)))"
    }
}

private let log = Logger(subsystem: "com.justinhhorner.OmarchyThemes", category: "app")
