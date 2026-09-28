// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "OmarchyThemesKit",
    platforms: [.macOS(.v14)],
    products: [
        // Catalog, GitHub resolution, palettes, store and ThemeApplier. No AppKit.
        .library(name: "OmarchyThemesKit", targets: ["OmarchyThemesKit"]),
        // The macOS DesktopBackend (NSWorkspace wallpaper, ImageIO conversion) and terminal exporters.
        .library(name: "OmarchyThemesMac", targets: ["OmarchyThemesMac"]),
        // The app's observable stores (catalog, library, desktop, terminals, preferences). No SwiftUI.
        .library(name: "OmarchyThemesStores", targets: ["OmarchyThemesStores"]),
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
        .target(
            name: "OmarchyThemesStores",
            dependencies: ["OmarchyThemesKit", "OmarchyThemesMac"]
        ),
        // Fakes and fixtures shared by the test targets; not part of any product.
        .target(
            name: "OmarchyThemesTestSupport",
            dependencies: ["OmarchyThemesKit", "OmarchyThemesMac"]
        ),
        .testTarget(
            name: "OmarchyThemesKitTests",
            dependencies: ["OmarchyThemesKit", "OmarchyThemesTestSupport"]
        ),
        .testTarget(
            name: "OmarchyThemesMacTests",
            dependencies: ["OmarchyThemesMac"]
        ),
        .testTarget(
            name: "OmarchyThemesStoresTests",
            dependencies: ["OmarchyThemesStores", "OmarchyThemesTestSupport"]
        ),
    ]
)
