import Foundation
import Observation
import OmarchyThemesKit

/// The app's settings (`settings.json`) and the welcome screen.
@MainActor
@Observable
public final class Preferences {
    public private(set) var settings: AppSettings

    /// Shows until the user continues past it once (and again from Settings or the Help menu).
    public var showWelcome: Bool

    private let store: SettingsStore

    public init(store: SettingsStore) {
        let loaded = store.load()
        self.store = store
        settings = loaded
        showWelcome = !loaded.welcomeSeen
    }

    /// Changes the settings and saves them, if anything changed.
    public func update(_ change: (inout AppSettings) -> Void) {
        var updated = settings
        change(&updated)
        guard updated != settings else { return }
        settings = updated
        do {
            try store.save(updated)
        } catch {
            // The change still applies for this session; it just won't be there next launch.
            logFailure("Saving settings", error)
        }
    }

    public func dismissWelcome() {
        showWelcome = false
        update { $0.welcomeSeen = true }
    }
}
