import Foundation
import Observation
import OmarchyThemesKit
import OmarchyThemesMac

/// Sending a theme's colors to a terminal app (see `TerminalExporters`).
@MainActor
@Observable
public final class TerminalStore {
    public let exporters: [any TerminalExporter]

    private let preferences: Preferences
    private let library: ThemeLibrary
    /// Bumped after adding or removing, so views re-read `isAdded`.
    private var revision = 0

    public init(exporters: [any TerminalExporter], preferences: Preferences, library: ThemeLibrary) {
        precondition(!exporters.isEmpty, "At least one terminal exporter is needed.")
        self.exporters = exporters
        self.preferences = preferences
        self.library = library
    }

    /// The terminal picked on theme pages (iTerm2 until the user picks another).
    public var selected: any TerminalExporter {
        exporters.first { $0.id == preferences.settings.terminalApp }
            ?? exporters.first { $0.id == TerminalExporters.defaultID }
            ?? exporters[0]
    }

    public func select(_ id: String) {
        preferences.update { $0.terminalApp = id }
    }

    /// Whether the selected terminal already has the theme's colors.
    public func isAdded(_ entry: CatalogEntry) -> Bool {
        _ = revision // Observed, so views ask again after an add or remove.
        return selected.isAdded(slug: entry.slug, themeName: library.themeName(for: entry))
    }

    public func add(_ entry: CatalogEntry, palette: Palette) async -> Banner {
        let exporter = selected
        let name = library.themeName(for: entry)
        let scheme = exporter.schemeName(forTheme: name)
        let colors = TerminalColors(await paletteForTerminal(entry, saved: palette))
        do {
            try exporter.add(slug: entry.slug, themeName: name, colors: colors)
            revision += 1
            // Terminal.app imports the profile after it opens the file.
            Task {
                try? await Task.sleep(for: .seconds(2))
                revision += 1
            }
            return Banner(kind: .success, title: "Added to \(exporter.displayName)", message: exporter.addedMessage(scheme: scheme))
        } catch {
            logFailure("Adding \(entry.slug) to \(exporter.id)", error)
            return Banner(kind: .error, title: "Couldn't add to \(exporter.displayName)", message: error.localizedDescription)
        }
    }

    /// Removes the theme's colors from the selected terminal, or says how to where that's manual.
    public func remove(_ entry: CatalogEntry) -> Banner {
        let exporter = selected
        let name = library.themeName(for: entry)
        let scheme = exporter.schemeName(forTheme: name)
        guard exporter.canRemove else {
            return Banner(kind: .info, title: "Remove it in \(exporter.displayName)", message: exporter.removeInstructions(scheme: scheme))
        }
        do {
            try exporter.remove(slug: entry.slug, themeName: name)
            revision += 1
            return Banner(kind: .info, title: "Removed from \(exporter.displayName)", message: exporter.removedMessage(scheme: scheme))
        } catch {
            logFailure("Removing \(entry.slug) from \(exporter.id)", error)
            return Banner(kind: .error, title: "Couldn't remove it from \(exporter.displayName)", message: error.localizedDescription)
        }
    }

    /// Themes downloaded before `muted` and `bright_foreground` were read saved a palette without
    /// them, so look the theme up again (cached, usually free) for Omarchy's exact bright black,
    /// bright white and cursor. Offline, use what's saved.
    private func paletteForTerminal(_ entry: CatalogEntry, saved: Palette) async -> Palette {
        let savedWithoutNamedExtras = saved.source == .colorsToml && saved.muted == nil && saved.brightForeground == nil
            && !saved.swatches.contains { $0.name == "Bright black" }
        guard library.installedTheme(entry.slug) != nil, savedWithoutNamedExtras else { return saved }
        return (try? await library.details(for: entry))?.palette ?? saved
    }
}
