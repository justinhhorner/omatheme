import OmarchyThemesKit
import OmarchyThemesStores
import OmarchyThemesMac
import SwiftUI

/// Sends the theme's colors to the terminal picked in the menu (see `TerminalExporter`).
struct TerminalColorsSection: View {
    @Environment(Banners.self) private var banners
    @Environment(ThemeLibrary.self) private var library
    @Environment(TerminalStore.self) private var terminals
    let entry: CatalogEntry
    let palette: Palette

    var body: some View {
        let exporter = terminals.selected
        let isAdded = terminals.isAdded(entry)

        VStack(alignment: .leading, spacing: 10) {
            Text("Terminal Colors").font(.title2.weight(.semibold))
            HStack(spacing: 10) {
                picker(selection: exporter.id)
                if isAdded {
                    Button(exporter.canRemove ? "Remove from \(exporter.displayName)" : "How to Remove…") {
                        showBanner(terminals.remove(entry))
                    }
                    .accessibilityIdentifier("removeFromTerminalButton")
                } else {
                    Button("Add to \(exporter.displayName)") {
                        Task { showBanner(await terminals.add(entry, palette: palette)) }
                    }
                    .disabled(!exporter.isInstalled)
                    .accessibilityIdentifier("addToTerminalButton")
                }
            }
            ansiStrip(TerminalColors(palette))
            Text(hint(exporter, isAdded: isAdded))
                .font(.caption)
                .foregroundStyle(.secondary)
            ContextBanner(context: .terminal(entry.slug))
        }
    }

    private func picker(selection: String) -> some View {
        Picker("Terminal", selection: Binding(get: { selection }, set: { terminals.select($0) })) {
            ForEach(terminals.exporters, id: \.id) { terminal in
                Text(terminal.isInstalled ? terminal.displayName : "\(terminal.displayName) (not installed)")
                    .tag(terminal.id)
            }
        }
        .fixedSize()
        .accessibilityIdentifier("terminalPicker")
    }

    /// The 16 ANSI colors as they'll appear in the terminal.
    private func ansiStrip(_ colors: TerminalColors) -> some View {
        HStack(spacing: 2) {
            ForEach(Array(colors.ansi.enumerated()), id: \.offset) { index, color in
                Rectangle()
                    .fill(Color(color))
                    .frame(width: 18, height: 12)
                    .help("\(index < 8 ? "" : "Bright ")\(TerminalColors.ansiNames[index % 8].lowercased()) \(color.hex.uppercased())")
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 3))
        .accessibilityHidden(true)
    }

    private func hint(_ exporter: any TerminalExporter, isAdded: Bool) -> String {
        if !exporter.isInstalled { return exporter.notInstalledHint }
        guard isAdded else { return exporter.addHint }
        let scheme = exporter.schemeName(forTheme: library.themeName(for: entry))
        return "Available in \(exporter.displayName) as “\(scheme)”."
    }

    private func showBanner(_ banner: Banner) {
        withAnimation { banners[.terminal(entry.slug)] = banner }
    }
}
