import Foundation
import Observation
import OmarchyThemesKit

/// Downloaded themes, theme lookups (GitHub) and downloads in progress.
@MainActor
@Observable
public final class ThemeLibrary {
    public private(set) var installed: [InstalledTheme] = []
    public private(set) var downloads: [String: DownloadState] = [:]

    private let resolver: ThemeResolver
    private let store: ThemeStore
    private let banners: Banners
    private var detailsCache: [String: ThemeDetails] = [:]
    private var resolveTasks: [String: Task<ThemeDetails, any Error>] = [:]
    /// Bumped by `clearDetailsCache`, so lookups already in flight don't refill the cache.
    private var cacheGeneration = 0

    public init(resolver: ThemeResolver, store: ThemeStore, banners: Banners) {
        self.resolver = resolver
        self.store = store
        self.banners = banners
        store.cleanUpStaging()
        installed = store.list()
    }

    public func installedTheme(_ slug: String) -> InstalledTheme? {
        installed.first { $0.slug == slug }
    }

    /// The downloaded theme's name if there is one (it's what was saved), else the catalog's.
    public func themeName(for entry: CatalogEntry) -> String {
        installedTheme(entry.slug)?.name ?? entry.name
    }

    public func reload() {
        installed = store.list()
    }

    // MARK: Lookups

    public func cachedDetails(_ slug: String) -> ThemeDetails? {
        detailsCache[slug]
    }

    /// Resolves a theme from GitHub, memoized for the session so revisiting a theme is instant.
    /// The request runs in a store-owned task shared by concurrent callers, so leaving the page
    /// doesn't waste the (rate-limited) API call.
    public func details(for entry: CatalogEntry, force: Bool = false) async throws -> ThemeDetails {
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

    public func clearDetailsCache() {
        detailsCache.removeAll()
        cacheGeneration += 1
    }

    // MARK: Downloads

    /// Downloads (or re-downloads) a theme. Runs in a store-owned task, so it carries on if the
    /// user navigates away; concurrent calls for the same theme share it. Returns nil if it
    /// failed or was cancelled, with the reason in the theme's banner.
    @discardableResult
    public func download(_ entry: CatalogEntry) async -> InstalledTheme? {
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
                reload()
                return installed
            } catch {
                banners[.theme(entry.slug)] = Self.downloadFailureBanner(error, slug: entry.slug)
                return nil
            }
        }
        state.task = task
        return await task.value
    }

    public func cancelDownload(_ slug: String) {
        downloads[slug]?.task?.cancel()
    }

    /// Removes a downloaded theme. Returns the outcome for the caller to show where the user is.
    public func remove(_ theme: InstalledTheme) -> Banner {
        defer { reload() }
        do {
            try store.remove(theme.slug)
            return Banner(kind: .info, title: "Download removed", message: "\(theme.name) was removed from this Mac.")
        } catch {
            logFailure("Removing \(theme.slug)", error)
            return Banner(kind: .error, title: "Couldn't remove \(theme.name)", message: error.localizedDescription)
        }
    }

    private static func downloadFailureBanner(_ error: any Error, slug: String) -> Banner {
        if Task.isCancelled || error is CancellationError || (error as? URLError)?.code == .cancelled {
            return Banner(kind: .info, title: "Download cancelled", message: "Nothing was saved.")
        }
        logFailure("Download of \(slug)", error)
        let message = (error as? URLError)?.code == .timedOut
            ? "The connection timed out. Try again."
            : error.localizedDescription
        return Banner(kind: .error, title: "Download failed", message: message)
    }
}

/// Progress of one theme download, shown on its detail page.
@MainActor
@Observable
public final class DownloadState {
    public private(set) var fraction: Double?
    public private(set) var status = "Reading theme from GitHub…"
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
