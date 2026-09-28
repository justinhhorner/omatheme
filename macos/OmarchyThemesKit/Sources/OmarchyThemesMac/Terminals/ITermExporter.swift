import Foundation
import OmarchyThemesKit

/// iTerm2: a Dynamic Profile, one JSON file per theme in
/// `~/Library/Application Support/iTerm2/DynamicProfiles/`. iTerm2 watches that folder, so the
/// profile appears (and disappears on Remove) without a restart.
public struct ITermExporter: TerminalExporter {
    public static let id = "iterm2"
    static let bundleIdentifier = "com.googlecode.iterm2"

    private let environment: TerminalEnvironment

    public init(environment: TerminalEnvironment) {
        self.environment = environment
    }

    public var id: String { Self.id }
    public var displayName: String { "iTerm2" }
    public var isInstalled: Bool { environment.findApp(Self.bundleIdentifier) != nil }
    public var addHint: String { "Adds these colors to iTerm2 as a new profile. Your iTerm2 settings aren't changed." }

    var profilesDirectory: URL {
        environment.homeDirectory.appending(path: "Library/Application Support/iTerm2/DynamicProfiles", directoryHint: .isDirectory)
    }

    func fileURL(slug: String) -> URL {
        profilesDirectory.appending(path: "omarchy-themes-\(slug).json")
    }

    public func isAdded(slug: String, themeName: String) -> Bool {
        FileManager.default.fileExists(atPath: fileURL(slug: slug).path)
    }

    public func add(slug: String, themeName: String, colors: TerminalColors) throws {
        let profile = try Self.profileJSON(slug: slug, name: schemeName(forTheme: themeName), colors: colors)
        try FileManager.default.writeAtomically(profile, to: fileURL(slug: slug))
    }

    public func remove(slug: String, themeName: String) throws {
        try FileManager.default.removeItemIfPresent(at: fileURL(slug: slug))
    }

    public func addedMessage(scheme: String) -> String {
        "“\(scheme)” is now an iTerm2 profile. In iTerm2, open Settings › Profiles and pick it for a new window, "
            + "or choose Other Actions › Set as Default to use it everywhere."
    }

    public func removedMessage(scheme: String) -> String {
        "“\(scheme)” is no longer one of iTerm2's profiles."
    }

    static func profileJSON(slug: String, name: String, colors: TerminalColors) throws -> Data {
        var profile: [String: Any] = [
            "Name": name,
            // Stable, so re-adding a theme updates its profile instead of making a second one.
            "Guid": "omarchy-themes-\(slug)",
            "Background Color": component(colors.background),
            "Foreground Color": component(colors.foreground),
            "Bold Color": component(colors.foreground),
            "Cursor Color": component(colors.cursor),
            "Cursor Text Color": component(colors.background),
            "Selection Color": component(colors.selectionBackground),
            "Selected Text Color": component(colors.foreground),
        ]
        for (index, color) in colors.ansi.enumerated() {
            profile["Ansi \(index) Color"] = component(color)
        }
        return try JSONSerialization.data(withJSONObject: ["Profiles": [profile]], options: [.prettyPrinted, .sortedKeys])
    }

    private static func component(_ color: RgbColor) -> [String: Any] {
        let c = color.unitComponents
        return [
            "Red Component": c.red, "Green Component": c.green, "Blue Component": c.blue,
            "Alpha Component": 1, "Color Space": "sRGB",
        ]
    }
}
