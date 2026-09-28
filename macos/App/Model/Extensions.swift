import Foundation
import OmarchyThemesKit

extension InstalledTheme {
    /// The screenshot, else the first wallpaper.
    var thumbnailURL: URL? {
        screenshotURL ?? wallpapers.first.map(wallpaperURL)
    }

    /// "1 wallpaper", "3 wallpapers".
    var wallpaperCountDescription: String {
        wallpapers.count == 1 ? "1 wallpaper" : "\(wallpapers.count) wallpapers"
    }
}

extension Date {
    var relativeDescription: String {
        formatted(.relative(presentation: .named))
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
