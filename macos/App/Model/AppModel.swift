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

    /// Terminal apps a theme's colors can be sent to (see `TerminalExporters`).
    let terminals: [any TerminalExporter]
    /// Bumped after adding or removing, so views re-read `isAdded`.
    private var terminalRevision = 0

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
    /// Bumped by Clear Cache, so lookups already in flight don't refill the cache.
    private var cacheGeneration = 0

    /// Result banners, keyed by where they're shown.
    var banners: [BannerContext: Banner] = [:]

    var showWelcome = false
    var sidebarSelection: SidebarItem? = .gallery

    init(environment: [String: String] = ProcessInfo.processInfo.environment) {
        // OMARCHY_THEMES_DATA_DIR points the app at another data folder (fresh state for testing).
        paths = environment["OMARCHY_THEMES_DATA_DIR"]
            .map { AppPaths(root: URL(filePath: $0, directoryHint: .isDirectory)) } ?? .default()
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
        terminals = TerminalExporters.all(in: Self.terminalEnvironment(paths: paths, isDryRun: isDryRun))

        store.cleanUpStaging()
        settings = settingsStore.load()
        installed = store.list()
        hasOriginalSnapshot = applier.hasOriginalSnapshot
        showWelcome = !settings.welcomeSeen
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
                ? "Couldn't reach omarchy.org, so this is the catalog from \(catalog.fetchedAt.relativeDescription)."
                : nil
            if let error = catalog.defaultThemesError, !catalog.entries.contains(where: \.isDefaultTheme) {
                logFailure("Default themes", error)
                defaultThemesNotice = "The themes that come with Omarchy couldn't be loaded from GitHub. \(error.localizedDescription)"
            } else {
                defaultThemesNotice = nil
            }
        } catch {
            logFailure("Catalog refresh", error)
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

    /// The downloaded theme's name if there is one (it's what was saved), else the catalog's.
    func themeName(for entry: CatalogEntry) -> String {
        installedTheme(entry.slug)?.name ?? entry.name
    }

    func cachedDetails(_ slug: String) -> ThemeDetails? {
        detailsCache[slug]
    }

    /// Resolves a theme from GitHub, memoized for the session so revisiting a theme is instant.
    /// Like the catalog refresh, the request runs in a model-owned task shared by concurrent
    /// callers, so leaving the page doesn't waste the (rate-limited) API call.
    func details(for entry: CatalogEntry, force: Bool = false) async throws -> ThemeDetails {
        if !force {
            if let cached = detailsCache[entry.slug] { return cached }
            if let running = resolveTasks[entry.slug] { return try await running.value }
        }
        // A forced lookup (Download Again) starts fresh rather than joining one already running.
        let generation = cacheGeneration
        let task = Task { [resolver] in try await resolver.resolve(entry) }
        resolveTasks[entry.slug] = task
        defer {
            if resolveTasks[entry.slug] == task { resolveTasks[entry.slug] = nil }
        }
        let details = try await task.value
        // Don't refill the cache if it was cleared while this was in flight.
        if generation == cacheGeneration { detailsCache[entry.slug] = details }
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
                banners[.theme(entry.slug)] = downloadFailureBanner(error, slug: entry.slug)
                return nil
            }
        }
        state.task = task
        return await task.value
    }

    private func downloadFailureBanner(_ error: any Error, slug: String) -> Banner {
        if Task.isCancelled || error is CancellationError || (error as? URLError)?.code == .cancelled {
            return Banner(kind: .info, title: "Download cancelled", message: "Nothing was saved.")
        }
        logFailure("Download of \(slug)", error)
        let message = (error as? URLError)?.code == .timedOut
            ? "The connection timed out. Try again."
            : error.localizedDescription
        return Banner(kind: .error, title: "Download failed", message: message)
    }

    func cancelDownload(_ slug: String) {
        downloads[slug]?.task?.cancel()
    }

    /// Removes a downloaded theme. Returns the outcome for the caller to show where the user is.
    func remove(_ theme: InstalledTheme) -> Banner {
        defer { reloadInstalled() }
        do {
            try store.remove(theme.slug)
            return Banner(kind: .info, title: "Download removed", message: "\(theme.name) was removed from this Mac.")
        } catch {
            logFailure("Removing \(theme.slug)", error)
            return Banner(kind: .error, title: "Couldn't remove \(theme.name)", message: error.localizedDescription)
        }
    }

    func reloadInstalled() {
        installed = store.list()
    }

    // MARK: Apply

    var capabilities: DesktopCapabilities { applier.capabilities }

    var supportedFits: [WallpaperFit] { applier.supportedFits }

    /// The saved one-click fit, or Fill if this Mac doesn't offer it (e.g. Tile from Windows).
    var defaultFit: WallpaperFit {
        supportedFits.contains(settings.applyDefaults.fit) ? settings.applyDefaults.fit : .fill
    }

    var activeSlug: String? { settings.lastAppliedSlug }

    /// The wallpaper to preselect for a downloaded theme: the one last applied, else the first.
    func preferredWallpaper(for theme: InstalledTheme) -> String? {
        if settings.lastAppliedSlug == theme.slug,
           let last = settings.lastAppliedWallpaper,
           theme.wallpapers.contains(last) {
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
            updateSettings { $0 = $0.afterApply(theme.slug, wallpaperFile: request.wallpaper?.lastPathComponent, result: result) }
            return ApplySummary.describe(result, themeName: theme.name, mode: theme.mode)
        } catch is CancellationError {
            return ApplySummary(kind: .info, title: "Apply cancelled", message: "Nothing was changed.")
        } catch {
            // ThemeApplier reports per-step failures in its result; anything thrown is unexpected.
            logFailure("Applying \(theme.slug)", error)
            return ApplySummary(kind: .error, title: "Couldn't apply \(theme.name)", message: error.localizedDescription)
        }
    }

    /// One-click apply with the defaults from Settings.
    func applyWithDefaults(_ theme: InstalledTheme) async -> ApplySummary {
        await apply(theme, wallpaperFile: preferredWallpaper(for: theme), options: settings.applyDefaults)
    }

    // MARK: Current theme

    /// The theme on the desktop, if it's still downloaded (nil after Restore, which clears it).
    var currentTheme: InstalledTheme? {
        settings.lastAppliedSlug.flatMap(installedTheme)
    }

    /// Which of the current theme's wallpapers is on the desktop, if any.
    var currentWallpaper: String? { settings.lastAppliedWallpaper }

    /// The current theme's wallpaper being set right now, for its spinner.
    private(set) var settingWallpaper: String?

    /// Sets another of the current theme's wallpapers: wallpaper only (its light/dark and accent
    /// are already applied), with the saved fit and the theme's background as fill color.
    /// Success shows as the check mark moving; problems go to the card's banner.
    func setCurrentWallpaper(_ file: String) async {
        guard let theme = currentTheme, file != currentWallpaper, applyingSlug == nil else { return }
        settingWallpaper = file
        banners[.currentTheme] = nil
        defer { settingWallpaper = nil }

        let options = ApplyOptions(wallpaper: true, appearanceMode: false, accentColor: false, fit: settings.applyDefaults.fit)
        let summary = await apply(theme, wallpaperFile: file, options: options)
        if summary.kind != .success {
            var banner = Banner(summary)
            if summary.kind == .error { banner.title = "Couldn't change the wallpaper" }
            banners[.currentTheme] = banner
        }
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

    // MARK: Terminal colors

    /// The terminal picked on theme pages (iTerm2 until the user picks another).
    var selectedTerminal: any TerminalExporter {
        terminals.first { $0.id == settings.terminalApp }
            ?? terminals.first { $0.id == TerminalExporters.defaultID }
            ?? terminals[0]
    }

    func selectTerminal(_ id: String) {
        updateSettings { $0.terminalApp = id }
    }

    /// Whether the selected terminal already has the theme's colors.
    func isAddedToSelectedTerminal(_ entry: CatalogEntry) -> Bool {
        _ = terminalRevision // Observed, so views ask again after an add or remove.
        return selectedTerminal.isAdded(slug: entry.slug, themeName: themeName(for: entry))
    }

    func addToTerminal(_ entry: CatalogEntry, palette: Palette) async -> Banner {
        let exporter = selectedTerminal
        let name = themeName(for: entry)
        let scheme = exporter.schemeName(forTheme: name)
        let colors = TerminalColors(await paletteForTerminal(entry, saved: palette))
        do {
            try exporter.add(slug: entry.slug, themeName: name, colors: colors)
            terminalRevision += 1
            // Terminal.app imports the profile after it opens the file.
            Task {
                try? await Task.sleep(for: .seconds(2))
                terminalRevision += 1
            }
            return Banner(kind: .success, title: "Added to \(exporter.displayName)", message: exporter.addedMessage(scheme: scheme))
        } catch {
            logFailure("Adding \(entry.slug) to \(exporter.id)", error)
            return Banner(kind: .error, title: "Couldn't add to \(exporter.displayName)", message: error.localizedDescription)
        }
    }

    /// Removes the theme's colors from the selected terminal, or says how to where that's manual.
    func removeFromTerminal(_ entry: CatalogEntry) -> Banner {
        let exporter = selectedTerminal
        let name = themeName(for: entry)
        let scheme = exporter.schemeName(forTheme: name)
        guard exporter.canRemove else {
            return Banner(
                kind: .info, title: "Remove it in \(exporter.displayName)", message: exporter.removeInstructions(scheme: scheme))
        }
        do {
            try exporter.remove(slug: entry.slug, themeName: name)
            terminalRevision += 1
            return Banner(
                kind: .info, title: "Removed from \(exporter.displayName)", message: exporter.removedMessage(scheme: scheme))
        } catch {
            logFailure("Removing \(entry.slug) from \(exporter.id)", error)
            return Banner(
                kind: .error, title: "Couldn't remove it from \(exporter.displayName)", message: error.localizedDescription)
        }
    }

    /// Themes downloaded before `muted` and `bright_foreground` were read saved a palette without
    /// them, so look the theme up again (cached, usually free) for Omarchy's exact bright black,
    /// bright white and cursor. Offline, use what's saved.
    private func paletteForTerminal(_ entry: CatalogEntry, saved: Palette) async -> Palette {
        let savedWithoutNamedExtras = saved.source == .colorsToml && saved.muted == nil && saved.brightForeground == nil
            && !saved.swatches.contains { $0.name == "Bright black" }
        guard installedTheme(entry.slug) != nil, savedWithoutNamedExtras else { return saved }
        return (try? await details(for: entry))?.palette ?? saved
    }

    // MARK: Settings

    func updateSettings(_ change: (inout AppSettings) -> Void) {
        var updated = settings
        change(&updated)
        guard updated != settings else { return }
        settings = updated
        do {
            try settingsStore.save(updated)
        } catch {
            // The change still applies for this session; it just won't be there next launch.
            logFailure("Saving settings", error)
        }
    }

    func dismissWelcome() {
        showWelcome = false
        updateSettings { $0.welcomeSeen = true }
    }

    func clearCache() throws {
        let fm = FileManager.default
        try fm.removeItemIfPresent(at: paths.cacheDir)
        try fm.createDirectory(at: paths.cacheDir, withIntermediateDirectories: true)
        images.clearMemory()
        detailsCache.removeAll()
        cacheGeneration += 1
    }

    /// Logs "<action> failed: <error>".
    private func logFailure(_ action: String, _ error: any Error) {
        log.error("\(action, privacy: .public) failed: \(String(describing: error), privacy: .public)")
    }
}

enum SidebarItem: Hashable {
    case gallery
    case downloaded
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
