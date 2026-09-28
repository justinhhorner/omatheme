import Foundation
import Observation
import OmarchyThemesKit

/// The gallery's catalog: Omarchy's default themes and the community gallery.
@MainActor
@Observable
public final class CatalogStore {
    /// The catalog is refreshed in the background when the cached copy is older than this.
    public static let autoRefreshAfter: TimeInterval = 12 * 3600

    public private(set) var entries: [CatalogEntry] = []
    public private(set) var fetchedAt: Date?
    /// First load with nothing cached.
    public private(set) var isLoading = false
    public private(set) var isRefreshing = false
    /// Shown in place of the gallery when there is no catalog at all.
    public private(set) var error: String?
    /// Shown above the gallery, e.g. "showing the cached catalog".
    public var notice: String?
    /// Shown above the gallery when Omarchy's default themes couldn't be loaded.
    public var defaultThemesNotice: String?

    private let service: CatalogService
    private let now: @Sendable () -> Date
    private var loaded = false
    private var refreshTask: Task<Void, Never>?

    public init(service: CatalogService, now: @escaping @Sendable () -> Date = { Date() }) {
        self.service = service
        self.now = now
    }

    /// Shows the cached catalog straight away, then refreshes it if it's old or has no default
    /// themes yet (e.g. they failed to load last time). Only the first call does anything.
    public func loadIfNeeded() async {
        guard !loaded else { return }
        loaded = true

        if let cached = service.loadCached() {
            show(cached)
            let hasDefaultThemes = cached.entries.contains(where: \.isDefaultTheme)
            if hasDefaultThemes && now().timeIntervalSince(cached.fetchedAt) < Self.autoRefreshAfter { return }
        } else {
            isLoading = true
        }
        await refresh()
    }

    /// Re-fetches the catalog. The work runs in a store-owned task: a view's `.task` being
    /// cancelled (e.g. the view is recreated) must not cancel the request. Concurrent callers
    /// share one refresh.
    public func refresh() async {
        if let refreshTask {
            return await refreshTask.value
        }
        let task = Task { await performRefresh() }
        refreshTask = task
        await task.value
        refreshTask = nil
    }

    /// The catalog entry for a downloaded theme, or one rebuilt from its manifest.
    public func entry(for theme: InstalledTheme) -> CatalogEntry {
        entries.first { $0.slug == theme.slug }
            ?? CatalogEntry(slug: theme.slug, name: theme.name, repoURL: theme.repoURL, screenshotURL: nil)
    }

    private func performRefresh() async {
        isRefreshing = true
        error = nil
        defer {
            isRefreshing = false
            isLoading = false
        }

        do {
            let catalog = try await service.refresh()
            show(catalog)
            notice = catalog.isStale
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
                self.error = message
            } else {
                notice = message
            }
        }
    }

    private func show(_ catalog: ThemeCatalog) {
        entries = catalog.entries
        fetchedAt = catalog.fetchedAt
    }
}
