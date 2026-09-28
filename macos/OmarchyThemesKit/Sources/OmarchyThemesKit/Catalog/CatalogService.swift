import Foundation

public struct ThemeCatalog: Sendable {
    /// Omarchy's default themes first, then the community gallery.
    public let entries: [CatalogEntry]
    public let fetchedAt: Date
    /// True when omarchy.org couldn't be reached and a cached copy is shown.
    public let isStale: Bool
    public let staleReason: (any Error)?
    /// Why the default themes are missing or stale, if they are (the community gallery still loads).
    public let defaultThemesError: (any Error)?
}

public struct CatalogFormatError: LocalizedError, Sendable {
    public var errorDescription: String? {
        "No themes were found on omarchy.org/themes. The page layout may have changed; check for an app update."
    }
}

/// Loads the catalog: the community gallery from omarchy.org/themes plus, when a GitHub client is
/// given, the themes that ship with Omarchy. The raw page and repo tree are cached (not the parsed
/// result), so a parser fix applies to the cached copy too and the app starts instantly offline.
public struct CatalogService: Sendable {
    private let cache: HTTPCache
    private let pageURL: URL
    private let github: GitHubClient?

    public init(cache: HTTPCache, pageURL: URL = CatalogParser.defaultPageURL, github: GitHubClient? = nil) {
        self.cache = cache
        self.pageURL = pageURL
        self.github = github
    }

    /// The last downloaded catalog, without any network access; nil on first run.
    public func loadCached() -> ThemeCatalog? {
        guard let cached = cache.cachedResponse(for: pageURL) else { return nil }
        let entries = CatalogParser.parse(cached.text, pageURL: pageURL)
        guard !entries.isEmpty else { return nil }

        let defaults = github?.cachedTree(for: DefaultThemes.repo).map(DefaultThemes.entries(from:)) ?? []
        return ThemeCatalog(
            entries: defaults + entries, fetchedAt: cached.fetchedAt, isStale: false, staleReason: nil, defaultThemesError: nil)
    }

    /// Re-fetches omarchy.org/themes (conditional GET), falling back to the cached copy if offline.
    public func refresh() async throws -> ThemeCatalog {
        let response = try await cache.get(pageURL, options: CacheOptions(forceRevalidate: true))
        let entries = CatalogParser.parse(response.text, pageURL: pageURL)
        guard !entries.isEmpty else { throw CatalogFormatError() }

        let (defaults, defaultsError) = try await loadDefaultThemes()
        return ThemeCatalog(entries: defaults + entries, fetchedAt: response.fetchedAt, isStale: response.isStale,
                            staleReason: response.error, defaultThemesError: defaultsError)
    }

    /// The default themes, from Omarchy's repo tree (re-checked at most hourly, then with a free
    /// ETag revalidation). A GitHub failure with nothing cached drops them rather than the catalog.
    private func loadDefaultThemes() async throws -> ([CatalogEntry], (any Error)?) {
        guard let github else { return ([], nil) }
        do {
            let tree = try await github.tree(for: DefaultThemes.repo)
            return (DefaultThemes.entries(from: tree), tree.staleReason)
        } catch {
            if Task.isCancelled || error is CancellationError { throw error }
            return ([], error)
        }
    }
}
