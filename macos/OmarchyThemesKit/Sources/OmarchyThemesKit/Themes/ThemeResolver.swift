import Foundation

/// A wallpaper image in a theme repo.
public struct WallpaperRef: Hashable, Sendable {
    /// Path relative to the theme root, e.g. "backgrounds/1.png".
    public var path: String
    public var size: Int64?
    public var downloadURL: URL

    public init(path: String, size: Int64?, downloadURL: URL) {
        self.path = path
        self.size = size
        self.downloadURL = downloadURL
    }

    public var fileName: String { path.split(separator: "/").last.map(String.init) ?? path }
}

/// Everything the app needs from a theme's repo to preview, download and apply it.
public struct ThemeDetails: Sendable {
    public var entry: CatalogEntry
    public var repo: RepoRef
    /// Nil when no supported palette file could be read; see `paletteError`.
    public var palette: Palette?
    public var paletteError: String?
    public var wallpapers: [WallpaperRef]
    public var mode: AppearanceMode
    /// Served from cache because GitHub was unreachable or rate-limited.
    public var isStale: Bool
    public var staleReason: (any Error)?

    public init(
        entry: CatalogEntry, repo: RepoRef, palette: Palette?, paletteError: String?, wallpapers: [WallpaperRef],
        mode: AppearanceMode, isStale: Bool = false, staleReason: (any Error)? = nil
    ) {
        self.entry = entry
        self.repo = repo
        self.palette = palette
        self.paletteError = paletteError
        self.wallpapers = wallpapers
        self.mode = mode
        self.isStale = isStale
        self.staleReason = staleReason
    }

    public var canApply: Bool { !wallpapers.isEmpty || palette != nil }
}

public struct ThemeResolveError: LocalizedError, Sendable {
    public let message: String
    public var errorDescription: String? { message }
}

/// Resolves a catalog entry to its palette and wallpapers. Called only when the user opens a
/// theme, and costs one GitHub API call (usually a free 304 on revisits).
public struct ThemeResolver: Sendable {
    /// Palette files in priority order.
    private static let paletteFiles: [(file: String, parse: @Sendable (String) throws -> Palette)] = [
        ("colors.toml", ColorsTomlParser.parse),
        ("alacritty.toml", AlacrittyParser.parse),
    ]

    private static let wallpaperDirs = ["backgrounds/", "wallpapers/"]
    private static let imageExtensions: Set<String> = ["png", "jpg", "jpeg", "webp", "bmp"]

    private let github: GitHubClient

    public init(github: GitHubClient) {
        self.github = github
    }

    public func resolve(_ entry: CatalogEntry) async throws -> ThemeDetails {
        guard let repo = RepoRef(parsing: entry.repoURL.absoluteString) else {
            throw ThemeResolveError(message: "\(entry.name) doesn't link to a GitHub repository, so it can't be read.")
        }

        let tree = try await github.tree(for: repo)

        var (palette, paletteError) = try await readPalette(repo: repo, tree: tree)
        let wallpapers = Self.findWallpapers(in: tree)
            .map { WallpaperRef(path: $0.path, size: $0.size, downloadURL: GitHubClient.rawURL(repo, $0.path)) }

        let hasLightModeFile = tree.findFile("light.mode") != nil
        if palette?.declaredMode == nil, hasLightModeFile {
            palette?.declaredMode = .light
        }
        let mode = palette?.mode ?? (hasLightModeFile ? .light : .dark)

        return ThemeDetails(entry: entry, repo: repo, palette: palette, paletteError: paletteError, wallpapers: wallpapers,
                            mode: mode, isStale: tree.isStale, staleReason: tree.staleReason)
    }

    private func readPalette(repo: RepoRef, tree: RepoTree) async throws -> (Palette?, String?) {
        var problems: [String] = []
        for (file, parse) in Self.paletteFiles {
            guard let item = tree.findFile(file) else { continue }
            let text: String
            do {
                text = try await github.rawText(repo, item.path)
            } catch {
                if Task.isCancelled { throw CancellationError() }
                problems.append("\(file) couldn't be downloaded: \(error.localizedDescription)")
                continue
            }
            do {
                return (try parse(text), nil)
            } catch {
                problems.append(error.localizedDescription)
            }
        }

        return problems.isEmpty
            ? (nil, "Couldn't read this theme's palette: it has no colors.toml or alacritty.toml.")
            : (nil, "Couldn't read this theme's palette. " + problems.joined(separator: " "))
    }

    static func findWallpapers(in tree: RepoTree) -> [RepoTreeItem] {
        let images = tree.files.filter { item in
            let ext = (item.path as NSString).pathExtension.lowercased()
            return imageExtensions.contains(ext)
        }

        for dir in wallpaperDirs {
            let inDir = images.filter { $0.path.lowercased().hasPrefix(dir) }
            if !inDir.isEmpty {
                return inDir.sorted { NaturalSort.isOrderedBefore($0.path, $1.path) }
            }
        }

        // Some repos keep a single background at the root.
        return images
            .filter(isRootWallpaper)
            .sorted { NaturalSort.isOrderedBefore($0.path, $1.path) }
    }

    private static func isRootWallpaper(_ item: RepoTreeItem) -> Bool {
        let name = item.fileName.lowercased()
        return !item.path.contains("/") && (name.hasPrefix("background") || name.hasPrefix("wallpaper"))
    }
}

/// Orders "2.png" before "10.png".
public enum NaturalSort {
    public static func isOrderedBefore(_ x: String, _ y: String) -> Bool {
        compare(x, y) < 0
    }

    static func compare(_ x: String, _ y: String) -> Int {
        let a = Array(x.unicodeScalars)
        let b = Array(y.unicodeScalars)
        var i = 0
        var j = 0
        while i < a.count && j < b.count {
            if isDigit(a[i]) && isDigit(b[j]) {
                let si = i
                while i < a.count && isDigit(a[i]) { i += 1 }
                let sj = j
                while j < b.count && isDigit(b[j]) { j += 1 }
                let na = String(String.UnicodeScalarView(a[si..<i])).drop { $0 == "0" }
                let nb = String(String.UnicodeScalarView(b[sj..<j])).drop { $0 == "0" }
                if na.count != nb.count { return na.count < nb.count ? -1 : 1 }
                if na != nb { return na < nb ? -1 : 1 }
            } else {
                let ca = lower(a[i])
                let cb = lower(b[j])
                if ca != cb { return ca < cb ? -1 : 1 }
                i += 1
                j += 1
            }
        }
        let restA = a.count - i
        let restB = b.count - j
        return restA == restB ? 0 : (restA < restB ? -1 : 1)
    }

    private static func isDigit(_ s: Unicode.Scalar) -> Bool { s >= "0" && s <= "9" }

    private static func lower(_ s: Unicode.Scalar) -> UInt32 {
        s >= "A" && s <= "Z" ? s.value + 32 : s.value
    }
}
