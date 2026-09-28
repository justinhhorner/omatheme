import AppKit
import OmarchyThemesKit

/// A terminal app that a theme's colors can be sent to, as a color scheme or profile of its own.
/// Exporters only add their own files; they never edit the user's terminal settings.
///
/// To support another terminal, add a type conforming to this protocol (see `ITermExporter` for a
/// small one) and list it in `TerminalExporters.all(in:)`. Everything outside the app (folders,
/// finding and opening apps, reading another app's settings) goes through `TerminalEnvironment`,
/// so an exporter is tested against temp folders and fakes.
public protocol TerminalExporter: Sendable {
    /// Stable identifier, saved in settings (e.g. "iterm2").
    var id: String { get }

    /// The app's name in the UI (e.g. "iTerm2").
    var displayName: String { get }

    /// Whether the terminal is on this Mac; the UI disables Add when it isn't.
    var isInstalled: Bool { get }

    /// Whether `remove` can take back what `add` did. When false, `removeInstructions` says how.
    var canRemove: Bool { get }

    /// One line under the button before the theme is added.
    var addHint: String { get }

    /// The name the theme's colors appear under in the terminal.
    func schemeName(forTheme themeName: String) -> String

    func isAdded(slug: String, themeName: String) -> Bool

    func add(slug: String, themeName: String, colors: TerminalColors) throws

    func remove(slug: String, themeName: String) throws

    /// What to do next after adding, e.g. where to pick the scheme.
    func addedMessage(scheme: String) -> String

    func removedMessage(scheme: String) -> String

    /// How to remove the scheme by hand, for terminals where `canRemove` is false.
    func removeInstructions(scheme: String) -> String
}

extension TerminalExporter {
    /// "Tokyo Night (Omarchy)", the same name the Windows app uses in Windows Terminal.
    public func schemeName(forTheme themeName: String) -> String {
        "\(themeName) (Omarchy)"
    }

    public var canRemove: Bool { true }

    public var notInstalledHint: String {
        "Install \(displayName) to use these colors in it."
    }

    public func removeInstructions(scheme: String) -> String { "" }
}

public struct TerminalExportUnsupportedError: LocalizedError, Sendable {
    public let message: String
    public var errorDescription: String? { message }
}

/// Everything the exporters need from outside the app.
public struct TerminalEnvironment: Sendable {
    public var homeDirectory: URL
    /// `$XDG_CONFIG_HOME`, or `~/.config` when unset.
    public var configDirectory: URL
    /// Where exporters that hand a file to the terminal keep it (in the app's data folder).
    public var exportsDirectory: URL
    /// The app with this bundle identifier, if installed.
    public var findApp: @Sendable (_ bundleIdentifier: String) -> URL?
    /// Opens `file` with the app at `app`.
    public var open: @Sendable (_ file: URL, _ app: URL) throws -> Void
    /// Reads a value from another app's preferences (read-only).
    public var readPreference: @Sendable (_ key: String, _ domain: String) -> Any?

    public init(
        homeDirectory: URL, configDirectory: URL, exportsDirectory: URL,
        findApp: @escaping @Sendable (String) -> URL?,
        open: @escaping @Sendable (URL, URL) throws -> Void,
        readPreference: @escaping @Sendable (String, String) -> Any?
    ) {
        self.homeDirectory = homeDirectory
        self.configDirectory = configDirectory
        self.exportsDirectory = exportsDirectory
        self.findApp = findApp
        self.open = open
        self.readPreference = readPreference
    }

    /// The real Mac. `exportsDirectory` is inside the app's data folder.
    public static func live(exportsDirectory: URL, environment: [String: String] = ProcessInfo.processInfo.environment) -> TerminalEnvironment {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let config = environment["XDG_CONFIG_HOME"].flatMap { $0.isEmpty ? nil : URL(filePath: $0, directoryHint: .isDirectory) }
            ?? home.appending(path: ".config", directoryHint: .isDirectory)
        return TerminalEnvironment(
            homeDirectory: home,
            configDirectory: config,
            exportsDirectory: exportsDirectory,
            findApp: { NSWorkspace.shared.urlForApplication(withBundleIdentifier: $0) },
            open: { file, app in
                NSWorkspace.shared.open([file], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration())
            },
            readPreference: { key, domain in CFPreferencesCopyAppValue(key as CFString, domain as CFString) })
    }
}

/// The supported terminals.
public enum TerminalExporters {
    /// Selected in the UI until the user picks another.
    public static let defaultID = ITermExporter.id

    /// Every supported terminal, in picker order. Register new exporters here.
    public static func all(in environment: TerminalEnvironment) -> [any TerminalExporter] {
        [
            ITermExporter(environment: environment),
            GhosttyExporter(environment: environment),
            TerminalAppExporter(environment: environment),
        ]
    }
}

// MARK: - Helpers shared by exporters

extension RgbColor {
    /// Components in 0...1.
    var unitComponents: (red: Double, green: Double, blue: Double) {
        (Double(r) / 255, Double(g) / 255, Double(b) / 255)
    }
}

enum ExportFiles {
    /// Writes `data` atomically, creating the folder if needed.
    static func write(_ data: Data, to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url, options: .atomic)
    }

    static func removeIfPresent(_ url: URL) throws {
        if FileManager.default.fileExists(atPath: url.path) {
            try FileManager.default.removeItem(at: url)
        }
    }
}
