import AppKit
import OmarchyThemesKit

/// Terminal.app has no folder of add-on profiles. The exporter writes a `.terminal` profile (in the
/// app's data folder) and opens it with Terminal, which imports it as a profile and opens a window
/// with it. Taking it back out would mean editing Terminal's settings, which exporters never do, so
/// removal is left to the user (Terminal › Settings › Profiles).
public struct TerminalAppExporter: TerminalExporter {
    public static let id = "terminal"
    static let bundleIdentifier = "com.apple.Terminal"

    private let environment: TerminalEnvironment

    public init(environment: TerminalEnvironment) {
        self.environment = environment
    }

    public var id: String { Self.id }
    public var displayName: String { "Terminal" }
    public var isInstalled: Bool { environment.findApp(Self.bundleIdentifier) != nil }
    public var canRemove: Bool { false }
    public var addHint: String { "Opens these colors in Terminal, which adds them as a new profile. Your other profiles aren't changed." }

    func fileURL(themeName: String) -> URL {
        environment.exportsDirectory.appending(path: "\(schemeName(forTheme: themeName).replacing("/", with: "-")).terminal")
    }

    /// Read-only look at Terminal's own profile list.
    public func isAdded(slug: String, themeName: String) -> Bool {
        let profiles = environment.readPreference("Window Settings", Self.bundleIdentifier) as? [String: Any]
        return profiles?[schemeName(forTheme: themeName)] != nil
    }

    public func add(slug: String, themeName: String, colors: TerminalColors) throws {
        guard let app = environment.findApp(Self.bundleIdentifier) else {
            throw TerminalExportUnsupportedError(message: "Terminal isn't installed.")
        }
        let file = fileURL(themeName: themeName)
        try ExportFiles.write(try Self.profile(name: schemeName(forTheme: themeName), colors: colors), to: file)
        try environment.open(file, app)
    }

    public func remove(slug: String, themeName: String) throws {
        throw TerminalExportUnsupportedError(message: removeInstructions(scheme: schemeName(forTheme: themeName)))
    }

    public func addedMessage(scheme: String) -> String {
        "Terminal opened a window with “\(scheme)” and added it to its profiles. To use it for every new window, open Terminal › Settings › Profiles, select it and click Default."
    }

    public func removedMessage(scheme: String) -> String { "" }

    public func removeInstructions(scheme: String) -> String {
        "Terminal profiles can only be removed in Terminal: open Terminal › Settings › Profiles, select “\(scheme)” and click the − button."
    }

    /// Terminal's keys for the 16 ANSI colors, in order.
    static let ansiKeys = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"].map { "ANSI\($0)Color" }
        + ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"].map { "ANSIBright\($0)Color" }

    /// A `.terminal` file: a property list whose colors are keyed-archived NSColors.
    static func profile(name: String, colors: TerminalColors) throws -> Data {
        var plist: [String: Any] = [
            "name": name,
            "type": "Window Settings",
            "ProfileCurrentVersion": 2.07,
            "BackgroundColor": try archive(colors.background),
            "TextColor": try archive(colors.foreground),
            "TextBoldColor": try archive(colors.foreground),
            "CursorColor": try archive(colors.cursor),
            "SelectionColor": try archive(colors.selectionBackground),
        ]
        for (key, color) in zip(ansiKeys, colors.ansi) {
            plist[key] = try archive(color)
        }
        return try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
    }

    private static func archive(_ color: RgbColor) throws -> Data {
        let c = color.unitComponents
        let nsColor = NSColor(srgbRed: c.red, green: c.green, blue: c.blue, alpha: 1)
        return try NSKeyedArchiver.archivedData(withRootObject: nsColor, requiringSecureCoding: true)
    }
}
