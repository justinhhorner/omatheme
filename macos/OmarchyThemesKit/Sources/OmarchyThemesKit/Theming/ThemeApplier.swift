import Foundation

/// Applies a theme through a `DesktopBackend`. Before the first change it saves the user's
/// current desktop (so "Restore my original desktop" can undo everything), then runs each step
/// independently: one failing step doesn't stop the others, and the result reports exactly what
/// happened per step.
public final class ThemeApplier: Sendable {
    private let backend: any DesktopBackend
    private let snapshots: any SnapshotStore
    private let gate = AsyncGate()

    public init(backend: any DesktopBackend, snapshots: any SnapshotStore) {
        self.backend = backend
        self.snapshots = snapshots
    }

    public var capabilities: DesktopCapabilities { backend.capabilities }

    public var supportedFits: [WallpaperFit] { backend.supportedFits }

    /// True when a snapshot was saved, including one that can't be read: Settings still offers
    /// Restore, which then says why it can't.
    public var hasOriginalSnapshot: Bool {
        do {
            return try snapshots.load() != nil
        } catch {
            return true
        }
    }

    public func apply(_ request: ApplyRequest, progress: (@Sendable (ApplyStep) -> Void)? = nil) async throws -> ApplyResult {
        try Task.checkCancellation()
        return try await gate.withLock {
            var results: [StepResult] = []

            progress?(.saveOriginal)
            let saved: DesktopSnapshot?
            do {
                saved = try snapshots.load()
            } catch {
                // Saving now would replace the user's original with a desktop we may already have themed.
                return Self.nothingChanged(
                    "\(error.localizedDescription) Nothing was changed, so it isn't overwritten. "
                        + "To save your current desktop instead, delete that file.")
            }
            if saved == nil {
                do {
                    try snapshots.save(try await backend.capture())
                    results.append(StepResult(.saveOriginal, .applied))
                } catch {
                    if error is CancellationError { throw error }
                    // Without a snapshot we couldn't undo, so change nothing.
                    return Self.nothingChanged(
                        "Couldn't save your current desktop, so nothing was changed. \(error.localizedDescription)")
                }
            }

            let options = request.options

            results.append(try await runStep(.wallpaper, enabled: options.wallpaper, capability: .wallpaper,
                                             hasData: request.wallpaper != nil, progress: progress) {
                let url = request.wallpaper!
                guard FileManager.default.fileExists(atPath: url.path) else {
                    throw MissingWallpaperError()
                }
                try await self.backend.setWallpaper(url, fit: options.fit, fillColor: request.background)
            })

            results.append(try await runStep(.appearanceMode, enabled: options.appearanceMode, capability: .appearanceMode,
                                             hasData: true, progress: progress) {
                try await self.backend.setAppearanceMode(request.mode)
            })

            results.append(try await runStep(.accentColor, enabled: options.accentColor, capability: .accentColor,
                                             hasData: request.accent != nil, progress: progress) {
                try await self.backend.setAccentColor(request.accent!)
            })

            return ApplyResult(steps: results)
        }
    }

    /// The original desktop couldn't be secured, so no step ran.
    private static func nothingChanged(_ message: String) -> ApplyResult {
        ApplyResult(steps: [StepResult(.saveOriginal, .failed, error: message)]
            + [ApplyStep.wallpaper, .appearanceMode, .accentColor].map { StepResult($0, .notAttempted) })
    }

    private func runStep(
        _ step: ApplyStep, enabled: Bool, capability: DesktopCapabilities, hasData: Bool,
        progress: (@Sendable (ApplyStep) -> Void)?, action: () async throws -> Void
    ) async throws -> StepResult {
        guard enabled else { return StepResult(step, .skippedByUser) }
        guard backend.capabilities.contains(capability) else { return StepResult(step, .notSupported) }
        guard hasData else { return StepResult(step, .noData) }

        try Task.checkCancellation()
        progress?(step)
        do {
            try await action()
            return StepResult(step, .applied)
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            return StepResult(step, .failed, error: error.localizedDescription)
        }
    }

    /// Puts back the desktop saved before the first Apply, then forgets the snapshot. Throws
    /// `SnapshotUnreadableError` (leaving the file alone) if the snapshot can't be read.
    public func restoreOriginal() async throws -> Bool {
        try await gate.withLock {
            guard let snapshot = try snapshots.load() else { return false }
            try await backend.restore(snapshot)
            try snapshots.clear()
            return true
        }
    }
}

public struct MissingWallpaperError: LocalizedError, Sendable {
    public var errorDescription: String? { "The wallpaper file is missing. Try downloading the theme again." }
}
