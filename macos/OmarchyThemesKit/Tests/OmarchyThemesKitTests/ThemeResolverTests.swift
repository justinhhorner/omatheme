import Foundation
import Testing
@testable import OmarchyThemesKit
import OmarchyThemesTestSupport

struct ThemeResolverTests {
    static let treeURL = "https://api.github.com/repos/o/r/git/trees/HEAD?recursive=1"
    static let raw = "https://raw.githubusercontent.com/o/r/HEAD/"
    static let entry = CatalogEntry(slug: "r", name: "My Theme", repoURL: URL(string: "https://github.com/o/r")!,
                                    screenshotURL: URL(string: "https://omarchy.org/assets/themes/r.webp"))

    let dir = TempDir()
    let http = FakeTransport()
    let clock = ManualClock(Date(timeIntervalSince1970: 0))

    func resolver(token: String? = nil) -> ThemeResolver {
        ThemeResolver(github: GitHubClient(cache: HTTPCache(transport: http, directory: dir.url, now: clock.function), token: token))
    }

    static func tree(_ items: (path: String, type: String, size: Int?)...) -> String {
        let json: [String: Any] = [
            "sha": "abc",
            "truncated": false,
            "tree": items.map { item -> [String: Any] in
                var entry: [String: Any] = ["path": item.path, "type": item.type]
                if let size = item.size { entry["size"] = size }
                return entry
            },
        ]
        return String(decoding: try! JSONSerialization.data(withJSONObject: json), as: UTF8.self)
    }

    @Test func resolvesPaletteAndNaturallySortedWallpapers() async throws {
        http.on(Self.treeURL, body: Self.tree(
                ("colors.toml", "blob", 500),
                ("alacritty.toml", "blob", 500),
                ("backgrounds", "tree", nil),
                ("backgrounds/10-night.png", "blob", 3000),
                ("backgrounds/2-dusk.jpg", "blob", 2000),
                ("backgrounds/1-day.webp", "blob", 1000),
                ("backgrounds/notes.txt", "blob", 10),
                ("preview.png", "blob", 999)))
            .on(Self.raw + "colors.toml", body: Fixture.read("colors-ansi.toml"))

        let details = try await resolver().resolve(Self.entry)

        let palette = try #require(details.palette)
        #expect(details.paletteError == nil)
        #expect(palette.source == .colorsToml)
        #expect(details.mode == .dark)
        #expect(details.wallpapers.map(\.fileName) == ["1-day.webp", "2-dusk.jpg", "10-night.png"])
        #expect(details.wallpapers[0].downloadURL.absoluteString == Self.raw + "backgrounds/1-day.webp")
        #expect(details.wallpapers[0].size == 1000)
        #expect(details.canApply)
        #expect(http.count(for: Self.raw + "alacritty.toml") == 0)
    }

    @Test func fallsBackToAlacrittyWhenColorsTomlIsUnusable() async throws {
        http.on(Self.treeURL, body: Self.tree(("colors.toml", "blob", 5), ("alacritty.toml", "blob", 5), ("backgrounds/a.png", "blob", 5)))
            .on(Self.raw + "colors.toml", body: "# empty")
            .on(Self.raw + "alacritty.toml", body: Fixture.read("alacritty.toml"))

        let details = try await resolver().resolve(Self.entry)

        #expect(details.palette?.source == .alacritty)
    }

    @Test func reportsAClearPaletteErrorButKeepsWallpapers() async throws {
        http.on(Self.treeURL, body: Self.tree(("README.md", "blob", 5), ("backgrounds/a.png", "blob", 5)))

        let details = try await resolver().resolve(Self.entry)

        #expect(details.palette == nil)
        #expect(details.paletteError?.hasPrefix("Couldn't read this theme's palette") == true)
        #expect(details.wallpapers.count == 1)
        #expect(details.canApply)
    }

    @Test func paletteDownloadFailureIsReported() async throws {
        http.on(Self.treeURL, body: Self.tree(("colors.toml", "blob", 5)))
            .on(Self.raw + "colors.toml") { _ in try FakeTransport.offline() }

        let details = try await resolver().resolve(Self.entry)

        #expect(details.palette == nil)
        #expect(details.paletteError?.contains("colors.toml couldn't be downloaded") == true)
    }

    @Test func themeWithNothingUsableCannotBeApplied() async throws {
        http.on(Self.treeURL, body: Self.tree(("README.md", "blob", 5)))

        let details = try await resolver().resolve(Self.entry)

        #expect(!details.canApply)
    }

    @Test func lightModeFileMarksThemeLight() async throws {
        http.on(Self.treeURL, body: Self.tree(("colors.toml", "blob", 5), ("light.mode", "blob", 0)))
            .on(Self.raw + "colors.toml", body: Fixture.read("colors-ansi.toml")) // dark background

        let details = try await resolver().resolve(Self.entry)

        #expect(details.mode == .light)
        #expect(details.palette?.declaredMode == .light)
    }

    @Test func usesRootBackgroundImageWhenThereIsNoBackgroundsFolder() async throws {
        http.on(Self.treeURL, body: Self.tree(("background.jpg", "blob", 5), ("preview.png", "blob", 5), ("assets/wallpaper.png", "blob", 5)))

        let details = try await resolver().resolve(Self.entry)

        #expect(details.wallpapers.map(\.path) == ["background.jpg"])
    }

    @Test func resolvesThemesInsideARepoSubFolder() async throws {
        var entry = Self.entry
        entry = CatalogEntry(slug: entry.slug, name: entry.name, repoURL: URL(string: "https://github.com/o/mono/tree/main/themes/foo")!,
                             screenshotURL: entry.screenshotURL)
        http.on("https://api.github.com/repos/o/mono/git/trees/main?recursive=1", body: Self.tree(
                ("themes/foo/colors.toml", "blob", 5),
                ("themes/foo/backgrounds/1.png", "blob", 5),
                ("themes/bar/backgrounds/1.png", "blob", 5)))
            .on("https://raw.githubusercontent.com/o/mono/main/themes/foo/colors.toml", body: Fixture.read("colors-ansi.toml"))

        let details = try await resolver().resolve(entry)

        #expect(details.palette != nil)
        let expected = ["https://raw.githubusercontent.com/o/mono/main/themes/foo/backgrounds/1.png"]
        #expect(details.wallpapers.map(\.downloadURL.absoluteString) == expected)
    }

    @Test func rateLimitSurfacesResetTime() async throws {
        http.on(Self.treeURL) { FakeTransport.status(403, for: $0, headers: ["x-ratelimit-remaining": "0", "x-ratelimit-reset": "1790482111"]) }

        let error = await #expect(throws: GitHubError.self) { try await resolver().resolve(Self.entry) }
        guard case .rateLimited(let resetsAt) = error else {
            Issue.record("Expected rateLimited, got \(String(describing: error))")
            return
        }
        #expect(resetsAt == Date(timeIntervalSince1970: 1_790_482_111))
        #expect(error?.localizedDescription.hasPrefix("GitHub's rate limit was reached. It resets at ") == true)
    }

    @Test func rateLimitedRevisitServesCachedTreeAsStale() async throws {
        http.on(Self.treeURL, body: Self.tree(("backgrounds/a.png", "blob", 5)), etag: "\"t\"")
        _ = try await resolver().resolve(Self.entry)
        clock.advance(by: 2 * 3600)
        http.on(Self.treeURL) { FakeTransport.status(429, for: $0) }

        let details = try await resolver().resolve(Self.entry)

        #expect(details.isStale)
        guard case .rateLimited = details.staleReason as? GitHubError else {
            Issue.record("Expected a rate-limit stale reason, got \(String(describing: details.staleReason))")
            return
        }
        #expect(details.wallpapers.count == 1)
    }

    @Test func revisitWithinAnHourMakesNoRequest() async throws {
        http.on(Self.treeURL, body: Self.tree(("backgrounds/a.png", "blob", 5)), etag: "\"t\"")
        _ = try await resolver().resolve(Self.entry)
        clock.advance(by: 30 * 60)

        _ = try await resolver().resolve(Self.entry)

        #expect(http.count(for: Self.treeURL) == 1)
    }

    @Test func missingRepoIsReportedAsNotFound() async throws {
        http.on(Self.treeURL) { FakeTransport.status(404, for: $0) }

        let error = await #expect(throws: GitHubError.self) { try await resolver().resolve(Self.entry) }
        guard case .notFound(let repo) = error else {
            Issue.record("Expected notFound, got \(String(describing: error))")
            return
        }
        #expect(repo == "o/r")
    }

    @Test func nonGitHubEntriesAreRejected() async {
        let entry = CatalogEntry(slug: "r", name: "R", repoURL: URL(string: "https://gitlab.com/o/r")!, screenshotURL: nil)
        await #expect(throws: ThemeResolveError.self) { try await resolver().resolve(entry) }
    }

    @Test func tokenIsSentToTheAPIOnly() async throws {
        http.on(Self.treeURL, body: Self.tree(("colors.toml", "blob", 5)))
            .on(Self.raw + "colors.toml", body: Fixture.read("colors-ansi.toml"))

        _ = try await resolver(token: " secret ").resolve(Self.entry)

        let api = try #require(http.requests.first { $0.url?.host() == "api.github.com" })
        let raw = try #require(http.requests.first { $0.url?.host() == "raw.githubusercontent.com" })
        #expect(api.value(forHTTPHeaderField: "Authorization") == "Bearer secret")
        #expect(api.value(forHTTPHeaderField: "User-Agent")?.isEmpty == false)
        #expect(raw.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test func naturalSortOrdersNumbersByValue() {
        let input = ["b10.png", "b2.png", "a.png", "b1.png", "B3.png", "b02.png"]
        #expect(input.sorted(by: NaturalSort.isOrderedBefore) == ["a.png", "b1.png", "b2.png", "b02.png", "B3.png", "b10.png"])
    }
}
