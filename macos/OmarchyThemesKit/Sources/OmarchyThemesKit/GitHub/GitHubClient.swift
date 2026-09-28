import Foundation

public enum GitHubError: LocalizedError, Sendable {
    /// The repository doesn't exist, was made private, or is empty.
    case notFound(repo: String)
    /// The (unauthenticated: 60/hour) API rate limit is exhausted.
    case rateLimited(resetsAt: Date?)
    case fileNotFound(path: String, repo: String)
    case unreadableTree(repo: String)
    case http(status: Int, repo: String)

    public var errorDescription: String? {
        switch self {
        case .notFound(let repo):
            "The GitHub repository \(repo) couldn't be found. It may have been renamed, deleted or made private."
        case .rateLimited(let resetsAt?):
            "GitHub's rate limit was reached. It resets at \(resetsAt.formatted(date: .omitted, time: .shortened))."
        case .rateLimited(nil):
            "GitHub's rate limit was reached. Try again in a few minutes."
        case .fileNotFound(let path, let repo):
            "\(path) wasn't found in \(repo)."
        case .unreadableTree(let repo):
            "GitHub returned an unreadable file list for \(repo)."
        case .http(let status, let repo):
            "GitHub returned \(status) \(HTTPURLResponse.localizedString(forStatusCode: status)) for \(repo)."
        }
    }
}

public struct RepoTreeItem: Hashable, Sendable {
    /// Path relative to the theme root.
    public let path: String
    public let isFile: Bool
    public let size: Int64?

    public init(path: String, isFile: Bool, size: Int64?) {
        self.path = path
        self.isFile = isFile
        self.size = size
    }

    public var fileName: String { path.split(separator: "/").last.map(String.init) ?? path }
}

public struct RepoTree: Sendable {
    /// Paths relative to the theme root (the repo's sub-folder if the link had one).
    public let items: [RepoTreeItem]
    public let truncated: Bool
    /// True when served from cache because GitHub couldn't be reached.
    public let isStale: Bool
    public let staleReason: (any Error)?

    public var files: [RepoTreeItem] { items.filter(\.isFile) }

    public func findFile(_ path: String) -> RepoTreeItem? {
        files.first { $0.path.caseInsensitiveCompare(path) == .orderedSame }
    }
}

/// Minimal GitHub access for theme repos. Uses exactly one REST call per repo (the recursive
/// tree) and fetches file contents from raw.githubusercontent.com, which isn't API-metered.
/// Everything goes through `HTTPCache` so revisits are free and work offline.
public struct GitHubClient: Sendable {
    /// Repo contents are re-checked at most this often (ETag revalidation after that).
    public static let freshFor: TimeInterval = 3600

    private let cache: HTTPCache
    private let token: String?

    public init(cache: HTTPCache, token: String? = nil) {
        self.cache = cache
        let trimmed = token?.trimmingCharacters(in: .whitespacesAndNewlines)
        self.token = trimmed?.isEmpty == false ? trimmed : nil
    }

    public func tree(for repo: RepoRef) async throws -> RepoTree {
        let token = self.token

        let response = try await cache.get(Self.treeURL(repo), options: CacheOptions(
            maxAge: Self.freshFor,
            configureRequest: { request in
                // GitHub rejects API requests without a User-Agent.
                request.setValue(HTTPSessions.userAgent, forHTTPHeaderField: "User-Agent")
                request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
                request.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
                if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
            },
            mapError: { Self.mapAPIError($0, repo: repo) }))

        return try Self.parseTree(response, repo: repo)
    }

    /// The cached file list for `repo` without touching the network, or nil.
    public func cachedTree(for repo: RepoRef) -> RepoTree? {
        cache.cachedResponse(for: Self.treeURL(repo)).flatMap { try? Self.parseTree($0, repo: repo) }
    }

    private static func treeURL(_ repo: RepoRef) -> URL {
        URL(string: "https://api.github.com/repos/\(repoPath(repo))/git/trees/\(escape(repo.effectiveRef))?recursive=1")!
    }

    /// "owner/name", each part escaped.
    private static func repoPath(_ repo: RepoRef) -> String {
        "\(escape(repo.owner))/\(escape(repo.name))"
    }

    private static func parseTree(_ response: CachedResponse, repo: RepoRef) throws -> RepoTree {
        guard let tree = try? JSONDecoder().decode(TreeResponse.self, from: response.body), let entries = tree.tree else {
            throw GitHubError.unreadableTree(repo: repo.fullName)
        }

        let prefix = repo.subPath.map { $0.hasSuffix("/") ? $0 : $0 + "/" }
        let items = entries.compactMap { entry -> RepoTreeItem? in
            guard var path = entry.path else { return nil }
            if let prefix {
                guard path.hasPrefix(prefix) else { return nil }
                path = String(path.dropFirst(prefix.count))
            }
            return RepoTreeItem(path: path, isFile: entry.type == "blob", size: entry.size)
        }

        return RepoTree(items: items, truncated: tree.truncated ?? false, isStale: response.isStale, staleReason: response.error)
    }

    /// raw.githubusercontent.com URL for a path relative to the theme root.
    public static func rawURL(_ repo: RepoRef, _ themeRelativePath: String) -> URL {
        let path = repo.repoPath(themeRelativePath).split(separator: "/", omittingEmptySubsequences: false)
            .map { escape(String($0)) }
            .joined(separator: "/")
        return URL(string: "https://raw.githubusercontent.com/\(repoPath(repo))/\(escape(repo.effectiveRef))/\(path)")!
    }

    public func rawText(_ repo: RepoRef, _ themeRelativePath: String) async throws -> String {
        let notFound = GitHubError.fileNotFound(path: themeRelativePath, repo: repo.fullName)
        let response = try await cache.get(Self.rawURL(repo, themeRelativePath), options: CacheOptions(
            maxAge: Self.freshFor,
            mapError: { $0.statusCode == 404 ? notFound : nil }))
        return response.text
    }

    static func mapAPIError(_ response: HTTPURLResponse, repo: RepoRef) -> any Error {
        let status = response.statusCode
        if status == 429 || (status == 403 && response.value(forHTTPHeaderField: "x-ratelimit-remaining") == "0") {
            let resetsAt = response.value(forHTTPHeaderField: "x-ratelimit-reset")
                .flatMap(TimeInterval.init)
                .map(Date.init(timeIntervalSince1970:))
            return GitHubError.rateLimited(resetsAt: resetsAt)
        }
        // 409 = empty repository.
        if status == 404 || status == 409 {
            return GitHubError.notFound(repo: repo.fullName)
        }
        return GitHubError.http(status: status, repo: repo.fullName)
    }

    /// Percent-encodes everything except RFC 3986 unreserved characters (like .NET's EscapeDataString).
    static func escape(_ s: String) -> String {
        s.addingPercentEncoding(withAllowedCharacters: unreserved) ?? s
    }

    private static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")

    private struct TreeResponse: Decodable {
        var tree: [Entry]?
        var truncated: Bool?

        struct Entry: Decodable {
            var path: String?
            var type: String?
            var size: Int64?
        }
    }
}
