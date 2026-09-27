import Foundation

public struct AppSettings: Codable, Sendable, Equatable {
    /// The welcome screen shows until the user continues past it once.
    public var welcomeSeen = false

    /// Defaults for the Apply sheet's per-aspect checkboxes, and for one-click apply.
    public var applyDefaults = ApplyOptions()

    public var lastAppliedSlug: String?
    public var lastAppliedWallpaper: String?

    public init() {}

    // Missing keys fall back to defaults, so older or hand-edited files still load.
    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        welcomeSeen = try c.decodeIfPresent(Bool.self, forKey: .welcomeSeen) ?? false
        applyDefaults = try c.decodeIfPresent(ApplyOptions.self, forKey: .applyDefaults) ?? ApplyOptions()
        lastAppliedSlug = try c.decodeIfPresent(String.self, forKey: .lastAppliedSlug)
        lastAppliedWallpaper = try c.decodeIfPresent(String.self, forKey: .lastAppliedWallpaper)
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
