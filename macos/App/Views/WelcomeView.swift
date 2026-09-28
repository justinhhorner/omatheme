import OmarchyThemesStores
import SwiftUI

/// Shown on first launch (and from Settings or the Help menu).
struct WelcomeView: View {
    @Environment(Preferences.self) private var preferences

    /// Markdown, for the links.
    private let credits: LocalizedStringKey = """
        Omarchy is made by DHH and contributors at [omarchy.org](https://omarchy.org) and \
        [basecamp/omarchy](https://github.com/basecamp/omarchy). This app isn't affiliated with Omarchy or 37signals, \
        and every theme belongs to its author.
        """

    var body: some View {
        VStack(spacing: 22) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 96, height: 96)
                .accessibilityHidden(true)

            VStack(spacing: 8) {
                Text("Welcome to Omarchy Themes")
                    .font(.largeTitle.weight(.bold))
                Text("Bring Omarchy's themes to your Mac.")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 14) {
                Feature(
                    symbol: "square.grid.2x2",
                    title: "Browse the gallery",
                    text: "See the themes that come with Omarchy and every community theme from omarchy.org/themes, "
                        + "with their colors and wallpapers.")
                Feature(
                    symbol: "arrow.down.circle",
                    title: "Download what you like",
                    text: "Downloaded themes are kept on this Mac and work offline.")
                Feature(
                    symbol: "photo.on.rectangle",
                    title: "Make it your desktop",
                    text: "Set a theme's wallpaper as your desktop picture. Nothing changes until you apply a theme, "
                        + "and your current desktop is saved first so you can restore it.")
            }
            .frame(maxWidth: 420)

            Text(credits)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 420)

            Button("Browse Themes") { preferences.dismissWelcome() }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .keyboardShortcut(.defaultAction)
                .accessibilityIdentifier("browseThemesButton")
        }
        .padding(36)
        .frame(width: 540)
    }
}

private struct Feature: View {
    let symbol: String
    let title: String
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: symbol)
                .font(.title2)
                .foregroundStyle(.tint)
                .frame(width: 30)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(text)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
