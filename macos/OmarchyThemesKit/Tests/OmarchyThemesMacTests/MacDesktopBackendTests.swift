import AppKit
import Foundation
import ImageIO
import OmarchyThemesKit
import Testing
import UniformTypeIdentifiers
@testable import OmarchyThemesMac

/// In-memory screens; records every set call instead of changing the real desktop.
final class FakeWallpaperAPI: WallpaperAPI, @unchecked Sendable {
    private let lock = NSLock()
    private var screens: [ScreenWallpaper]
    private var calls: [(url: URL, options: WallpaperOptions, screenID: String)] = []
    var failingScreens: Set<String> = []

    init(_ screens: [ScreenWallpaper]) {
        self.screens = screens
    }

    var setCalls: [(url: URL, options: WallpaperOptions, screenID: String)] { lock.withLock { calls } }

    func connect(_ screen: ScreenWallpaper) {
        lock.withLock { screens.append(screen) }
    }

    func disconnectAll() {
        lock.withLock { screens.removeAll() }
    }

    func currentWallpapers() async -> [ScreenWallpaper] {
        lock.withLock { screens }
    }

    func setWallpaper(_ url: URL, options: WallpaperOptions, screenID: String) async throws {
        try lock.withLock {
            if failingScreens.contains(screenID) { throw NoSuchScreenError() }
            calls.append((url, options, screenID))
            if let i = screens.firstIndex(where: { $0.screenID == screenID }) {
                screens[i].imageURL = url
                screens[i].options = options
            }
        }
    }
}

final class TempFolder {
    let url = FileManager.default.temporaryDirectory.appending(path: "omatheme-mac-tests/\(UUID().uuidString)", directoryHint: .isDirectory)

    init() {
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }

    @discardableResult
    func file(_ name: String, _ contents: Data = Data([1, 2, 3])) -> URL {
        let file = url.appending(path: name)
        try! FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try! contents.write(to: file)
        return file
    }
}

/// Writes a small solid-color image with ImageIO in the given format.
func writeImage(to url: URL, type: UTType) throws {
    let context = CGContext(data: nil, width: 4, height: 3, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(CGColor(srgbRed: 0.48, green: 0.64, blue: 0.97, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: 4, height: 3))
    let destination = try #require(CGImageDestinationCreateWithURL(url as CFURL, type.identifier as CFString, 1, nil))
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    #expect(CGImageDestinationFinalize(destination))
}

struct MacDesktopBackendTests {
    let temp = TempFolder()
    let original: URL
    let systemPicture = URL(filePath: "/System/Library/Desktop Pictures/Example.heic")
    let api: FakeWallpaperAPI
    let backend: MacDesktopBackend

    init() {
        original = temp.file("Pictures/beach.jpg")
        api = FakeWallpaperAPI([
            ScreenWallpaper(screenID: "1", imageURL: original, options: WallpaperOptions(scaling: 3, allowClipping: true)),
            ScreenWallpaper(
                screenID: "2", imageURL: original,
                options: WallpaperOptions(scaling: 1, allowClipping: false, fillColor: RgbColor(hex: "#102030"))),
        ])
        backend = MacDesktopBackend(wallpapers: api, snapshotAssetsDir: temp.url.appending(path: "original-desktop"))
    }

    @Test func reportsOnlyWallpaperAndTheFitsNSWorkspaceSupports() {
        #expect(backend.capabilities == .wallpaper)
        #expect(backend.supportedFits == [.fill, .fit, .stretch, .center])
    }

    @Test func fitsMapToNSWorkspaceOptions() {
        let fill = RgbColor(hex: "#000000")
        let proportional = NSImageScaling.scaleProportionallyUpOrDown.rawValue
        #expect(WallpaperOptions(fit: .fill, fillColor: fill) == WallpaperOptions(scaling: proportional, allowClipping: true, fillColor: fill))
        #expect(WallpaperOptions(fit: .fit, fillColor: nil) == WallpaperOptions(scaling: proportional, allowClipping: false))
        #expect(WallpaperOptions(fit: .stretch, fillColor: nil).scaling == NSImageScaling.scaleAxesIndependently.rawValue)
        #expect(WallpaperOptions(fit: .center, fillColor: nil).scaling == NSImageScaling.scaleNone.rawValue)
        #expect(WallpaperOptions(fit: .tile, fillColor: nil) == WallpaperOptions(fit: .fill, fillColor: nil))
    }

    @Test func optionsRoundTripThroughTheNSWorkspaceDictionary() {
        let options = WallpaperOptions(scaling: NSImageScaling.scaleNone.rawValue, allowClipping: false, fillColor: RgbColor(hex: "#7aa2f7"))
        #expect(NSWorkspaceWallpaperAPI.options(from: NSWorkspaceWallpaperAPI.dictionary(from: options)) == options)
        #expect(NSWorkspaceWallpaperAPI.dictionary(from: WallpaperOptions()).isEmpty)
    }

    @Test func captureRecordsEveryScreenAndCopiesEachOriginalOnce() async throws {
        let snapshot = try await backend.capture()

        #expect(snapshot.values["screens"] == "1,2")
        #expect(snapshot.values["screen.url.1"] == original.path)
        let copy = try #require(snapshot.values["screen.copy.1"])
        #expect(snapshot.values["screen.copy.2"] == copy)
        #expect(FileManager.default.contents(atPath: copy) == Data([1, 2, 3]))
        let copies = try FileManager.default.contentsOfDirectory(atPath: temp.url.appending(path: "original-desktop").path)
        #expect(copies == ["0.jpg"])
    }

    @Test func captureDoesNotCopySystemPictures() async throws {
        let api = FakeWallpaperAPI([ScreenWallpaper(screenID: "1", imageURL: systemPicture, options: WallpaperOptions())])
        let backend = MacDesktopBackend(wallpapers: api, snapshotAssetsDir: temp.url.appending(path: "original-desktop"))

        let snapshot = try await backend.capture()

        #expect(snapshot.values["screen.url.1"] == systemPicture.path)
        #expect(snapshot.values["screen.copy.1"] == nil)
    }

    @Test func captureFailsWithoutScreens() async {
        let backend = MacDesktopBackend(wallpapers: FakeWallpaperAPI([]), snapshotAssetsDir: temp.url)
        await #expect(throws: NoScreensError.self) { try await backend.capture() }
    }

    @Test func restorePutsBackEachScreensPictureAndOptions() async throws {
        let snapshot = try await backend.capture()
        let themed = temp.file("themes/tokyo/wallpapers/1.png")
        try await backend.setWallpaper(themed, fit: .fit, fillColor: nil)

        try await backend.restore(snapshot)

        let screens = await api.currentWallpapers()
        #expect(screens.map(\.imageURL) == [original, original])
        #expect(screens[0].options == WallpaperOptions(scaling: 3, allowClipping: true))
        #expect(screens[1].options == WallpaperOptions(scaling: 1, allowClipping: false, fillColor: RgbColor(hex: "#102030")))
    }

    @Test func restoreFallsBackToThePrivateCopyWhenTheOriginalIsGone() async throws {
        let snapshot = try await backend.capture()
        try FileManager.default.removeItem(at: original)

        try await backend.restore(snapshot)

        let restored = await api.currentWallpapers().map(\.imageURL?.path)
        #expect(restored == [snapshot.values["screen.copy.1"], snapshot.values["screen.copy.2"]])
    }

    @Test func aScreenConnectedLaterGetsTheMainScreensOriginal() async throws {
        let snapshot = try await backend.capture()
        api.connect(ScreenWallpaper(screenID: "3", imageURL: temp.file("other.png"), options: WallpaperOptions()))

        try await backend.restore(snapshot)

        let screens = await api.currentWallpapers()
        #expect(screens[2].imageURL == original)
        #expect(screens[2].options == WallpaperOptions(scaling: 3, allowClipping: true))
    }

    @Test func restoreFailsOnlyIfEveryScreenFails() async throws {
        let snapshot = try await backend.capture()
        api.failingScreens = ["2"]
        try await backend.restore(snapshot)

        api.failingScreens = ["1", "2"]
        await #expect(throws: NoSuchScreenError.self) { try await backend.restore(snapshot) }
    }

    @Test func restoreWithNoDisplayKeepsTheOriginalDesktop() async throws {
        let applier = ThemeApplier(backend: backend, snapshots: FileSnapshotStore(paths: AppPaths(root: temp.url)))
        let themed = temp.file("themes/tokyo/wallpapers/1.png")
        _ = try await applier.apply(ApplyRequest(themeName: "Tokyo", wallpaper: themed, mode: .dark, accent: nil, options: ApplyOptions()))
        api.disconnectAll()

        await #expect(throws: NoScreensError.self) { try await applier.restoreOriginal() }

        #expect(applier.hasOriginalSnapshot)
    }

    @Test func restoreFailsWhenEveryDisplayItTriedFails() async throws {
        // Screen 2 has no saved picture, so only screen 1 is attempted; it failing is a failure.
        let snapshot = DesktopSnapshot(takenAt: Date(), values: [
            "screens": "1,2",
            "screen.url.1": original.path,
            "screen.url.2": "",
        ])
        api.failingScreens = ["1"]

        await #expect(throws: NoSuchScreenError.self) { try await backend.restore(snapshot) }
    }

    @Test func aDisplayThatFailsDoesNotStopTheOthersAndTheErrorSaysHowMany() async throws {
        let image = temp.file("themes/tokyo/wallpapers/1.png")
        api.failingScreens = ["1"]

        let error = await #expect(throws: PartialWallpaperError.self) {
            try await backend.setWallpaper(image, fit: .fill, fillColor: nil)
        }

        #expect(api.setCalls.map(\.screenID) == ["2"])
        #expect(error?.localizedDescription.hasPrefix("It was set on 1 of 2 displays.") == true)
    }

    @Test func everyDisplayFailingIsAPlainFailure() async throws {
        let image = temp.file("themes/tokyo/wallpapers/1.png")
        api.failingScreens = ["1", "2"]

        await #expect(throws: NoSuchScreenError.self) { try await backend.setWallpaper(image, fit: .fill, fillColor: nil) }
    }

    @Test func setWallpaperAppliesToEveryScreenWithTheFitAndFillColor() async throws {
        let image = temp.file("themes/tokyo/wallpapers/1.png")

        try await backend.setWallpaper(image, fit: .center, fillColor: RgbColor(hex: "#1a1b26"))

        let calls = api.setCalls
        #expect(calls.map(\.screenID) == ["1", "2"])
        #expect(calls.allSatisfy { $0.url == image })
        #expect(calls[0].options == WallpaperOptions(fit: .center, fillColor: RgbColor(hex: "#1a1b26")))
    }

    @Test func webpIsConvertedToPNGBeforeBeingSet() async throws {
        let webp = temp.url.appending(path: "wallpapers/night.webp")
        try FileManager.default.createDirectory(at: webp.deletingLastPathComponent(), withIntermediateDirectories: true)
        try WebP.onePixel.write(to: webp)

        try await backend.setWallpaper(webp, fit: .fill, fillColor: nil)

        let url = try #require(api.setCalls.first?.url)
        #expect(url.lastPathComponent == "night.png")
        #expect(url.deletingLastPathComponent().lastPathComponent == ".converted")
    }

    @Test func modeAndAccentAreNotSupported() async {
        await #expect(throws: UnsupportedOnMacError.self) { try await backend.setAppearanceMode(.dark) }
        await #expect(throws: UnsupportedOnMacError.self) { try await backend.setAccentColor(RgbColor(r: 1, g: 2, b: 3)) }
    }

    @Test func applyingThroughThemeApplierReportsModeAndAccentAsNotSupported() async throws {
        let themed = temp.file("themes/tokyo/wallpapers/1.png")
        let applier = ThemeApplier(backend: backend, snapshots: FileSnapshotStore(paths: AppPaths(root: temp.url)))

        let request = ApplyRequest(
            themeName: "Tokyo", wallpaper: themed, mode: .dark, accent: RgbColor(hex: "#7aa2f7"), options: ApplyOptions())
        let result = try await applier.apply(request)

        #expect(result.result(for: .saveOriginal)?.outcome == .applied)
        #expect(result.result(for: .wallpaper)?.outcome == .applied)
        #expect(result.result(for: .appearanceMode)?.outcome == .notSupported)
        #expect(result.result(for: .accentColor)?.outcome == .notSupported)
        #expect(ApplySummary.describe(result, themeName: "Tokyo", mode: .dark).message == "Updated the wallpaper.")

        #expect(try await applier.restoreOriginal())
        #expect(await api.currentWallpapers().map(\.imageURL) == [original, original])
    }
}

struct WallpaperImageConverterTests {
    let temp = TempFolder()
    let converter = WallpaperImageConverter()

    @Test(arguments: ["a.png", "b.JPG", "c.jpeg", "d.heic", "e.tiff"])
    func directFormatsAreUsedAsIs(name: String) throws {
        let file = temp.file(name)
        #expect(try converter.ensureSupportedFormat(file) == file)
    }

    @Test func imageIODecodesWebP() throws {
        let file = temp.file("one.webp", WebP.onePixel)
        let source = try #require(CGImageSourceCreateWithURL(file as CFURL, nil))
        let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
        #expect(image.width == 1 && image.height == 1)
    }

    @Test func bmpIsConvertedToPNGOnceAndReused() throws {
        let bmp = temp.url.appending(path: "wallpapers/old.bmp")
        try FileManager.default.createDirectory(at: bmp.deletingLastPathComponent(), withIntermediateDirectories: true)
        try writeImage(to: bmp, type: .bmp)

        let first = try converter.ensureSupportedFormat(bmp)
        let modified = try first.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
        let second = try converter.ensureSupportedFormat(bmp)

        #expect(first == second)
        #expect(try second.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate == modified)
        let source = try #require(CGImageSourceCreateWithURL(first as CFURL, nil))
        #expect(CGImageSourceGetType(source) as String? == UTType.png.identifier)
        #expect(CGImageSourceCreateImageAtIndex(source, 0, nil)?.width == 4)
    }

    @Test func unreadableImagesGiveAnActionableError() {
        let broken = temp.file("broken.webp", Data("not an image".utf8))
        #expect(throws: ImageConversionError.self) { try converter.ensureSupportedFormat(broken) }
        #expect(ImageConversionError(fileName: "broken.webp").localizedDescription.contains("downloading the theme again"))
    }
}

enum WebP {
    /// A 1×1 lossless WebP.
    static let onePixel = Data(base64Encoded: "UklGRhoAAABXRUJQVlA4TA0AAAAvAAAAEAcQERGIiP4HAA==")!
}
