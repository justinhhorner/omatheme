import Foundation
import OmarchyThemesKit

/// Ghostty: a theme file in `$XDG_CONFIG_HOME/ghostty/themes/` (`~/.config/ghostty/themes/`), named
/// after the scheme. The user picks it with `theme = "<name>"` in their config; this never edits
/// the config itself.
public struct GhosttyExporter: TerminalExporter {
    public static let id = "ghostty"
    static let bundleIdentifier = "com.mitchellh.ghostty"

    private let environment: TerminalEnvironment

    public init(environment: TerminalEnvironment) {
        self.environment = environment
    }

    public var id: String { Self.id }
    public var displayName: String { "Ghostty" }

    /// The app, or a Ghostty config folder (for installs Launch Services doesn't know about).
    public var isInstalled: Bool {
        environment.findApp(Self.bundleIdentifier) != nil
            || FileManager.default.fileExists(atPath: environment.configDirectory.appending(path: "ghostty").path)
    }

    public var addHint: String { "Adds these colors to Ghostty as a theme. Your Ghostty config isn't changed." }

    var themesDirectory: URL {
        environment.configDirectory.appending(path: "ghostty/themes", directoryHint: .isDirectory)
    }

    func fileURL(themeName: String) -> URL {
        // The file name is the name Ghostty's `theme =` refers to.
        themesDirectory.appending(path: schemeFileName(forTheme: themeName))
    }

    public func isAdded(slug: String, themeName: String) -> Bool {
        FileManager.default.fileExists(atPath: fileURL(themeName: themeName).path)
    }

    public func add(slug: String, themeName: String, colors: TerminalColors) throws {
        let text = Self.themeFile(name: schemeName(forTheme: themeName), colors: colors)
        try FileManager.default.writeAtomically(Data(text.utf8), to: fileURL(themeName: themeName))
    }

    public func remove(slug: String, themeName: String) throws {
        try FileManager.default.removeItemIfPresent(at: fileURL(themeName: themeName))
    }

    public func addedMessage(scheme: String) -> String {
        "To use it, set theme = \"\(scheme)\" in your Ghostty config, then reload the config (⌘⇧,) or restart Ghostty."
    }

    public func removedMessage(scheme: String) -> String {
        "The “\(scheme)” theme file was deleted. If your Ghostty config still says theme = \"\(scheme)\", change it."
    }

    static func themeFile(name: String, colors: TerminalColors) -> String {
        var lines = [
            "# \(name), added by Omarchy Themes. Use it with: theme = \"\(name)\"",
            "background = \(colors.background.hex)",
            "foreground = \(colors.foreground.hex)",
            "cursor-color = \(colors.cursor.hex)",
            "selection-background = \(colors.selectionBackground.hex)",
            "selection-foreground = \(colors.foreground.hex)",
        ]
        lines += colors.ansi.enumerated().map { "palette = \($0.offset)=\($0.element.hex)" }
        return lines.joined(separator: "\n") + "\n"
    }
}
