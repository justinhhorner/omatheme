import Foundation
import OmarchyThemesKit

/// Used when OMARCHY_THEMES_DRY_RUN=1: behaves like the macOS backend (wallpaper only) but
/// changes nothing, so the UI can be exercised end to end without touching the real desktop.
struct DryRunDesktopBackend: DesktopBackend {
    let capabilities: DesktopCapabilities = [.wallpaper]
    let supportedFits: [WallpaperFit] = [.fill, .fit, .stretch, .center]

    func capture() async throws -> DesktopSnapshot {
        DesktopSnapshot(takenAt: Date(), values: ["dryRun": "1"])
    }

    func restore(_ snapshot: DesktopSnapshot) async throws {}

    func setWallpaper(_ image: URL, fit: WallpaperFit, fillColor: RgbColor?) async throws {
        try await Task.sleep(for: .milliseconds(300))
    }

    func setAppearanceMode(_ mode: AppearanceMode) async throws {}

    func setAccentColor(_ accent: RgbColor) async throws {}
}
