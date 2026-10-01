import Foundation

/// The user's per-aspect choices in the Apply sheet.
public struct ApplyOptions: Codable, Sendable, Equatable {
    public var wallpaper = true
    public var appearanceMode = true
    public var accentColor = true
    public var fit = WallpaperFit.fill

    public init(wallpaper: Bool = true, appearanceMode: Bool = true, accentColor: Bool = true, fit: WallpaperFit = .fill) {
        self.wallpaper = wallpaper
        self.appearanceMode = appearanceMode
        self.accentColor = accentColor
        self.fit = fit
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        wallpaper = try c.decodeIfPresent(Bool.self, forKey: .wallpaper) ?? true
        appearanceMode = try c.decodeIfPresent(Bool.self, forKey: .appearanceMode) ?? true
        accentColor = try c.decodeIfPresent(Bool.self, forKey: .accentColor) ?? true
        fit = (try? c.decodeIfPresent(WallpaperFit.self, forKey: .fit)) ?? .fill
    }
}

/// What to apply. Built from a downloaded theme, so no network is involved.
public struct ApplyRequest: Sendable {
    public var themeName: String
    public var wallpaper: URL?
    public var mode: AppearanceMode
    public var accent: RgbColor?
    /// The theme's background, shown around wallpapers that don't cover the screen.
    public var background: RgbColor?
    public var options: ApplyOptions

    public init(
        themeName: String, wallpaper: URL?, mode: AppearanceMode, accent: RgbColor?, background: RgbColor? = nil,
        options: ApplyOptions
    ) {
        self.themeName = themeName
        self.wallpaper = wallpaper
        self.mode = mode
        self.accent = accent
        self.background = background
        self.options = options
    }

    /// - Parameter wallpaperFile: One of `theme.wallpapers`; defaults to the first.
    public static func from(_ theme: InstalledTheme, wallpaperFile: String?, options: ApplyOptions) -> ApplyRequest {
        let file = wallpaperFile.flatMap { theme.wallpapers.contains($0) ? $0 : nil } ?? theme.wallpapers.first
        return ApplyRequest(
            themeName: theme.name,
            wallpaper: file.map(theme.wallpaperURL),
            mode: theme.mode,
            accent: theme.palette?.accent,
            background: theme.palette?.background,
            options: options)
    }
}

public enum ApplyStep: String, Sendable, CaseIterable {
    case saveOriginal
    case wallpaper
    case appearanceMode
    case accentColor
}

public enum StepOutcome: Sendable, Equatable {
    case applied
    /// The user unchecked it.
    case skippedByUser
    /// This OS/backend can't change it.
    case notSupported
    /// The theme has nothing for it (e.g. no wallpaper or palette).
    case noData
    /// An earlier step failed in a way that made continuing unsafe.
    case notAttempted
    case failed
}

public struct StepResult: Sendable, Equatable {
    public var step: ApplyStep
    public var outcome: StepOutcome
    public var error: String?

    public init(_ step: ApplyStep, _ outcome: StepOutcome, error: String? = nil) {
        self.step = step
        self.outcome = outcome
        self.error = error
    }
}

public struct ApplyResult: Sendable, Equatable {
    public var steps: [StepResult]

    public init(steps: [StepResult]) {
        self.steps = steps
    }

    public var anyApplied: Bool { steps.contains { $0.outcome == .applied } }

    public func result(for step: ApplyStep) -> StepResult? {
        steps.first { $0.step == step }
    }
}
