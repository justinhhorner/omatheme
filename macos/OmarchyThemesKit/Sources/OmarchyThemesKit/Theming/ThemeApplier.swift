import Foundation

/// The user's per-aspect choices in the Apply sheet.
public struct ApplyOptions: Codable, Sendable, Equatable {
    public var wallpaper = true
    public var appearanceMode = true
    public var accentColor = true
    public var fit = WallpaperFit.fill

    public init(wallpaper: Bool = true, appearanceMode: Bool = true, accentColor: Bool = true, fit: WallpaperFit = .fill) {
        self.wallpaper = wallpaper
        self.appearanceMode = appearanceMode
        self.accentColor = accentColor
        self.fit = fit
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        wallpaper = try c.decodeIfPresent(Bool.self, forKey: .wallpaper) ?? true
        appearanceMode = try c.decodeIfPresent(Bool.self, forKey: .appearanceMode) ?? true
        accentColor = try c.decodeIfPresent(Bool.self, forKey: .accentColor) ?? true
        fit = (try? c.decodeIfPresent(WallpaperFit.self, forKey: .fit)) ?? .fill
    }
}

/// What to apply. Built from a downloaded theme, so no network is involved.
public struct ApplyRequest: Sendable {
    public var themeName: String
    public var wallpaper: URL?
    public var mode: AppearanceMode
    public var accent: RgbColor?
    /// The theme's background, shown around wallpapers that don't cover the screen.
    public var background: RgbColor?
    public var options: ApplyOptions

    public init(themeName: String, wallpaper: URL?, mode: AppearanceMode, accent: RgbColor?, background: RgbColor? = nil, options: ApplyOptions) {
        self.themeName = themeName
        self.wallpaper = wallpaper
        self.mode = mode
        self.accent = accent
        self.background = background
        self.options = options
    }

    /// - Parameter wallpaperFile: One of `theme.wallpapers`; defaults to the first.
    public static func from(_ theme: InstalledTheme, wallpaperFile: String?, options: ApplyOptions) -> ApplyRequest {
        let file = wallpaperFile.flatMap { theme.wallpapers.contains($0) ? $0 : nil } ?? theme.wallpapers.first
        return ApplyRequest(
            themeName: theme.name,
            wallpaper: file.map(theme.wallpaperURL),
            mode: theme.mode,
            accent: theme.palette?.accent,
            background: theme.palette?.background,
            options: options)
    }
}

public enum ApplyStep: String, Sendable, CaseIterable {
    case saveOriginal
    case wallpaper
    case appearanceMode
    case accentColor
}

public enum StepOutcome: Sendable, Equatable {
    case applied
    /// The user unchecked it.
    case skippedByUser
    /// This OS/backend can't change it.
    case notSupported
    /// The theme has nothing for it (e.g. no wallpaper or palette).
    case noData
    /// An earlier step failed in a way that made continuing unsafe.
    case notAttempted
    case failed
}

public struct StepResult: Sendable, Equatable {
    public var step: ApplyStep
    public var outcome: StepOutcome
    public var error: String?

    public init(_ step: ApplyStep, _ outcome: StepOutcome, error: String? = nil) {
        self.step = step
        self.outcome = outcome
        self.error = error
    }
}

public struct ApplyResult: Sendable, Equatable {
    public var steps: [StepResult]

    public init(steps: [StepResult]) {
        self.steps = steps
    }

    public var anyApplied: Bool { steps.contains { $0.outcome == .applied } }
    public var anyFailed: Bool { steps.contains { $0.outcome == .failed } }
    public var succeeded: Bool { anyApplied && !anyFailed }

    public func result(for step: ApplyStep) -> StepResult? {
        steps.first { $0.step == step }
    }
}

public protocol SnapshotStore: Sendable {
    func load() -> DesktopSnapshot?
    func save(_ snapshot: DesktopSnapshot) throws
    func clear() throws
}

public struct FileSnapshotStore: SnapshotStore {
    private let paths: AppPaths

    public init(paths: AppPaths) {
        self.paths = paths
    }

    public func load() -> DesktopSnapshot? {
        JSONFile.read(DesktopSnapshot.self, from: paths.snapshotFile)
    }

    public func save(_ snapshot: DesktopSnapshot) throws {
        try JSONFile.write(snapshot, to: paths.snapshotFile)
    }

    public func clear() throws {
        if FileManager.default.fileExists(atPath: paths.snapshotFile.path) {
            try FileManager.default.removeItem(at: paths.snapshotFile)
        }
    }
}

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

    public var hasOriginalSnapshot: Bool { snapshots.load() != nil }

    public func apply(_ request: ApplyRequest, progress: (@Sendable (ApplyStep) -> Void)? = nil) async throws -> ApplyResult {
        try Task.checkCancellation()
        return try await gate.withLock {
            var results: [StepResult] = []

            progress?(.saveOriginal)
            if snapshots.load() == nil {
                do {
                    try snapshots.save(try await backend.capture())
                    results.append(StepResult(.saveOriginal, .applied))
                } catch {
                    if error is CancellationError { throw error }
                    // Without a snapshot we couldn't undo, so change nothing.
                    results.append(StepResult(.saveOriginal, .failed,
                                              error: "Couldn't save your current desktop, so nothing was changed. \(error.localizedDescription)"))
                    results += [ApplyStep.wallpaper, .appearanceMode, .accentColor].map { StepResult($0, .notAttempted) }
                    return ApplyResult(steps: results)
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

    /// Puts back the desktop saved before the first Apply, then forgets the snapshot.
    public func restoreOriginal() async throws -> Bool {
        try await gate.withLock {
            guard let snapshot = snapshots.load() else { return false }
            try await backend.restore(snapshot)
            try snapshots.clear()
            return true
        }
    }
}

public struct MissingWallpaperError: LocalizedError, Sendable {
    public var errorDescription: String? { "The wallpaper file is missing. Try downloading the theme again." }
}

/// Serializes async operations (like a semaphore of one). An actor alone wouldn't: it's
/// re-entrant across the backend calls an Apply awaits.
final class AsyncGate: Sendable {
    private let state = State()

    func withLock<T: Sendable>(_ body: () async throws -> T) async throws -> T {
        await state.acquire()
        do {
            let value = try await body()
            await state.release()
            return value
        } catch {
            await state.release()
            throw error
        }
    }

    private actor State {
        private var busy = false
        private var waiters: [CheckedContinuation<Void, Never>] = []

        func acquire() async {
            if busy {
                await withCheckedContinuation { waiters.append($0) }
            } else {
                busy = true
            }
        }

        func release() {
            if waiters.isEmpty {
                busy = false
            } else {
                waiters.removeFirst().resume()
            }
        }
    }
}
