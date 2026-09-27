import Foundation
import Testing
@testable import OmarchyThemesKit

struct CatalogParserTests {
    @Test func parsesLiveOmarchyOrgCardStructure() {
        let entries = CatalogParser.parse(Fixture.read("catalog-live-structure.html"))

        #expect(entries.map(\.slug) == ["aetheria", "amberbyte", "arc-blueberry", "all-hallows-eve"])

        let aetheria = entries[0]
        #expect(aetheria.name == "Aetheria")
        #expect(aetheria.repoURL.absoluteString == "https://github.com/JJDizz1L/aetheria")
        #expect(aetheria.screenshotURL?.absoluteString == "https://omarchy.org/assets/themes/aetheria.webp")
    }

    @Test func normalizesRepoURLsNamesAndRelativeScreenshots() {
        let entries = Dictionary(uniqueKeysWithValues: CatalogParser.parse(Fixture.read("catalog-live-structure.html")).map { ($0.slug, $0) })

        #expect(entries["amberbyte"]?.repoURL.absoluteString == "https://github.com/tahfizhabib/omarchy-amberbyte-theme")
        #expect(entries["arc-blueberry"]?.name == "Arc Blueberry")
        #expect(entries["arc-blueberry"]?.screenshotURL?.absoluteString == "https://omarchy.org/assets/themes/arc-blueberry.webp")
        #expect(entries["all-hallows-eve"]?.name == "All Hallow's Eve")
        #expect(entries["all-hallows-eve"]?.repoURL.absoluteString == "https://github.com/someone/monorepo/tree/main/themes/all-hallows-eve")
        #expect(entries["all-hallows-eve"]?.screenshotURL?.absoluteString == "https://cdn.example.com/shots/all-hallows-eve.png")
        #expect(entries["all-hallows-eve"]?.repoDisplay == "someone/monorepo")
    }

    @Test func skipsNavLinksNonGitHubLinksCardsWithoutScreenshotsAndDuplicates() {
        let entries = CatalogParser.parse(Fixture.read("catalog-live-structure.html"))

        #expect(!entries.contains { $0.repoURL.absoluteString.contains("basecamp/omarchy") })
        #expect(!entries.contains { $0.repoURL.absoluteString.contains("omarchy-site") })
        #expect(!entries.contains { $0.name == "Not GitHub" })
        #expect(!entries.contains { $0.name == "No Screenshot" })
        #expect(entries.filter { $0.repoURL.absoluteString.hasSuffix("/aetheria") }.count == 1)
    }

    @Test func supportsFigureLayout() throws {
        let entries = CatalogParser.parse(Fixture.read("catalog-figure.html"))
        try #require(entries.count == 3)

        #expect(entries[0].slug == "tokyo-night")
        #expect(entries[0].name == "Tokyo Night")
        #expect(entries[0].repoURL.absoluteString == "https://github.com/someone/omarchy-tokyo-night-theme")

        #expect(entries[1].slug == "rose-pine")
        #expect(entries[1].name == "Rosé Pine")

        // No screenshot URL: slug falls back to the repo name minus omarchy-/-theme.
        #expect(entries[2].slug == "no-src")
        #expect(entries[2].screenshotURL == nil)
    }

    @Test func makesSlugsUnique() {
        let html = """
            <ul>
              <li><a href="https://github.com/a/one"><img src="/assets/themes/same.webp"><span>One</span></a></li>
              <li><a href="https://github.com/b/two"><img src="/assets/themes/same.webp"><span>Two</span></a></li>
            </ul>
            """

        #expect(CatalogParser.parse(html).map(\.slug) == ["same", "same-2"])
    }

    @Test(arguments: [
        "",
        "<html><body><p>Maintenance</p></body></html>",
        "<ul><li><a href=\"https://github.com/x/y\"",
    ])
    func returnsEmptyForPagesWithoutThemeCards(html: String) {
        #expect(CatalogParser.parse(html).isEmpty)
    }
}

struct HTTPCacheTests {
    let url = URL(string: "https://example.com/data.json")!
    let dir = TempDir()
    let http = FakeTransport()
    let clock = ManualClock(Date(timeIntervalSince1970: 1_790_424_000)) // 2026-09-26 12:00 UTC
    let cache: HTTPCache

    init() {
        cache = HTTPCache(transport: http, directory: dir.url, now: clock.function)
    }

    @Test func servesFromDiskWithinMaxAgeWithoutNetwork() async throws {
        http.on(url.absoluteString, body: "v1", etag: "\"a\"")
        let options = CacheOptions(maxAge: 3600)

        let first = try await cache.get(url, options: options)
        clock.advance(by: 1800)
        let second = try await cache.get(url, options: options)

        #expect(!first.fromCache)
        #expect(second.fromCache)
        #expect(second.text == "v1")
        #expect(http.count(for: url.absoluteString) == 1)
    }

    @Test func revalidatesWithETagAndUsesCachedBodyOn304() async throws {
        http.on(url.absoluteString, body: "v1", etag: "\"a\"")
        _ = try await cache.get(url)

        http.on(url.absoluteString) { request in
            #expect(request.value(forHTTPHeaderField: "If-None-Match") == "\"a\"")
            return FakeTransport.status(304, for: request)
        }
        clock.advance(by: 300)
        let revalidated = try await cache.get(url)

        #expect(revalidated.fromCache)
        #expect(!revalidated.isStale)
        #expect(revalidated.text == "v1")
        #expect(revalidated.fetchedAt == clock.now)
        #expect(cache.cachedResponse(for: url)?.fetchedAt == clock.now)
    }

    @Test func replacesCacheWhenContentChanges() async throws {
        http.on(url.absoluteString, body: "v1", etag: "\"a\"")
        _ = try await cache.get(url)
        http.on(url.absoluteString, body: "v2", etag: "\"b\"")

        let updated = try await cache.get(url)

        #expect(updated.text == "v2")
        #expect(cache.cachedResponse(for: url)?.text == "v2")
    }

    @Test func returnsStaleCopyWhenOffline() async throws {
        http.on(url.absoluteString, body: "v1")
        _ = try await cache.get(url)
        http.on(url.absoluteString) { _ in try FakeTransport.offline() }

        let stale = try await cache.get(url)

        #expect(stale.isStale)
        #expect(stale.text == "v1")
        #expect(stale.error is URLError)
    }

    @Test func throwsWhenOfflineWithNothingCached() async {
        http.on(url.absoluteString) { _ in try FakeTransport.offline() }
        await #expect(throws: URLError.self) { try await cache.get(url) }
    }

    @Test func mapsServerErrorsThroughOptions() async {
        http.on(url.absoluteString) { FakeTransport.status(500, for: $0) }
        let options = CacheOptions(mapError: { _ in FakeFailure(message: "mapped") })

        await #expect(throws: FakeFailure.self) { try await cache.get(url, options: options) }
    }

    @Test func unmappedServerErrorsDescribeTheStatus() async throws {
        http.on(url.absoluteString) { FakeTransport.status(500, for: $0) }

        let error = await #expect(throws: HTTPStatusError.self) { try await cache.get(url) }
        #expect(error?.localizedDescription == "example.com returned 500 Internal server error.")
    }

    @Test func stripsByteOrderMark() async throws {
        http.on(url.absoluteString, body: "\u{FEFF}hello")
        #expect(try await cache.get(url).text == "hello")
    }

    @Test func catalogServiceLoadsCacheOfflineAndRejectsPagesWithoutThemes() async throws {
        let page = CatalogParser.defaultPageURL.absoluteString
        let catalog = CatalogService(cache: cache)
        #expect(catalog.loadCached() == nil)

        http.on(page, body: Fixture.read("catalog-live-structure.html"))
        let fresh = try await catalog.refresh()
        #expect(fresh.entries.count == 4)
        #expect(!fresh.isStale)
        #expect(catalog.loadCached()?.entries.count == 4)

        http.on(page) { _ in try FakeTransport.offline() }
        let offline = try await catalog.refresh()
        #expect(offline.isStale)
        #expect(offline.entries.count == 4)

        http.on(page, body: "<html><body>Under maintenance</body></html>")
        await #expect(throws: CatalogFormatError.self) { try await catalog.refresh() }
    }
}
