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

    /// The theme's files. Each file is named after the theme, so it also says which theme it's for
    /// (`slugLinePrefix`): a theme renamed upstream still finds its file under the old name, and two
    /// themes with the same name are told apart. An untagged file under the theme's name (written
    /// before files were tagged) counts too. Only "… (Omarchy)" files are read.
    func files(slug: String, themeName: String) -> [URL] {
        let ownName = schemeFileName(forTheme: themeName)
        let ourSuffix = schemeFileName(forTheme: "") // " (Omarchy)"
        let names = (try? FileManager.default.contentsOfDirectory(atPath: themesDirectory.path)) ?? []
        return names.filter { $0.hasSuffix(ourSuffix) }.compactMap { name in
            let file = themesDirectory.appending(path: name)
            switch Self.slug(in: file) {
            case slug?: return file
            case nil where name == ownName: return file
            default: return nil
            }
        }
    }

    public func isAdded(slug: String, themeName: String) -> Bool {
        !files(slug: slug, themeName: themeName).isEmpty
    }

    public func add(slug: String, themeName: String, colors: TerminalColors) throws {
        let file = fileURL(themeName: themeName)
        let scheme = schemeName(forTheme: themeName)
        if let owner = Self.slug(in: file), owner != slug {
            throw GhosttyThemeNameTakenError(scheme: scheme)
        }
        // A theme renamed since it was added: its file under the old name goes.
        for old in files(slug: slug, themeName: themeName) where old != file {
            try FileManager.default.removeItemIfPresent(at: old)
        }
        let text = Self.themeFile(name: scheme, slug: slug, colors: colors)
        try FileManager.default.writeAtomically(Data(text.utf8), to: file)
    }

    public func remove(slug: String, themeName: String) throws {
        for file in files(slug: slug, themeName: themeName) {
            try FileManager.default.removeItemIfPresent(at: file)
        }
    }

    public func addedMessage(scheme: String) -> String {
        "To use it, set theme = \"\(scheme)\" in your Ghostty config, then reload the config (⌘⇧,) or restart Ghostty."
    }

    public func removedMessage(scheme: String) -> String {
        "The “\(scheme)” theme file was deleted. If your Ghostty config still says theme = \"\(scheme)\", change it."
    }

    /// The comment line that says which theme a file is for.
    static let slugLinePrefix = "# omarchy-themes-slug: "

    /// The slug a theme file is tagged with, or nil (missing, unreadable, untagged or not ours).
    static func slug(in file: URL) -> String? {
        guard let text = try? String(contentsOf: file, encoding: .utf8) else { return nil }
        return text.split(whereSeparator: \.isNewline)
            .first { $0.hasPrefix(slugLinePrefix) }
            .map { String($0.dropFirst(slugLinePrefix.count)) }
    }

    static func themeFile(name: String, slug: String, colors: TerminalColors) -> String {
        var lines = [
            "# \(name), added by Omarchy Themes. Use it with: theme = \"\(name)\"",
            slugLinePrefix + slug,
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

/// Another theme with the same name already has the Ghostty theme file this one would use.
public struct GhosttyThemeNameTakenError: LocalizedError, Sendable {
    public let scheme: String

    public var errorDescription: String? {
        "Ghostty already has a “\(scheme)” theme, added for another theme with the same name. "
            + "Remove that one first (from its page), then try again."
    }
}
