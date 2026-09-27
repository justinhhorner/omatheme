import Foundation

/// What a platform backend can change. Add a member here when adding a new theming API.
public struct DesktopCapabilities: OptionSet, Sendable, Hashable {
    public let rawValue: Int

    public init(rawValue: Int) {
        self.rawValue = rawValue
    }

    public static let wallpaper = DesktopCapabilities(rawValue: 1 << 0)
    public static let appearanceMode = DesktopCapabilities(rawValue: 1 << 1)
    public static let accentColor = DesktopCapabilities(rawValue: 1 << 2)

    public static let all: DesktopCapabilities = [.wallpaper, .appearanceMode, .accentColor]
}

/// How a wallpaper is scaled. Tile and Span exist on Windows; a backend lists what it supports
/// in `DesktopBackend.supportedFits`.
public enum WallpaperFit: String, Codable, Sendable, CaseIterable, Identifiable {
    case fill
    case fit
    case stretch
    case center
    case tile
    case span

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .fill: "Fill Screen"
        case .fit: "Fit to Screen"
        case .stretch: "Stretch to Fill Screen"
        case .center: "Center"
        case .tile: "Tile"
        case .span: "Span"
        }
    }
}

/// Opaque, backend-defined record of the user's desktop before we changed it. The Kit only stores
/// and returns it; the backend decides what goes in `values`.
public struct DesktopSnapshot: Codable, Sendable, Equatable {
    public var takenAt: Date
    public var values: [String: String]

    public init(takenAt: Date, values: [String: String]) {
        self.takenAt = takenAt
        self.values = values
    }
}

/// The OS-specific side of theming. Implementations call the platform's real APIs (macOS:
/// NSWorkspace). Methods for capabilities the backend doesn't report are never called.
public protocol DesktopBackend: Sendable {
    var capabilities: DesktopCapabilities { get }

    /// The fits the Apply sheet and Settings offer, in display order.
    var supportedFits: [WallpaperFit] { get }

    func capture() async throws -> DesktopSnapshot

    func restore(_ snapshot: DesktopSnapshot) async throws

    /// Sets `image` (a local file) as the wallpaper on every screen. `fillColor` shows around
    /// images that don't cover the screen (Fit, Center), where the OS supports it.
    func setWallpaper(_ image: URL, fit: WallpaperFit, fillColor: RgbColor?) async throws

    func setAppearanceMode(_ mode: AppearanceMode) async throws

    func setAccentColor(_ accent: RgbColor) async throws
}
