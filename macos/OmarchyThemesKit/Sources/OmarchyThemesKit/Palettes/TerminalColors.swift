import Foundation

/// A theme's colors as a terminal color scheme: background, foreground, cursor, selection and the
/// 16 ANSI colors. For Omarchy's named colors.toml this follows Omarchy's own terminal template
/// (default/themed/alacritty.toml.tpl): black = background, white = foreground, bright black = muted,
/// bright white and the cursor = bright_foreground. Themes that ship the 16 colors (color0..15 or
/// alacritty.toml) use them as-is. Missing bright colors fall back to their normal ones.
public struct TerminalColors: Hashable, Sendable {
    public var background: RgbColor
    public var foreground: RgbColor
    public var cursor: RgbColor
    public var selectionBackground: RgbColor
    /// 16 colors: black, red, green, yellow, blue, magenta, cyan, white, then the bright ones.
    public var ansi: [RgbColor]

    public static let ansiNames = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"]

    public init(background: RgbColor, foreground: RgbColor, cursor: RgbColor, selectionBackground: RgbColor, ansi: [RgbColor]) {
        self.background = background
        self.foreground = foreground
        self.cursor = cursor
        self.selectionBackground = selectionBackground
        self.ansi = ansi
    }

    public init(_ palette: Palette) {
        var swatches: [String: RgbColor] = [:]
        for swatch in palette.swatches where swatches[swatch.name.lowercased()] == nil {
            swatches[swatch.name.lowercased()] = swatch.color
        }
        func swatch(_ name: String) -> RgbColor? { swatches[name.lowercased()] }

        // A neutral between background and foreground, for themes with neither muted nor a bright black.
        let between = Self.mix(palette.background, palette.foreground, 0.35)

        let normal = Self.ansiNames.map { name -> RgbColor in
            switch name {
            case "Black": swatch("Black") ?? palette.background
            case "White": swatch("White") ?? palette.foreground
            default: swatch(name) ?? palette.foreground
            }
        }
        let bright = Self.ansiNames.enumerated().map { i, name -> RgbColor in
            switch name {
            case "Black": swatch("Bright black") ?? palette.muted ?? between
            case "White": swatch("Bright white") ?? palette.brightForeground ?? normal[i]
            default: swatch("Bright " + name.lowercased()) ?? normal[i]
            }
        }

        self.init(
            background: palette.background,
            foreground: palette.foreground,
            cursor: palette.cursor ?? palette.brightForeground ?? palette.foreground,
            selectionBackground: palette.selection ?? Self.mix(palette.background, palette.foreground, 0.2),
            ansi: normal + bright)
    }

    private static func mix(_ a: RgbColor, _ b: RgbColor, _ t: Double) -> RgbColor {
        // Halves round to even, like .NET's Math.Round, so both apps export identical colors.
        func channel(_ x: UInt8, _ y: UInt8) -> UInt8 {
            UInt8((Double(x) + (Double(y) - Double(x)) * t).rounded(.toNearestOrEven))
        }
        return RgbColor(r: channel(a.r, b.r), g: channel(a.g, b.g), b: channel(a.b, b.b))
    }
}
