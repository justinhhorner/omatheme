import AppKit
import OmarchyThemesKit

/// Desktop picture options, mirroring `NSWorkspace.DesktopImageOptionKey`. Nil means the key
/// was absent (the system default).
public struct WallpaperOptions: Codable, Sendable, Equatable {
    /// `NSImageScaling` raw value.
    public var scaling: UInt?
    public var allowClipping: Bool?
    public var fillColor: RgbColor?

    public init(scaling: UInt? = nil, allowClipping: Bool? = nil, fillColor: RgbColor? = nil) {
        self.scaling = scaling
        self.allowClipping = allowClipping
        self.fillColor = fillColor
    }

    /// Tile and Span have no NSWorkspace equivalent and fall back to Fill.
    public init(fit: WallpaperFit, fillColor: RgbColor?) {
        switch fit {
        case .fill, .tile, .span:
            self.init(scaling: NSImageScaling.scaleProportionallyUpOrDown.rawValue, allowClipping: true, fillColor: fillColor)
        case .fit:
            self.init(scaling: NSImageScaling.scaleProportionallyUpOrDown.rawValue, allowClipping: false, fillColor: fillColor)
        case .stretch:
            self.init(scaling: NSImageScaling.scaleAxesIndependently.rawValue, allowClipping: true, fillColor: fillColor)
        case .center:
            self.init(scaling: NSImageScaling.scaleNone.rawValue, allowClipping: true, fillColor: fillColor)
        }
    }
}

/// One screen's desktop picture.
public struct ScreenWallpaper: Sendable, Equatable {
    /// The display's `NSScreenNumber` (CGDirectDisplayID), stable while it stays connected.
    public var screenID: String
    public var imageURL: URL?
    public var options: WallpaperOptions

    public init(screenID: String, imageURL: URL?, options: WallpaperOptions) {
        self.screenID = screenID
        self.imageURL = imageURL
        self.options = options
    }
}

/// The NSWorkspace desktop-picture calls, behind a protocol so `MacDesktopBackend` is tested
/// without changing the real desktop.
public protocol WallpaperAPI: Sendable {
    /// Every connected screen, main screen first.
    func currentWallpapers() async -> [ScreenWallpaper]

    func setWallpaper(_ url: URL, options: WallpaperOptions, screenID: String) async throws
}

public struct NoSuchScreenError: LocalizedError, Sendable {
    public var errorDescription: String? { "That display is no longer connected." }
}

/// The real thing. NSWorkspace's desktop-image methods must be called on the main thread.
/// Note that they only affect the current Space on each screen.
public struct NSWorkspaceWallpaperAPI: WallpaperAPI {
    public init() {}

    public func currentWallpapers() async -> [ScreenWallpaper] {
        await MainActor.run {
            let workspace = NSWorkspace.shared
            return NSScreen.screens.compactMap { screen in
                guard let id = Self.id(of: screen) else { return nil }
                return ScreenWallpaper(
                    screenID: id,
                    imageURL: workspace.desktopImageURL(for: screen),
                    options: Self.options(from: workspace.desktopImageOptions(for: screen) ?? [:]))
            }
        }
    }

    public func setWallpaper(_ url: URL, options: WallpaperOptions, screenID: String) async throws {
        try await MainActor.run {
            guard let screen = NSScreen.screens.first(where: { Self.id(of: $0) == screenID }) else {
                throw NoSuchScreenError()
            }
            try NSWorkspace.shared.setDesktopImageURL(url, for: screen, options: Self.dictionary(from: options))
        }
    }

    @MainActor
    private static func id(of screen: NSScreen) -> String? {
        (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.stringValue
    }

    static func options(from dictionary: [NSWorkspace.DesktopImageOptionKey: Any]) -> WallpaperOptions {
        WallpaperOptions(
            scaling: (dictionary[.imageScaling] as? NSNumber)?.uintValue,
            allowClipping: (dictionary[.allowClipping] as? NSNumber)?.boolValue,
            fillColor: (dictionary[.fillColor] as? NSColor).flatMap(rgb))
    }

    static func dictionary(from options: WallpaperOptions) -> [NSWorkspace.DesktopImageOptionKey: Any] {
        var dictionary: [NSWorkspace.DesktopImageOptionKey: Any] = [:]
        if let scaling = options.scaling { dictionary[.imageScaling] = NSNumber(value: scaling) }
        if let allowClipping = options.allowClipping { dictionary[.allowClipping] = NSNumber(value: allowClipping) }
        if let fill = options.fillColor {
            dictionary[.fillColor] = NSColor(srgbRed: CGFloat(fill.r) / 255, green: CGFloat(fill.g) / 255, blue: CGFloat(fill.b) / 255, alpha: 1)
        }
        return dictionary
    }

    private static func rgb(_ color: NSColor) -> RgbColor? {
        guard let srgb = color.usingColorSpace(.sRGB) else { return nil }
        func byte(_ v: CGFloat) -> UInt8 { UInt8((min(max(v, 0), 1) * 255).rounded()) }
        return RgbColor(r: byte(srgb.redComponent), g: byte(srgb.greenComponent), b: byte(srgb.blueComponent))
    }
}
