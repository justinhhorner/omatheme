import Foundation
import Testing
@testable import OmarchyThemesKit

struct DefaultThemesTests {
    static let treeURL = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1"
    static let raw = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/"
    static let pageURL = CatalogParser.defaultPageURL.absoluteString

    let dir = TempDir()
    let http = FakeTransport()
    let clock = ManualClock(Date(timeIntervalSince1970: 1_790_467_200)) // 2026-09-27 UTC
    let cache: HTTPCache
    let github: GitHubClient

    init() {
        cache = HTTPCache(transport: http, directory: dir.url, now: clock.function)
        github = GitHubClient(cache: cache)
    }

    func catalog() -> CatalogService {
        CatalogService(cache: cache, github: github)
    }

    @Test func listsEachFolderUnderThemesWithOmarchysNaming() async throws {
        http.on(Self.treeURL, body: Fixture.read("omarchy-tree.json"))

        let entries = DefaultThemes.entries(from: try await github.tree(for: DefaultThemes.repo))

        #expect(entries.map(\.slug) == ["omarchy.catppuccin-latte", "omarchy.retro-82", "omarchy.tokyo-night"])
        #expect(entries.map(\.name) == ["Catppuccin Latte", "Retro 82", "Tokyo Night"])

        let tokyo = entries[2]
        #expect(tokyo.repoURL.absoluteString == "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night")
        #expect(tokyo.screenshotURL?.absoluteString == Self.raw + "tokyo-night/preview.png")
        #expect(tokyo.isDefaultTheme)
        #expect(tokyo.repoDisplay == "Included with Omarchy")

        // No preview.png in the folder: no screenshot rather than a broken link.
        #expect(entries[1].screenshotURL == nil)
    }

    @Test(arguments: [("tokyo-night", "Tokyo Night"), ("retro-82", "Retro 82"), ("white", "White"), ("flexoki-light", "Flexoki Light")])
    func displayNamesFollowOmarchyThemeList(folder: String, name: String) {
        #expect(DefaultThemes.displayName(folder) == name)
    }

    @Test func communityEntriesAreNotDefaultThemes() {
        let entry = CatalogEntry(slug: "aetheria", name: "Aetheria", repoURL: URL(string: "https://github.com/JJDizz1L/aetheria")!, screenshotURL: nil)

        #expect(!entry.isDefaultTheme)
        #expect(entry.repoDisplay == "JJDizz1L/aetheria")
    }

    @Test func catalogListsDefaultThemesFirstThenTheCommunityGallery() async throws {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Self.treeURL, body: Fixture.read("omarchy-tree.json"))

        let catalog = try await catalog().refresh()

        #expect(catalog.entries.count == 7)
        let defaultsFirst = catalog.entries.prefix(3).allSatisfy(\.isDefaultTheme)
        #expect(defaultsFirst)
        #expect(catalog.entries[3].slug == "aetheria")
        #expect(catalog.defaultThemesError == nil)
        #expect(Set(catalog.entries.map(\.slug)).count == catalog.entries.count)
    }

    @Test func catalogStillLoadsWhenGitHubFailsAndNothingIsCached() async throws {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Self.treeURL) { FakeTransport.status(403, for: $0, headers: ["x-ratelimit-remaining": "0"]) }

        let catalog = try await catalog().refresh()

        #expect(catalog.entries.count == 4)
        guard case .rateLimited = catalog.defaultThemesError as? GitHubError else {
            Issue.record("Expected a rate-limit error, got \(String(describing: catalog.defaultThemesError))")
            return
        }
        #expect(!catalog.isStale)
    }

    @Test func defaultThemesLoadFromCacheOffline() async throws {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Self.treeURL, body: Fixture.read("omarchy-tree.json"), etag: "\"tree\"")
        _ = try await catalog().refresh()

        #expect(catalog().loadCached()?.entries.count == 7)

        clock.advance(by: 2 * 3600)
        http.on(Self.pageURL) { _ in try FakeTransport.offline() }
            .on(Self.treeURL) { _ in try FakeTransport.offline() }
        let offline = try await catalog().refresh()

        #expect(offline.isStale)
        #expect(offline.entries.count == 7)
        #expect(offline.defaultThemesError is URLError)
    }

    @Test func catalogWithoutAGitHubClientIsCommunityOnly() async throws {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))

        let catalog = try await CatalogService(cache: cache).refresh()

        #expect(catalog.entries.count == 4)
        #expect(http.count(for: Self.treeURL) == 0)
    }

    @Test func resolvingADefaultThemeReusesTheCachedTree() async throws {
        http.on(Self.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Self.treeURL, body: Fixture.read("omarchy-tree.json"))
            .on(Self.raw + "tokyo-night/colors.toml", body: Fixture.read("colors-named.toml"))
        let tokyo = try #require(try await catalog().refresh().entries.first { $0.slug == "omarchy.tokyo-night" })

        let details = try await ThemeResolver(github: github).resolve(tokyo)

        #expect(http.count(for: Self.treeURL) == 1)
        #expect(details.palette?.source == .colorsToml)
        #expect(details.mode == .light)
        // backgrounds/ only: not preview.png or unlock.png, naturally sorted.
        #expect(details.wallpapers.map(\.fileName) == ["0-winding-road.webp", "2-swirl-buck.webp", "10-oma.webp"])
        #expect(details.wallpapers.first?.downloadURL.absoluteString == Self.raw + "tokyo-night/backgrounds/0-winding-road.webp")
    }

    @Test func defaultThemeSlugsAreValidFolderNames() throws {
        #expect(try dir.paths.themeDir("omarchy.tokyo-night").lastPathComponent == "omarchy.tokyo-night")
    }
}
