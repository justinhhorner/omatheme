import CryptoKit
import Foundation
import Testing
@testable import OmarchyThemesKit

/// The saved files match docs/data-format.md, the format shared with the Windows app. The Windows
/// `DataFormatTests` check the same fixtures (fixtures/data), so the two apps write identical files.
struct DataFormatTests {
    let dir = TempDir()

    /// Equal as JSON: same keys, types and values (key order and whitespace don't matter).
    static func expectSameJSON(_ expected: String, _ actual: Data, sourceLocation: SourceLocation = #_sourceLocation) throws {
        let a = try JSONSerialization.jsonObject(with: Data(expected.utf8)) as? NSDictionary
        let b = try JSONSerialization.jsonObject(with: actual) as? NSDictionary
        #expect(a != nil && a == b, "Expected JSON equivalent to:\n\(expected)\nActual:\n\(String(decoding: actual, as: UTF8.self))",
                sourceLocation: sourceLocation)
    }

    static func write<T: Encodable>(_ value: T) throws -> Data {
        try JSONFile.makeEncoder().encode(value)
    }

    static func read<T: Decodable>(_ type: T.Type, _ fixture: String) throws -> T {
        try JSONFile.makeDecoder().decode(type, from: Data(Fixture.read(fixture).utf8))
    }

    static let downloadedAt = Date(timeIntervalSince1970: 1_790_499_600.123) // 2026-09-27T09:00:00.123Z

    static func tokyoNight() -> InstalledTheme {
        InstalledTheme(
            slug: "omarchy.tokyo-night",
            name: "Tokyo Night",
            repoURL: URL(string: "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night")!,
            palette: Palette(
                background: RgbColor("#1a1b26"), foreground: RgbColor("#a9b1d6"), accent: RgbColor("#7aa2f7"),
                selection: RgbColor("#292e42"), muted: RgbColor("#414868"), brightForeground: RgbColor("#c0caf5"),
                declaredMode: .dark,
                swatches: [NamedColor(name: "Red", color: RgbColor("#f7768e")), NamedColor(name: "Bright red", color: RgbColor("#ff7a93"))],
                source: .colorsToml),
            mode: .dark,
            wallpapers: ["0-winding-road.webp", "1-quattro.webp"],
            screenshotFile: "screenshot.png",
            downloadedAt: downloadedAt,
            directory: URL(filePath: "/Users/me/Library/Application Support/OmarchyThemes/themes/omarchy.tokyo-night"))
    }

    @Test func themeManifestIsWrittenInTheSharedFormat() throws {
        // "repoUrl", no nulls, no directory, UTC date with "Z".
        try Self.expectSameJSON(Fixture.read("data/theme.json"), Self.write(Self.tokyoNight()))
    }

    @Test(arguments: ["data/theme.json", "data/legacy/windows-theme.json", "data/legacy/macos-theme.json"])
    func themeManifestsReadInEveryFormatRewriteInTheSharedOne(file: String) throws {
        let theme = try Self.read(InstalledTheme.self, file)

        #expect(theme.repoURL.absoluteString == "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night")
        #expect(abs(theme.downloadedAt.timeIntervalSince(Self.downloadedAt)) < 0.0005)
        #expect(theme.palette?.cursor == nil)
        #expect(theme.palette?.brightForeground == RgbColor("#c0caf5"))
        try Self.expectSameJSON(Fixture.read("data/theme.json"), Self.write(theme))
    }

    @Test func settingsAreWrittenInTheSharedFormat() throws {
        var settings = AppSettings()
        settings.welcomeSeen = true
        settings.applyDefaults = ApplyOptions(appearanceMode: false, fit: .center)
        settings.lastAppliedSlug = "omarchy.tokyo-night"
        settings.lastAppliedWallpaper = "1-quattro.webp"
        settings.terminalApp = "ghostty"

        try Self.expectSameJSON(Fixture.read("data/settings.json"), Self.write(settings))
        try Self.expectSameJSON(Fixture.read("data/settings-default.json"), Self.write(AppSettings()))
        try Self.expectSameJSON(Fixture.read("data/settings.json"), Self.write(Self.read(AppSettings.self, "data/settings.json")))
    }

    @Test func olderWindowsSettingsStillRead() throws {
        let settings = try Self.read(AppSettings.self, "data/legacy/windows-settings.json")

        #expect(settings.welcomeSeen)
        #expect(settings.applyDefaults.fit == .center)
        #expect(settings.lastAppliedSlug == "omarchy.tokyo-night")
        #expect(settings.lastAppliedWallpaper == nil)
        #expect(settings.terminalApp == nil)
    }

    @Test func snapshotEnvelopeIsShared() throws {
        let snapshot = DesktopSnapshot(
            takenAt: Date(timeIntervalSince1970: 1_790_499_600.25),
            values: ["screens": "1", "screen.url.1": "/Users/me/Pictures/beach.jpg"])

        try Self.expectSameJSON(Fixture.read("data/original-desktop.json"), Self.write(snapshot))
        #expect(try Self.read(DesktopSnapshot.self, "data/original-desktop.json") == snapshot)
    }

    @Test func httpCacheMetadataIsWrittenInTheSharedFormat() async throws {
        let url = URL(string: "https://example.com/data.json")!
        let http = FakeTransport().on(url.absoluteString) { request in
            (Data("v1".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1",
                                             headerFields: ["ETag": "\"a\"", "Last-Modified": "Sat, 26 Sep 2026 11:00:00 GMT"])!)
        }
        let clock = ManualClock(Date(timeIntervalSince1970: 1_790_424_000)) // 2026-09-26T12:00:00Z

        _ = try await HTTPCache(transport: http, directory: dir.url, now: clock.function).get(url)

        let meta = cachePath(url).appendingPathExtension("json")
        try Self.expectSameJSON(Fixture.read("data/http-cache-meta.json"), Data(contentsOf: meta))
    }

    @Test(arguments: ["data/http-cache-meta.json", "data/legacy/windows-cache-meta.json", "data/legacy/macos-cache-meta.json"])
    func httpCacheMetadataReadsInEveryFormat(file: String) throws {
        let url = URL(string: "https://example.com/data.json")!
        let path = cachePath(url)
        try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(Fixture.read(file).utf8).write(to: path.appendingPathExtension("json"))
        try Data("v1".utf8).write(to: path.appendingPathExtension("body"))

        let cached = try #require(HTTPCache(transport: FakeTransport(), directory: dir.url).cachedResponse(for: url))

        #expect(cached.text == "v1")
        #expect(cached.fetchedAt == Date(timeIntervalSince1970: 1_790_424_000))
        // Windows v0.1 wrote "eTag" and an ISO Last-Modified; they're not usable as validators any more,
        // which only costs one full download of that URL.
        if file.contains("windows") {
            #expect(cached.etag == nil)
        } else {
            #expect(cached.etag == "\"a\"")
            #expect(cached.lastModified == "Sat, 26 Sep 2026 11:00:00 GMT")
        }
    }

    @Test(arguments: [
        "2026-09-27T09:00:00.123Z",
        "2026-09-27T09:00:00.1230000+00:00",
        "2026-09-27T11:00:00.123+02:00",
        "2026-09-27T05:00:00.123-0400",
        "2026-09-27T09:00:00.123", // older macOS files: no zone meant UTC
    ])
    func datesReadInAnyISOFormAndWriteAsUTCMilliseconds(text: String) throws {
        let date = try #require(JSONFile.parseDate(text))

        #expect(abs(date.timeIntervalSince(Self.downloadedAt)) < 0.0005)
        #expect(JSONFile.formatDate(date) == "2026-09-27T09:00:00.123Z")
    }

    @Test func datesWithoutAFractionAndBadDates() {
        #expect(JSONFile.parseDate("2026-09-27T09:00:00Z") == Date(timeIntervalSince1970: 1_790_499_600))
        #expect(JSONFile.parseDate("yesterday") == nil)
        #expect(JSONFile.parseDate("2026-09-27") == nil)
    }

    /// cache/<xx>/<sha256 of the URL>, as the format document specifies.
    private func cachePath(_ url: URL) -> URL {
        let key = SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
        return dir.url.appending(path: String(key.prefix(2)), directoryHint: .isDirectory).appending(path: key)
    }
}
