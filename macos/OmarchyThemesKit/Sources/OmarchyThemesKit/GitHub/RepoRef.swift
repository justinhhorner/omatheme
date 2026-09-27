import Foundation

/// A GitHub repository, optionally pinned to a branch/tag and a sub-folder
/// (from links like https://github.com/owner/repo/tree/main/themes/foo).
public struct RepoRef: Hashable, Sendable {
    public let owner: String
    public let name: String
    public let ref: String?
    public let subPath: String?

    public init(owner: String, name: String, ref: String? = nil, subPath: String? = nil) {
        self.owner = owner
        self.name = name
        self.ref = ref
        self.subPath = subPath
    }

    private static let reservedOwners: Set<String> = [
        "orgs", "settings", "sponsors", "topics", "marketplace", "explore", "features", "login", "search", "about",
    ]

    public var fullName: String { "\(owner)/\(name)" }

    /// Git ref used for API and raw URLs; HEAD resolves to the default branch.
    public var effectiveRef: String { ref ?? "HEAD" }

    public var htmlURL: URL {
        var path = "/\(owner)/\(name)"
        if ref != nil || subPath != nil {
            path += "/tree/\(effectiveRef)"
            if let subPath { path += "/\(subPath)" }
        }
        var components = URLComponents()
        components.scheme = "https"
        components.host = "github.com"
        components.path = path
        return components.url!
    }

    /// Maps a path relative to the theme root to a path relative to the repo root.
    public func repoPath(_ themeRelativePath: String) -> String {
        subPath.map { "\($0)/\(themeRelativePath)" } ?? themeRelativePath
    }

    /// Parses a github.com repo link; nil for anything else (profiles, /compare, /issues, other hosts).
    public init?(parsing url: String?) {
        guard let text = url?.trimmingCharacters(in: .whitespacesAndNewlines),
              let components = URLComponents(string: text),
              let scheme = components.scheme?.lowercased(), scheme == "https" || scheme == "http",
              let host = components.host?.lowercased(), host == "github.com" || host == "www.github.com"
        else { return nil }

        // percentEncodedPath keeps "." and ".." segments, so traversal attempts can be rejected.
        let segments = components.percentEncodedPath
            .split(separator: "/", omittingEmptySubsequences: true)
            .map { $0.removingPercentEncoding ?? String($0) }
        guard segments.count >= 2 else { return nil }

        let owner = segments[0]
        let name = segments[1].lowercased().hasSuffix(".git") ? String(segments[1].dropLast(4)) : segments[1]
        guard Self.isValidName(owner), Self.isValidName(name), !Self.reservedOwners.contains(owner.lowercased()) else {
            return nil
        }

        if segments.count == 2 {
            self.init(owner: owner, name: name)
            return
        }

        // Only /tree/<ref>[/<sub/path>] is a repo view we understand; /compare, /issues etc. are not themes.
        guard segments.count >= 4, segments[2] == "tree" else { return nil }
        let rest = segments.dropFirst(4)
        if rest.contains(where: { $0 == "." || $0 == ".." }) { return nil }
        self.init(owner: owner, name: name, ref: segments[3], subPath: rest.isEmpty ? nil : rest.joined(separator: "/"))
    }

    private static func isValidName(_ s: String) -> Bool {
        (1...100).contains(s.count) && s != "." && s != ".."
            && s.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" || $0 == ".") }
    }
}
