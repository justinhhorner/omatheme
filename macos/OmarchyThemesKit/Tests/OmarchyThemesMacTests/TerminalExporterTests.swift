import AppKit
import Foundation
import OmarchyThemesKit
import Testing
@testable import OmarchyThemesMac

/// A fake Mac for exporters: temp folders, a chosen set of installed apps, recorded "open" calls
/// and canned preferences. Nothing touches the real terminals.
final class FakeTerminalMac: @unchecked Sendable {
    let temp = TempFolder()
    private let lock = NSLock()
    private var installed: Set<String>
    private var opened: [(file: URL, app: URL)] = []
    var preferences: [String: Any] = [:]

    init(installed: Set<String>) {
        self.installed = installed
    }

    var openCalls: [(file: URL, app: URL)] { lock.withLock { opened } }

    var environment: TerminalEnvironment {
        TerminalEnvironment(
            homeDirectory: temp.url.appending(path: "home", directoryHint: .isDirectory),
            configDirectory: temp.url.appending(path: "home/.config", directoryHint: .isDirectory),
            exportsDirectory: temp.url.appending(path: "data/terminal", directoryHint: .isDirectory),
            findApp: { id in self.lock.withLock { self.installed.contains(id) } ? URL(filePath: "/Applications/\(id).app") : nil },
            open: { file, app in self.lock.withLock { self.opened.append((file, app)) } },
            readPreference: { key, domain in self.lock.withLock { self.preferences["\(domain)|\(key)"] } })
    }
}

let tokyo = TerminalColors(
    background: RgbColor(hex: "#1a1b26")!, foreground: RgbColor(hex: "#a9b1d6")!, cursor: RgbColor(hex: "#c0caf5")!,
    selectionBackground: RgbColor(hex: "#292e42")!,
    ansi: (0..<16).map { RgbColor(r: UInt8($0 * 16), g: 0x40, b: 0x80) })

struct TerminalExportersTests {
    @Test func registryListsITermFirstAndItIsTheDefault() {
        let exporters = TerminalExporters.all(in: FakeTerminalMac(installed: []).environment)

        #expect(exporters.map(\.id) == ["iterm2", "ghostty", "terminal"])
        #expect(exporters.map(\.displayName) == ["iTerm2", "Ghostty", "Terminal"])
        #expect(TerminalExporters.defaultID == "iterm2")
        #expect(Set(exporters.map(\.id)).count == exporters.count)
    }

    @Test func schemesUseTheSameNameAsWindowsTerminal() {
        let exporter = ITermExporter(environment: FakeTerminalMac(installed: []).environment)
        #expect(exporter.schemeName(forTheme: "Tokyo Night") == "Tokyo Night (Omarchy)")
    }

    @Test func installedFollowsTheApp() {
        let mac = FakeTerminalMac(installed: ["com.googlecode.iterm2"])
        #expect(ITermExporter(environment: mac.environment).isInstalled)
        #expect(!TerminalAppExporter(environment: mac.environment).isInstalled)
    }
}

struct ITermExporterTests {
    let mac = FakeTerminalMac(installed: ["com.googlecode.iterm2"])

    @Test func addWritesADynamicProfileAndRemoveDeletesIt() throws {
        let iterm = ITermExporter(environment: mac.environment)
        #expect(!iterm.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))

        try iterm.add(slug: "omarchy.tokyo-night", themeName: "Tokyo Night", colors: tokyo)

        let file = mac.temp.url.appending(path: "home/Library/Application Support/iTerm2/DynamicProfiles/omarchy-themes-omarchy.tokyo-night.json")
        #expect(FileManager.default.fileExists(atPath: file.path))
        #expect(iterm.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))

        try iterm.remove(slug: "omarchy.tokyo-night", themeName: "Tokyo Night")
        #expect(!iterm.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))
        try iterm.remove(slug: "omarchy.tokyo-night", themeName: "Tokyo Night") // already gone: no error
    }

    @Test func profileHasEveryColorAsSRGBComponents() throws {
        let data = try ITermExporter.profileJSON(slug: "tokyo", name: "Tokyo Night (Omarchy)", colors: tokyo)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let profile = try #require((json?["Profiles"] as? [[String: Any]])?.first)

        #expect(profile["Name"] as? String == "Tokyo Night (Omarchy)")
        #expect(profile["Guid"] as? String == "omarchy-themes-tokyo")
        for key in (0..<16).map({ "Ansi \($0) Color" }) + ["Background Color", "Foreground Color", "Cursor Color", "Selection Color"] {
            #expect(profile[key] != nil, "missing \(key)")
        }
        let background = try #require(profile["Background Color"] as? [String: Any])
        #expect(background["Color Space"] as? String == "sRGB")
        #expect(abs((background["Red Component"] as? Double ?? -1) - 0x1a / 255.0) < 0.0001)
        #expect(abs((background["Blue Component"] as? Double ?? -1) - 0x26 / 255.0) < 0.0001)
        let ansi5 = try #require(profile["Ansi 5 Color"] as? [String: Any])
        #expect(abs((ansi5["Red Component"] as? Double ?? -1) - 80 / 255.0) < 0.0001)
    }
}

struct GhosttyExporterTests {
    @Test func addWritesAThemeFileNamedAfterTheScheme() throws {
        let mac = FakeTerminalMac(installed: ["com.mitchellh.ghostty"])
        let ghostty = GhosttyExporter(environment: mac.environment)

        try ghostty.add(slug: "omarchy.tokyo-night", themeName: "Tokyo Night", colors: tokyo)

        let file = mac.temp.url.appending(path: "home/.config/ghostty/themes/Tokyo Night (Omarchy)")
        let text = try String(contentsOf: file, encoding: .utf8)
        let lines = text.split(separator: "\n").map(String.init)
        #expect(lines.contains("background = #1a1b26"))
        #expect(lines.contains("foreground = #a9b1d6"))
        #expect(lines.contains("cursor-color = #c0caf5"))
        #expect(lines.contains("selection-background = #292e42"))
        #expect(lines.contains("palette = 0=#004080"))
        #expect(lines.contains("palette = 15=#f04080"))
        #expect(lines.filter { $0.hasPrefix("palette = ") }.count == 16)
        #expect(ghostty.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))

        try ghostty.remove(slug: "omarchy.tokyo-night", themeName: "Tokyo Night")
        #expect(!FileManager.default.fileExists(atPath: file.path))
    }

    let mac = FakeTerminalMac(installed: ["com.mitchellh.ghostty"])
    var ghostty: GhosttyExporter { GhosttyExporter(environment: mac.environment) }
    var themes: URL { mac.temp.url.appending(path: "home/.config/ghostty/themes") }

    @Test func aRenamedThemeFindsAndReplacesItsFileUnderTheOldName() throws {
        try ghostty.add(slug: "nord", themeName: "Nord", colors: tokyo)

        #expect(ghostty.isAdded(slug: "nord", themeName: "Nord Deep"))

        try ghostty.add(slug: "nord", themeName: "Nord Deep", colors: tokyo)
        #expect(try FileManager.default.contentsOfDirectory(atPath: themes.path) == ["Nord Deep (Omarchy)"])

        try ghostty.remove(slug: "nord", themeName: "Nord Deep")
        #expect(try FileManager.default.contentsOfDirectory(atPath: themes.path).isEmpty)
    }

    @Test func anotherThemeWithTheSameNameIsNotOverwritten() throws {
        try ghostty.add(slug: "omarchy.nord", themeName: "Nord", colors: tokyo)

        #expect(!ghostty.isAdded(slug: "nord", themeName: "Nord"))
        #expect(throws: GhosttyThemeNameTakenError.self) { try ghostty.add(slug: "nord", themeName: "Nord", colors: tokyo) }
        try ghostty.remove(slug: "nord", themeName: "Nord")

        #expect(GhosttyExporter.slug(in: themes.appending(path: "Nord (Omarchy)")) == "omarchy.nord")
    }

    @Test func untaggedFilesFromEarlierVersionsStillCount() throws {
        let old = themes.appending(path: "Nord (Omarchy)")
        try FileManager.default.writeAtomically(Data("# Nord (Omarchy), added by Omarchy Themes.\nbackground = #2e3440\n".utf8), to: old)
        try FileManager.default.writeAtomically(Data("background = #000000\n".utf8), to: themes.appending(path: "My Own"))

        #expect(ghostty.isAdded(slug: "nord", themeName: "Nord"))

        try ghostty.remove(slug: "nord", themeName: "Nord")
        #expect(try FileManager.default.contentsOfDirectory(atPath: themes.path) == ["My Own"])
    }

    @Test func aConfigFolderCountsAsInstalled() throws {
        let mac = FakeTerminalMac(installed: [])
        let ghostty = GhosttyExporter(environment: mac.environment)
        #expect(!ghostty.isInstalled)

        try FileManager.default.createDirectory(at: mac.environment.configDirectory.appending(path: "ghostty"), withIntermediateDirectories: true)
        #expect(ghostty.isInstalled)
    }
}

struct TerminalAppExporterTests {
    let mac = FakeTerminalMac(installed: ["com.apple.Terminal"])

    @Test func addWritesAProfileAndOpensItWithTerminal() throws {
        let terminal = TerminalAppExporter(environment: mac.environment)

        try terminal.add(slug: "omarchy.tokyo-night", themeName: "Tokyo Night", colors: tokyo)

        let call = try #require(mac.openCalls.first)
        #expect(call.file.lastPathComponent == "Tokyo Night (Omarchy).terminal")
        #expect(call.file.deletingLastPathComponent().lastPathComponent == "terminal")
        #expect(call.app.lastPathComponent == "com.apple.Terminal.app")

        let plist = try #require(try PropertyListSerialization.propertyList(from: Data(contentsOf: call.file), format: nil) as? [String: Any])
        #expect(plist["name"] as? String == "Tokyo Night (Omarchy)")
        #expect(plist["type"] as? String == "Window Settings")
        for key in TerminalAppExporter.ansiKeys + ["BackgroundColor", "TextColor", "CursorColor", "SelectionColor"] {
            #expect(plist[key] is Data, "missing \(key)")
        }
        let data = try #require(plist["BackgroundColor"] as? Data)
        let color = try #require(try NSKeyedUnarchiver.unarchivedObject(ofClass: NSColor.self, from: data)?.usingColorSpace(.sRGB))
        #expect(Int((color.redComponent * 255).rounded()) == 0x1a)
        #expect(Int((color.blueComponent * 255).rounded()) == 0x26)
    }

    @Test func isAddedReadsTerminalsProfilesAndRemovalIsManual() throws {
        let terminal = TerminalAppExporter(environment: mac.environment)
        #expect(!terminal.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))

        mac.preferences["com.apple.Terminal|Window Settings"] = ["Basic": [:], "Tokyo Night (Omarchy)": [:]]

        #expect(terminal.isAdded(slug: "omarchy.tokyo-night", themeName: "Tokyo Night"))
        #expect(!terminal.canRemove)
        #expect(throws: TerminalExportUnsupportedError.self) { try terminal.remove(slug: "omarchy.tokyo-night", themeName: "Tokyo Night") }
        #expect(terminal.removeInstructions(scheme: "Tokyo Night (Omarchy)").contains("Settings › Profiles"))
    }

    @Test func addFailsWhenTerminalIsMissing() {
        let terminal = TerminalAppExporter(environment: FakeTerminalMac(installed: []).environment)
        #expect(throws: TerminalExportUnsupportedError.self) { try terminal.add(slug: "t", themeName: "T", colors: tokyo) }
    }
}
