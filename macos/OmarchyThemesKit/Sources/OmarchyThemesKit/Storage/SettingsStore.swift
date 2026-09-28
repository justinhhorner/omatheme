import Foundation

public struct AppSettings: Codable, Sendable, Equatable {
    /// The welcome screen shows until the user continues past it once.
    public var welcomeSeen = false

    /// Defaults for the Apply sheet's per-aspect checkboxes, and for one-click apply.
    public var applyDefaults = ApplyOptions()

    /// The theme on the desktop (the last one applied), or nil.
    public var lastAppliedSlug: String?

    /// Which of that theme's wallpapers is on the desktop, or nil if none of them is.
    public var lastAppliedWallpaper: String?

    /// The terminal app the theme page sends colors to (a `TerminalExporter` id); nil means the
    /// platform default (iTerm2 on macOS). Optional so a fresh file matches the Windows app's.
    public var terminalApp: String?

    public init() {}

    // Missing keys fall back to defaults, so older or hand-edited files still load.
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        welcomeSeen = try c.decodeIfPresent(Bool.self, forKey: .welcomeSeen) ?? false
        applyDefaults = try c.decodeIfPresent(ApplyOptions.self, forKey: .applyDefaults) ?? ApplyOptions()
        lastAppliedSlug = try c.decodeIfPresent(String.self, forKey: .lastAppliedSlug)
        lastAppliedWallpaper = try c.decodeIfPresent(String.self, forKey: .lastAppliedWallpaper)
        terminalApp = try c.decodeIfPresent(String.self, forKey: .terminalApp)
    }

    /// The settings after applying `slug`. The theme becomes current if any step applied; its
    /// wallpaper only if the wallpaper step itself applied (with the wallpaper unchecked, or failing,
    /// the desktop keeps showing whatever it showed before).
    public func afterApply(_ slug: String, wallpaperFile: String?, result: ApplyResult) -> AppSettings {
        guard result.anyApplied else { return self }
        var updated = self
        let wallpaperApplied = result.result(for: .wallpaper)?.outcome == .applied
        updated.lastAppliedWallpaper = wallpaperApplied ? wallpaperFile
            : lastAppliedSlug == slug ? lastAppliedWallpaper
            : nil
        updated.lastAppliedSlug = slug
        return updated
    }
}

public struct SettingsStore: Sendable {
    private let paths: AppPaths

    public init(paths: AppPaths) {
        self.paths = paths
    }

    public func load() -> AppSettings {
        JSONFile.read(AppSettings.self, from: paths.settingsFile) ?? AppSettings()
    }

    public func save(_ settings: AppSettings) throws {
        try JSONFile.write(settings, to: paths.settingsFile)
    }

    @discardableResult
    public func update(_ change: (inout AppSettings) -> Void) throws -> AppSettings {
        var settings = load()
        change(&settings)
        try save(settings)
        return settings
    }
}
