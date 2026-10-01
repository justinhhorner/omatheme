import Foundation
import OmarchyThemesKit

/// Every store, wired together from one set of services. The app puts each store in the SwiftUI
/// environment, so a view depends only on the ones it uses.
@MainActor
public final class AppStores {
    public let paths: AppPaths
    public let isDryRun: Bool
    public let gitHubTokenIsSet: Bool

    public let banners: Banners
    public let preferences: Preferences
    public let catalog: CatalogStore
    public let library: ThemeLibrary
    public let desktop: DesktopStore
    public let terminals: TerminalStore

    private let httpCache: HTTPCache

    public init(services: AppServices, now: @escaping @Sendable () -> Date = { Date() }) {
        paths = services.paths
        httpCache = services.httpCache
        isDryRun = services.isDryRun
        gitHubTokenIsSet = services.gitHubTokenIsSet

        banners = Banners()
        preferences = Preferences(store: services.settings)
        catalog = CatalogStore(service: services.catalog, now: now)
        library = ThemeLibrary(resolver: services.resolver, store: services.themes, banners: banners)
        desktop = DesktopStore(applier: services.applier, preferences: preferences, library: library, banners: banners)
        terminals = TerminalStore(exporters: services.terminals, preferences: preferences, library: library)
    }

    /// Deletes the HTTP cache (catalog, GitHub responses) and forgets resolved themes. Downloaded
    /// themes are kept, and so are the app's thumbnails (it clears those through their own cache).
    public func clearCache() throws {
        // Even a partly cleared cache must be read again rather than served from memory.
        defer { library.clearDetailsCache() }
        try httpCache.clear()
    }
}
