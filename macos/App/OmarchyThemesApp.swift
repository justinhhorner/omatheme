import AppKit
import SwiftUI

@main
struct OmarchyThemesApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model = AppModel()

    var body: some Scene {
        Window("Omarchy Themes", id: WindowID.main) {
            ContentView()
                .environment(model)
                .frame(minWidth: 820, minHeight: 540)
        }
        .defaultSize(width: 1180, height: 800)
        .commands { AppCommands(model: model) }

        Settings {
            SettingsView()
                .environment(model)
        }
    }
}

enum WindowID {
    static let main = "main"
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

struct AppCommands: Commands {
    let model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            Button("About Omarchy Themes") { AboutPanel.show() }
        }
        CommandGroup(replacing: .newItem) {}

        CommandGroup(before: .sidebar) {
            Button("Gallery") { model.sidebarSelection = .gallery }
                .keyboardShortcut("1")
            Button("Downloaded") { model.sidebarSelection = .downloaded }
                .keyboardShortcut("2")
            Divider()
            Button("Refresh Themes") { Task { await model.refreshCatalog() } }
                .keyboardShortcut("r")
                .disabled(model.isRefreshing)
            Divider()
        }

        CommandGroup(replacing: .help) {
            Button("Show Welcome Screen") {
                model.showWelcome = true
                openWindow(id: WindowID.main)
            }
            Divider()
            Link("Omarchy Theme Gallery", destination: URL(string: "https://omarchy.org/themes/")!)
            Link("Omarchy on GitHub", destination: URL(string: "https://github.com/basecamp/omarchy")!)
        }
    }
}

/// The standard About panel, with credits for Omarchy.
@MainActor
enum AboutPanel {
    private static let links = [
        ("omarchy.org", "https://omarchy.org"),
        ("Theme gallery", "https://omarchy.org/themes/"),
        ("basecamp/omarchy", "https://github.com/basecamp/omarchy"),
    ]

    static func show() {
        NSApp.orderFrontStandardAboutPanel(options: [.credits: credits()])
        NSApp.activate()
    }

    private static func credits() -> NSAttributedString {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        let base: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize),
            .foregroundColor: NSColor.secondaryLabelColor,
            .paragraphStyle: paragraph,
        ]
        func append(_ text: String, to credits: NSMutableAttributedString, link: String? = nil) {
            var attributes = base
            if let link { attributes[.link] = URL(string: link)! }
            credits.append(NSAttributedString(string: text, attributes: attributes))
        }

        let credits = NSMutableAttributedString()
        append("A companion for the themes that ship with Omarchy and its community theme gallery.\n\n", to: credits)
        append("Omarchy is by DHH and contributors:\n", to: credits)
        for (index, (title, url)) in links.enumerated() {
            append(title, to: credits, link: url)
            append(index == links.count - 1 ? "\n" : " · ", to: credits)
        }
        append("\nNot affiliated with Omarchy or 37signals. Every theme belongs to its author.", to: credits)
        return credits
    }
}
