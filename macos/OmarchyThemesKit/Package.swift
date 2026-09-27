// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "OmarchyThemesKit",
    platforms: [.macOS(.v14)],
    products: [
        // Catalog, GitHub resolution, palettes, store and ThemeApplier. No AppKit.
        .library(name: "OmarchyThemesKit", targets: ["OmarchyThemesKit"]),
        // The macOS DesktopBackend (NSWorkspace wallpaper, ImageIO conversion).
        .library(name: "OmarchyThemesMac", targets: ["OmarchyThemesMac"]),
    ],
    dependencies: [
        .package(url: "https://github.com/scinfu/SwiftSoup.git", from: "2.13.9"),
    ],
    targets: [
        .target(
            name: "OmarchyThemesKit",
            dependencies: [.product(name: "SwiftSoup", package: "SwiftSoup")]
        ),
        .target(
            name: "OmarchyThemesMac",
            dependencies: ["OmarchyThemesKit"]
        ),
        .testTarget(
            name: "OmarchyThemesKitTests",
            dependencies: ["OmarchyThemesKit"]
        ),
        .testTarget(
            name: "OmarchyThemesMacTests",
            dependencies: ["OmarchyThemesMac"]
        ),
    ]
)
