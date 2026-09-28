import Foundation

public enum AppearanceMode: String, Codable, Sendable, CaseIterable {
    case dark
    case light
}

/// Which file in the theme repo the palette was read from.
public enum PaletteSource: String, Codable, Sendable {
    case colorsToml
    case alacritty
}

public struct NamedColor: Hashable, Sendable, Codable {
    public let name: String
    public let color: RgbColor

    public init(name: String, color: RgbColor) {
        self.name = name
        self.color = color
    }
}

/// A theme's colors, normalized from whichever file format the theme ships.
public struct Palette: Hashable, Sendable, Codable {
    public var background: RgbColor
    public var foreground: RgbColor
    public var accent: RgbColor
    public var cursor: RgbColor?
    public var selection: RgbColor?

    /// Named colors.toml `muted` (Omarchy uses it as the terminal's bright black).
    public var muted: RgbColor?

    /// Named colors.toml `bright_foreground` (the terminal's bright white and cursor).
    public var brightForeground: RgbColor?

    /// Mode stated by the theme itself (colors.toml `mode` or a light.mode file).
    public var declaredMode: AppearanceMode?

    /// Terminal-style colors for display, in a stable order.
    public var swatches: [NamedColor]

    public var source: PaletteSource

    public init(
        background: RgbColor, foreground: RgbColor, accent: RgbColor, cursor: RgbColor? = nil, selection: RgbColor? = nil,
        muted: RgbColor? = nil, brightForeground: RgbColor? = nil, declaredMode: AppearanceMode? = nil, swatches: [NamedColor] = [], source: PaletteSource
    ) {
        self.background = background
        self.foreground = foreground
        self.accent = accent
        self.cursor = cursor
        self.selection = selection
        self.muted = muted
        self.brightForeground = brightForeground
        self.declaredMode = declaredMode
        self.swatches = swatches
        self.source = source
    }

    /// The declared mode, otherwise inferred from the background's luminance.
    public var mode: AppearanceMode {
        declaredMode ?? (background.isLight ? .light : .dark)
    }
}

public struct PaletteParseError: LocalizedError, Sendable {
    public let message: String
    public var errorDescription: String? { message }
}
