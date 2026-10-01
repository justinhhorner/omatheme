import Foundation
import OmarchyThemesKit
import OmarchyThemesTestSupport
import Testing
@testable import OmarchyThemesStores

@MainActor
struct PreferencesTests {
    let harness = Harness()

    @Test func welcomeShowsUntilDismissedAndStaysDismissed() {
        let stores = harness.makeStores()
        #expect(stores.preferences.showWelcome)

        stores.preferences.dismissWelcome()

        #expect(!stores.preferences.showWelcome)
        #expect(!harness.makeStores().preferences.showWelcome)
    }

    @Test func updatesAreSaved() {
        harness.makeStores().preferences.update { $0.applyDefaults.fit = .center }
        #expect(harness.makeStores().preferences.settings.applyDefaults.fit == .center)
    }
}

@MainActor
struct CatalogStoreTests {
    let harness = Harness()

    @Test func firstLoadFetchesAndListsDefaultThemesFirst() async {
        harness.serveCatalog()
        let catalog = harness.makeStores().catalog

        await catalog.loadIfNeeded()

        #expect(catalog.entries.count == 7)
        #expect(catalog.entries.first?.isDefaultTheme == true)
        #expect(catalog.fetchedAt == harness.clock.now)
        #expect(!catalog.isLoading && !catalog.isRefreshing)
        #expect(catalog.error == nil && catalog.notice == nil && catalog.defaultThemesNotice == nil)
    }

    @Test func aFreshCachedCatalogLoadsWithoutTheNetwork() async {
        harness.serveCatalog()
        await harness.makeStores().catalog.loadIfNeeded()
        let requests = harness.http.requests.count
        harness.http.on(Harness.pageURL) { _ in try FakeTransport.offline() }

        let catalog = harness.makeStores().catalog
        await catalog.loadIfNeeded()

        #expect(catalog.entries.count == 7)
        #expect(harness.http.requests.count == requests)
    }

    @Test func anOldCachedCatalogIsRefreshed() async {
        harness.serveCatalog()
        await harness.makeStores().catalog.loadIfNeeded()
        harness.clock.advance(by: CatalogStore.autoRefreshAfter + 60)

        await harness.makeStores().catalog.loadIfNeeded()

        #expect(harness.http.count(for: Harness.pageURL) == 2)
    }

    @Test func concurrentRefreshesShareOneRequest() async {
        harness.serveCatalog()
        let catalog = harness.makeStores().catalog

        async let first: Void = catalog.refresh()
        async let second: Void = catalog.refresh()
        _ = await (first, second)

        #expect(harness.http.count(for: Harness.pageURL) == 1)
    }

    @Test func offlineFirstLaunchShowsAnError() async {
        harness.http.on(Harness.pageURL) { _ in try FakeTransport.offline() }
        let catalog = harness.makeStores().catalog

        await catalog.loadIfNeeded()

        #expect(catalog.entries.isEmpty)
        #expect(catalog.error?.hasPrefix("Couldn't load themes from omarchy.org") == true)
    }

    @Test func goingOfflineLaterKeepsTheCatalogWithANotice() async {
        harness.serveCatalog()
        let catalog = harness.makeStores().catalog
        await catalog.refresh()
        harness.http.on(Harness.pageURL) { _ in try FakeTransport.offline() }

        await catalog.refresh()

        #expect(catalog.entries.count == 7)
        #expect(catalog.error == nil)
        #expect(catalog.notice?.hasPrefix("Couldn't reach omarchy.org") == true)
    }

    @Test func missingDefaultThemesGetTheirOwnNotice() async {
        harness.http.on(Harness.pageURL, body: Fixture.read("catalog-live-structure.html"))
            .on(Harness.treeURL) { FakeTransport.status(403, for: $0, headers: ["x-ratelimit-remaining": "0"]) }
        let catalog = harness.makeStores().catalog

        await catalog.refresh()

        #expect(catalog.entries.count == 4)
        #expect(catalog.defaultThemesNotice?.contains("rate limit") == true)
    }

    @Test func entryForADownloadedThemeFallsBackToItsManifest() {
        let catalog = harness.makeStores().catalog
        let theme = InstalledTheme(slug: "gone", name: "Gone", repoURL: URL(string: "https://github.com/o/gone")!)

        #expect(catalog.entry(for: theme) == CatalogEntry(slug: "gone", name: "Gone", repoURL: theme.repoURL, screenshotURL: nil))
    }
}

@MainActor
struct ThemeLibraryTests {
    let harness = Harness()

    @Test func downloadInstallsAndListsTheTheme() async throws {
        let stores = harness.makeStores()

        let theme = try await harness.installTokyo(in: stores)

        #expect(theme.wallpapers == ["0-winding-road.webp", "2-swirl-buck.webp", "10-oma.webp"])
        #expect(stores.library.installed.map(\.slug) == ["omarchy.tokyo-night"])
        #expect(stores.library.downloads.isEmpty)
        #expect(stores.banners[.theme(theme.slug)] == nil)
        #expect(stores.library.themeName(for: Harness.tokyoEntry) == "Tokyo Night")
    }

    @Test func concurrentDownloadsOfATheSameThemeShareOne() async {
        harness.serveCatalog()
        let library = harness.makeStores().library

        async let first = library.download(Harness.tokyoEntry)
        async let second = library.download(Harness.tokyoEntry)
        let (a, b) = await (first, second)

        #expect(a != nil && a == b)
        // 3 wallpapers and the screenshot, once.
        #expect(harness.downloader.downloaded.count == 4)
    }

    @Test func aFailedDownloadSaysWhyAndSavesNothing() async {
        harness.serveCatalog()
        harness.downloader.fail(URL(string: Harness.raw + "tokyo-night/backgrounds/2-swirl-buck.webp")!)
        let stores = harness.makeStores()

        let theme = await stores.library.download(Harness.tokyoEntry)

        #expect(theme == nil)
        #expect(stores.library.installed.isEmpty)
        #expect(stores.banners[.theme(Harness.tokyoEntry.slug)]?.title == "Download failed")
    }

    @Test func aCancelledDownloadSavesNothing() async {
        harness.serveCatalog()
        let stores = harness.makeStores()

        let download = Task { await stores.library.download(Harness.tokyoEntry) }
        await Task.yield() // the download starts, then waits for its own task
        stores.library.cancelDownload(Harness.tokyoEntry.slug)

        #expect(await download.value == nil)
        #expect(stores.library.installed.isEmpty)
        #expect(stores.banners[.theme(Harness.tokyoEntry.slug)]?.title == "Download cancelled")
    }

    @Test func lookupsAreMemoizedUntilTheCacheIsCleared() async throws {
        harness.serveCatalog()
        let stores = harness.makeStores()

        _ = try await stores.library.details(for: Harness.tokyoEntry)
        #expect(stores.library.cachedDetails(Harness.tokyoEntry.slug) != nil)

        try stores.clearCache()

        #expect(stores.library.cachedDetails(Harness.tokyoEntry.slug) == nil)
        #expect((try? FileManager.default.contentsOfDirectory(atPath: harness.dir.paths.cacheDir.path)) == [])
    }

    @Test func removeReportsItsOutcome() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)

        let banner = stores.library.remove(theme)

        #expect(banner.title == "Download removed")
        #expect(stores.library.installed.isEmpty)
    }
}

@MainActor
struct DesktopStoreTests {
    let harness = Harness()

    @Test func applyRecordsTheCurrentThemeAndSnapshot() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        #expect(!stores.desktop.hasOriginalSnapshot)

        let summary = await stores.desktop.apply(theme, wallpaperFile: "2-swirl-buck.webp", options: ApplyOptions())

        #expect(summary.kind == .success)
        #expect(stores.desktop.hasOriginalSnapshot)
        #expect(stores.desktop.currentTheme?.slug == theme.slug)
        #expect(stores.desktop.currentWallpaper == "2-swirl-buck.webp")
        #expect(stores.desktop.preferredWallpaper(for: theme) == "2-swirl-buck.webp")
        #expect(stores.desktop.applyingSlug == nil)
    }

    /// Mirrors the Windows `Choices_from_the_apply_dialog_can_become_the_defaults`.
    @Test func choicesFromTheApplySheetCanBecomeTheDefaults() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        let choices = ApplyOptions(wallpaper: true, appearanceMode: false, accentColor: false, fit: .center)

        _ = await stores.desktop.apply(theme, wallpaperFile: nil, options: choices)
        #expect(stores.preferences.settings.applyDefaults == ApplyOptions())

        _ = await stores.desktop.apply(theme, wallpaperFile: nil, options: choices, rememberOptions: true)
        #expect(stores.preferences.settings.applyDefaults == choices)
        #expect(harness.makeStores().preferences.settings.applyDefaults == choices)
    }

    @Test func onlyOneApplyRunsAtATime() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)

        async let first = stores.desktop.apply(theme, wallpaperFile: nil, options: ApplyOptions())
        async let second = stores.desktop.apply(theme, wallpaperFile: nil, options: ApplyOptions())
        let titles = await [first, second].map(\.title).sorted()

        #expect(titles == ["Already applying a theme", "Tokyo Night applied"])
    }

    @Test func switchingTheCurrentWallpaperSetsOnlyTheWallpaperWithTheSavedFit() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        stores.preferences.update { $0.applyDefaults.fit = .center }
        _ = await stores.desktop.apply(theme, wallpaperFile: "0-winding-road.webp", options: ApplyOptions())
        harness.backend.clearCalls()

        await stores.desktop.setCurrentWallpaper("10-oma.webp")

        #expect(harness.backend.calls == ["wallpaper:10-oma.webp:center:#e1e2e7"])
        #expect(stores.desktop.currentWallpaper == "10-oma.webp")
        #expect(stores.banners[.currentTheme] == nil)
        #expect(stores.desktop.settingWallpaper == nil)
    }

    @Test func aFailedWallpaperSwitchShowsInTheCard() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        _ = await stores.desktop.apply(theme, wallpaperFile: "0-winding-road.webp", options: ApplyOptions())
        harness.backend.failOn("wallpaper")

        await stores.desktop.setCurrentWallpaper("10-oma.webp")

        #expect(stores.banners[.currentTheme]?.title == "Couldn't change the wallpaper")
        #expect(stores.desktop.currentWallpaper == "0-winding-road.webp")
    }

    @Test func restoreForgetsTheCurrentTheme() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        _ = await stores.desktop.apply(theme, wallpaperFile: nil, options: ApplyOptions())

        let banner = await stores.desktop.restoreOriginal()

        #expect(banner.kind == .success)
        #expect(stores.desktop.currentTheme == nil)
        #expect(!stores.desktop.hasOriginalSnapshot)
    }

    @Test func aFailedRestoreKeepsEverything() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        _ = await stores.desktop.apply(theme, wallpaperFile: nil, options: ApplyOptions())
        harness.backend.failOn("restore")

        let banner = await stores.desktop.restoreOriginal()

        #expect(banner.kind == .error)
        #expect(stores.desktop.hasOriginalSnapshot)
        #expect(stores.desktop.currentTheme?.slug == theme.slug)
    }

    @Test func aSavedFitThisMacCantDoFallsBackToFill() {
        harness.backend.supportedFits = [.fill, .fit, .stretch, .center]
        let stores = harness.makeStores()

        stores.preferences.update { $0.applyDefaults.fit = .center }
        #expect(stores.desktop.defaultFit == .center)

        stores.preferences.update { $0.applyDefaults.fit = .span } // e.g. from Windows
        #expect(stores.desktop.defaultFit == .fill)
    }
}

@MainActor
struct TerminalStoreTests {
    let harness = Harness()

    @Test func iTermIsSelectedUntilAnotherIsPicked() {
        let stores = harness.makeStores()
        #expect(stores.terminals.selected.id == "iterm2")

        stores.terminals.select("terminal")

        #expect(harness.makeStores().terminals.selected.id == "terminal")
    }

    @Test func addAndRemove() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        let palette = try #require(theme.palette)

        let added = await stores.terminals.add(Harness.tokyoEntry, palette: palette)

        #expect(added.title == "Added to iTerm2")
        #expect(stores.terminals.isAdded(Harness.tokyoEntry))

        let removed = stores.terminals.remove(Harness.tokyoEntry)

        #expect(removed.title == "Removed from iTerm2")
        #expect(!stores.terminals.isAdded(Harness.tokyoEntry))
    }

    @Test func terminalsThatCantRemoveExplainHow() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        stores.terminals.select("terminal")
        _ = await stores.terminals.add(Harness.tokyoEntry, palette: try #require(theme.palette))

        let banner = stores.terminals.remove(Harness.tokyoEntry)

        #expect(banner.title == "Remove it in Terminal")
        #expect(banner.message == "Remove Tokyo Night (Omarchy) by hand.")
        #expect(stores.terminals.isAdded(Harness.tokyoEntry))
    }

    @Test func failuresAreReported() async throws {
        let stores = harness.makeStores()
        let theme = try await harness.installTokyo(in: stores)
        harness.iterm.failNextCalls()

        let banner = await stores.terminals.add(Harness.tokyoEntry, palette: try #require(theme.palette))

        #expect(banner.kind == .error)
        #expect(banner.message == "iTerm2 is read-only")
    }

    @Test func oldDownloadsAreLookedUpAgainForTheirBrightColors() async throws {
        let stores = harness.makeStores()
        _ = try await harness.installTokyo(in: stores)
        // A palette saved before `muted` was read.
        let saved = Palette(background: RgbColor("#e1e2e7"), foreground: RgbColor("#3760bf"), accent: RgbColor("#2e7de9"),
                            source: .colorsToml)

        _ = await stores.terminals.add(Harness.tokyoEntry, palette: saved)

        let colors = try #require(harness.iterm.colors[Harness.tokyoEntry.slug])
        #expect(colors.ansi[8] == RgbColor("#8990b3")) // colors-named.toml's muted, from GitHub
    }
}

@MainActor
struct AppServicesTests {
    let dir = TempDir()

    func live(_ environment: [String: String]) -> AppServices {
        // Dry run and a temp data folder: building the services must not touch the real Mac.
        AppServices.live(environment: environment.merging(
            ["OMARCHY_THEMES_DRY_RUN": "1", "OMARCHY_THEMES_DATA_DIR": dir.url.path]) { $1 })
    }

    @Test func theGitHubTokenComesFromGITHUB_TOKENOnly() {
        #expect(live(["GITHUB_TOKEN": "secret"]).gitHubTokenIsSet)
        #expect(!live(["OMARCHY_THEMES_GITHUB_TOKEN": "secret"]).gitHubTokenIsSet)
        #expect(!live([:]).gitHubTokenIsSet)
    }

    @Test func testSwitchesAreHonoured() {
        let services = live([:])
        #expect(services.isDryRun)
        #expect(services.paths.root == dir.paths.root)
    }
}
