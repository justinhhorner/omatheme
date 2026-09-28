import Foundation
import Observation
import OmarchyThemesKit

/// Applying themes to the desktop, the theme currently on it, and restoring the original.
@MainActor
@Observable
public final class DesktopStore {
    /// The theme being applied right now, if any; only one apply runs at a time.
    public private(set) var applyingSlug: String?
    public private(set) var hasOriginalSnapshot: Bool
    /// The current theme's wallpaper being set from the Current Theme card, for its spinner.
    public private(set) var settingWallpaper: String?

    private let applier: ThemeApplier
    private let preferences: Preferences
    private let library: ThemeLibrary
    private let banners: Banners

    public init(applier: ThemeApplier, preferences: Preferences, library: ThemeLibrary, banners: Banners) {
        self.applier = applier
        self.preferences = preferences
        self.library = library
        self.banners = banners
        hasOriginalSnapshot = applier.hasOriginalSnapshot
    }

    public var capabilities: DesktopCapabilities { applier.capabilities }

    public var supportedFits: [WallpaperFit] { applier.supportedFits }

    /// The saved one-click fit, or Fill if this Mac doesn't offer it (e.g. Tile from Windows).
    public var defaultFit: WallpaperFit {
        let fit = preferences.settings.applyDefaults.fit
        return supportedFits.contains(fit) ? fit : .fill
    }

    // MARK: Current theme

    /// The slug of the theme on the desktop (the last one applied).
    public var activeSlug: String? { preferences.settings.lastAppliedSlug }

    /// The theme on the desktop, if it's still downloaded (nil after Restore, which clears it).
    public var currentTheme: InstalledTheme? {
        activeSlug.flatMap(library.installedTheme)
    }

    /// Which of the current theme's wallpapers is on the desktop, if any.
    public var currentWallpaper: String? { preferences.settings.lastAppliedWallpaper }

    /// The wallpaper to preselect for a downloaded theme: the one last applied, else the first.
    public func preferredWallpaper(for theme: InstalledTheme) -> String? {
        if activeSlug == theme.slug, let last = currentWallpaper, theme.wallpapers.contains(last) {
            return last
        }
        return theme.wallpapers.first
    }

    // MARK: Apply

    public func apply(_ theme: InstalledTheme, wallpaperFile: String?, options: ApplyOptions) async -> ApplySummary {
        guard applyingSlug == nil else {
            return ApplySummary(kind: .info, title: "Already applying a theme", message: "Wait for it to finish, then try again.")
        }
        applyingSlug = theme.slug
        defer {
            applyingSlug = nil
            hasOriginalSnapshot = applier.hasOriginalSnapshot
        }

        let request = ApplyRequest.from(theme, wallpaperFile: wallpaperFile, options: options)
        do {
            let result = try await applier.apply(request)
            preferences.update { $0 = $0.afterApply(theme.slug, wallpaperFile: request.wallpaper?.lastPathComponent, result: result) }
            return ApplySummary.describe(result, themeName: theme.name, mode: theme.mode)
        } catch is CancellationError {
            return ApplySummary(kind: .info, title: "Apply cancelled", message: "Nothing was changed.")
        } catch {
            // ThemeApplier reports per-step failures in its result; anything thrown is unexpected.
            logFailure("Applying \(theme.slug)", error)
            return ApplySummary(kind: .error, title: "Couldn't apply \(theme.name)", message: error.localizedDescription)
        }
    }

    /// One-click apply with the defaults from Settings.
    public func applyWithDefaults(_ theme: InstalledTheme) async -> ApplySummary {
        await apply(theme, wallpaperFile: preferredWallpaper(for: theme), options: preferences.settings.applyDefaults)
    }

    /// Sets another of the current theme's wallpapers: wallpaper only (its light/dark and accent
    /// are already applied), with the saved fit and the theme's background as fill color.
    /// Success shows as the check mark moving; problems go to the card's banner.
    public func setCurrentWallpaper(_ file: String) async {
        guard let theme = currentTheme, file != currentWallpaper, applyingSlug == nil else { return }
        settingWallpaper = file
        banners[.currentTheme] = nil
        defer { settingWallpaper = nil }

        let options = ApplyOptions(wallpaper: true, appearanceMode: false, accentColor: false, fit: preferences.settings.applyDefaults.fit)
        let summary = await apply(theme, wallpaperFile: file, options: options)
        if summary.kind != .success {
            var banner = Banner(summary)
            if summary.kind == .error { banner.title = "Couldn't change the wallpaper" }
            banners[.currentTheme] = banner
        }
    }

    // MARK: Restore

    /// Puts back the desktop saved before the first Apply and forgets the current theme.
    public func restoreOriginal() async -> Banner {
        defer { hasOriginalSnapshot = applier.hasOriginalSnapshot }
        do {
            guard try await applier.restoreOriginal() else {
                return Banner(kind: .info, title: "Nothing to restore", message: "No saved desktop was found.")
            }
            preferences.update {
                $0.lastAppliedSlug = nil
                $0.lastAppliedWallpaper = nil
            }
            return Banner(kind: .success, title: "Original desktop restored", message: "Your previous wallpaper is back.")
        } catch {
            return Banner(kind: .error, title: "Couldn't restore your desktop", message: error.localizedDescription)
        }
    }
}
