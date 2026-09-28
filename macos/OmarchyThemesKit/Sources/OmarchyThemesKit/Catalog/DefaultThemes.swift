import Foundation

/// The themes that ship with Omarchy, in the themes/ folder of its repo. They aren't listed on
/// omarchy.org/themes, so they're discovered from the repo's file tree: the same cached API call
/// that resolving any one of them uses, so opening a default theme costs no extra request.
public enum DefaultThemes {
    /// Omarchy's repo. Its default branch (HEAD) is the current release line.
    public static let repo = RepoRef(owner: "omacom", name: "omarchy")

    /// Prefixes default-theme slugs. Community slugs never contain a ".", so the two can't
    /// collide, and a downloaded default theme keeps its folder name.
    public static let slugPrefix = "omarchy."

    private static let folder = "themes/"

    public static func isDefault(_ slug: String) -> Bool {
        slug.hasPrefix(slugPrefix)
    }

    /// One entry per folder directly under themes/, sorted by name.
    public static func entries(from tree: RepoTree) -> [CatalogEntry] {
        let files = Set(tree.files.map { $0.path.lowercased() })

        return tree.items
            .filter { !$0.isFile && $0.path.hasPrefix(folder) && !$0.path.dropFirst(folder.count).contains("/") }
            .map { String($0.path.dropFirst(folder.count)) }
            .filter { AppPaths.isValidSlug(slugPrefix + $0) }
            .sorted()
            .map { name in
                let theme = RepoRef(owner: repo.owner, name: repo.name, ref: "HEAD", subPath: folder + name)
                let preview = files.contains("\(folder)\(name)/preview.png".lowercased())
                    ? GitHubClient.rawURL(theme, "preview.png")
                    : nil
                return CatalogEntry(slug: slugPrefix + name, name: displayName(name), repoURL: theme.htmlURL, screenshotURL: preview)
            }
    }

    /// Omarchy's own naming (omarchy-theme-list): "retro-82" → "Retro 82".
    public static func displayName(_ folder: String) -> String {
        folder.split(separator: "-").map(\.uppercasingFirst).joined(separator: " ")
    }
}
