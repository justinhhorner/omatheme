import Foundation

/// Omarchy's colors.toml. Two shapes exist in the wild:
/// - Terminal-style: accent, cursor, foreground, background, selection_*, color0..color15.
/// - Named (newer Omarchy): mode, accent, background, foreground, red, bright_red, … .
///
/// Both are accepted, and a file mixing them is fine.
public enum ColorsTomlParser {
    private static let ansiNames = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"]

    private static let namedKeys = [
        "red", "orange", "yellow", "green", "cyan", "blue", "magenta", "brown",
        "bright_red", "bright_yellow", "bright_green", "bright_cyan", "bright_blue", "bright_magenta",
    ]

    public static func parse(_ text: String) throws -> Palette {
        let values = FlatToml.parse(text)
        let get = colorLookup(values)

        guard let background = get("background") else { throw PaletteParseError(message: "colors.toml has no valid 'background' color.") }
        guard let foreground = get("foreground") else { throw PaletteParseError(message: "colors.toml has no valid 'foreground' color.") }

        var swatches: [NamedColor] = (0..<16).compactMap { i in
            get("color\(i)").map { NamedColor(name: i < 8 ? ansiNames[i] : "Bright " + ansiNames[i - 8].lowercased(), color: $0) }
        }
        if swatches.isEmpty {
            swatches = namedKeys.compactMap { key in get(key).map { NamedColor(name: humanize(key), color: $0) } }
        }

        return Palette(
            background: background,
            foreground: foreground,
            accent: get("accent") ?? get("color4") ?? get("blue") ?? foreground,
            cursor: get("cursor"),
            selection: get("selection_background") ?? get("selection"),
            muted: get("muted"),
            brightForeground: get("bright_foreground"),
            declaredMode: parseMode(values["mode"]),
            swatches: swatches,
            source: .colorsToml)
    }

    static func parseMode(_ mode: String?) -> AppearanceMode? {
        switch mode?.trimmingCharacters(in: .whitespaces).lowercased() {
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }

    static func colorLookup(_ values: [String: String]) -> (String) -> RgbColor? {
        { key in values[key.lowercased()].flatMap(RgbColor.init(hex:)) }
    }

    private static func humanize(_ key: String) -> String {
        key.prefix(1).uppercased() + key.dropFirst().replacing("_", with: " ")
    }
}

/// alacritty.toml, the fallback for themes that predate colors.toml.
public enum AlacrittyParser {
    private static let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]

    public static func parse(_ text: String) throws -> Palette {
        let values = FlatToml.parse(text)
        let get = ColorsTomlParser.colorLookup(values)

        guard let background = get("colors.primary.background") else {
            throw PaletteParseError(message: "alacritty.toml has no valid [colors.primary] background.")
        }
        guard let foreground = get("colors.primary.foreground") else {
            throw PaletteParseError(message: "alacritty.toml has no valid [colors.primary] foreground.")
        }

        var swatches: [NamedColor] = []
        for (group, label) in [("normal", ""), ("bright", "Bright ")] {
            for name in names {
                if let c = get("colors.\(group).\(name)") {
                    swatches.append(NamedColor(name: label.isEmpty ? name.prefix(1).uppercased() + name.dropFirst() : label + name, color: c))
                }
            }
        }

        return Palette(
            background: background,
            foreground: foreground,
            accent: get("colors.normal.blue") ?? foreground,
            cursor: get("colors.cursor.cursor"),
            selection: get("colors.selection.background"),
            swatches: swatches,
            source: .alacritty)
    }
}
