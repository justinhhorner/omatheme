import Foundation
import Testing
@testable import OmarchyThemesKit

struct ThemeApplierTests {
    let dir = TempDir()
    let backend = FakeDesktopBackend()
    let snapshots = InMemorySnapshotStore()
    let applier: ThemeApplier
    let wallpaper: URL

    init() throws {
        applier = ThemeApplier(backend: backend, snapshots: snapshots)
        wallpaper = dir.url.appending(path: "wall.png")
        try Data([1, 2, 3]).write(to: wallpaper)
    }

    func request(_ options: ApplyOptions = ApplyOptions(), wallpaper: URL?? = .none, accent: RgbColor? = RgbColor("#7aa2f7")) -> ApplyRequest {
        ApplyRequest(themeName: "Tokyo", wallpaper: wallpaper ?? self.wallpaper, mode: .light, accent: accent,
                     background: RgbColor("#1a1b26"), options: options)
    }

    @Test func savesOriginalDesktopThenAppliesEveryAspectInOrder() async throws {
        let result = try await applier.apply(request())

        #expect(backend.calls == ["capture", "wallpaper:wall.png:fill:#1a1b26", "mode:light", "accent:#7aa2f7"])
        #expect(result.succeeded)
        #expect(result.steps.allSatisfy { $0.outcome == .applied })
        #expect(snapshots.snapshot == backend.snapshotToReturn)
    }

    @Test func onlyTheFirstApplyTakesASnapshot() async throws {
        _ = try await applier.apply(request())
        backend.clearCalls()

        let second = try await applier.apply(request())

        #expect(!backend.calls.contains("capture"))
        #expect(second.result(for: .saveOriginal) == nil)
    }

    @Test func changesNothingIfTheOriginalDesktopCannotBeSaved() async throws {
        backend.failOn("capture")

        let result = try await applier.apply(request())

        #expect(backend.calls == ["capture"])
        #expect(result.result(for: .saveOriginal)?.outcome == .failed)
        #expect(result.result(for: .wallpaper)?.outcome == .notAttempted)
        #expect(!result.anyApplied)
        #expect(snapshots.snapshot == nil)
    }

    @Test func respectsUserChoices() async throws {
        let result = try await applier.apply(request(ApplyOptions(appearanceMode: false, accentColor: false, fit: .center)))

        #expect(backend.calls == ["capture", "wallpaper:wall.png:center:#1a1b26"])
        #expect(result.result(for: .appearanceMode)?.outcome == .skippedByUser)
        #expect(result.result(for: .accentColor)?.outcome == .skippedByUser)
        #expect(result.succeeded)
    }

    @Test func skipsWhatThePlatformCannotDo() async throws {
        backend.capabilities = .wallpaper

        let result = try await applier.apply(request())

        #expect(backend.calls == ["capture", "wallpaper:wall.png:fill:#1a1b26"])
        #expect(result.result(for: .appearanceMode)?.outcome == .notSupported)
        #expect(result.result(for: .accentColor)?.outcome == .notSupported)
        #expect(result.succeeded)
    }

    @Test func skipsAspectsTheThemeHasNoDataFor() async throws {
        let result = try await applier.apply(request(wallpaper: .some(nil), accent: nil))

        #expect(backend.calls == ["capture", "mode:light"])
        #expect(result.result(for: .wallpaper)?.outcome == .noData)
        #expect(result.result(for: .accentColor)?.outcome == .noData)
    }

    @Test func oneFailingStepDoesNotStopTheOthers() async throws {
        backend.failOn("mode")

        let result = try await applier.apply(request())

        #expect(backend.calls.contains("accent:#7aa2f7"))
        let mode = try #require(result.result(for: .appearanceMode))
        #expect(mode.outcome == .failed)
        #expect(mode.error == "mode exploded")
        #expect(result.anyApplied)
        #expect(result.anyFailed)
        #expect(!result.succeeded)
    }

    @Test func missingWallpaperFileFailsThatStepWithoutCallingTheOS() async throws {
        let result = try await applier.apply(request(wallpaper: dir.url.appending(path: "gone.png")))

        #expect(!backend.calls.contains { $0.hasPrefix("wallpaper") })
        #expect(result.result(for: .wallpaper)?.outcome == .failed)
        #expect(result.result(for: .wallpaper)?.error == MissingWallpaperError().errorDescription)
        #expect(result.result(for: .accentColor)?.outcome == .applied)
    }

    @Test func cancellationPropagates() async throws {
        let applier = self.applier
        let request = self.request()
        let task = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            return try await applier.apply(request)
        }

        await #expect(throws: CancellationError.self) { try await task.value }
        #expect(backend.calls.isEmpty)
    }

    @Test func reportsProgressForEachStepItRuns() async throws {
        let steps = Recorder<ApplyStep>()

        _ = try await applier.apply(request(ApplyOptions(accentColor: false)), progress: steps.append)

        #expect(steps.values == [.saveOriginal, .wallpaper, .appearanceMode])
    }

    @Test func concurrentAppliesDoNotInterleave() async throws {
        let applier = self.applier
        let request = self.request(ApplyOptions(appearanceMode: false, accentColor: false))

        try await withThrowingTaskGroup(of: ApplyResult.self) { group in
            for _ in 0..<5 {
                group.addTask { try await applier.apply(request) }
            }
            for try await _ in group {}
        }

        // Exactly one snapshot, however the applies were scheduled.
        #expect(backend.calls.filter { $0 == "capture" }.count == 1)
        #expect(backend.calls.count == 6)
    }

    @Test func restorePutsBackTheSnapshotOnce() async throws {
        #expect(try await applier.restoreOriginal() == false)

        _ = try await applier.apply(request())
        #expect(applier.hasOriginalSnapshot)

        #expect(try await applier.restoreOriginal())
        #expect(backend.restored == backend.snapshotToReturn)
        #expect(!applier.hasOriginalSnapshot)
        #expect(try await applier.restoreOriginal() == false)
    }

    @Test func failedRestoreKeepsTheSnapshot() async throws {
        _ = try await applier.apply(request())
        backend.failOn("restore")

        await #expect(throws: FakeFailure.self) { try await applier.restoreOriginal() }
        #expect(applier.hasOriginalSnapshot)
    }

    @Test func requestFromInstalledThemeUsesChosenOrFirstWallpaper() {
        let theme = InstalledTheme(slug: "t", name: "T", repoURL: URL(string: "https://github.com/o/t")!, mode: .dark,
                                   wallpapers: ["1.png", "2.png"], directory: dir.url)
        let options = ApplyOptions()

        #expect(ApplyRequest.from(theme, wallpaperFile: "2.png", options: options).wallpaper?.lastPathComponent == "2.png")
        #expect(ApplyRequest.from(theme, wallpaperFile: "missing.png", options: options).wallpaper?.lastPathComponent == "1.png")
        #expect(ApplyRequest.from(theme, wallpaperFile: nil, options: options).wallpaper?.lastPathComponent == "1.png")
        #expect(ApplyRequest.from(theme, wallpaperFile: nil, options: options).accent == nil)

        var empty = theme
        empty.wallpapers = []
        #expect(ApplyRequest.from(empty, wallpaperFile: nil, options: options).wallpaper == nil)
    }

    @Test func fileSnapshotStoreRoundTrips() throws {
        let store = FileSnapshotStore(paths: dir.paths)
        let snapshot = DesktopSnapshot(takenAt: Date(timeIntervalSince1970: 1_790_380_800.25), values: ["k": "v"])

        try store.save(snapshot)
        #expect(store.load() == snapshot)

        try store.clear()
        #expect(store.load() == nil)
        try store.clear()
    }
}

struct ApplySummaryTests {
    static func result(_ steps: (ApplyStep, StepOutcome, String?)...) -> ApplyResult {
        ApplyResult(steps: steps.map { StepResult($0.0, $0.1, error: $0.2) })
    }

    @Test func successListsWhatChanged() {
        let summary = ApplySummary.describe(Self.result(
            (.saveOriginal, .applied, nil),
            (.wallpaper, .applied, nil),
            (.appearanceMode, .applied, nil),
            (.accentColor, .applied, nil)), themeName: "Tokyo Night", mode: .dark)

        #expect(summary.kind == .success)
        #expect(summary.title == "Tokyo Night applied")
        #expect(summary.message == "Updated the wallpaper, dark mode, and the accent color.")
    }

    @Test func wallpaperOnlySuccess() {
        let summary = ApplySummary.describe(Self.result(
            (.wallpaper, .applied, nil),
            (.appearanceMode, .notSupported, nil),
            (.accentColor, .notSupported, nil)), themeName: "Snow", mode: .light)

        #expect(summary.kind == .success)
        #expect(summary.message == "Updated the wallpaper.")
    }

    @Test func partialFailureIsAWarningWithTheReason() {
        let summary = ApplySummary.describe(Self.result(
            (.wallpaper, .applied, nil),
            (.appearanceMode, .applied, nil),
            (.accentColor, .failed, "Access denied.")), themeName: "Snow", mode: .light)

        #expect(summary.kind == .warning)
        #expect(summary.message == "Updated the wallpaper and light mode. The accent color: Access denied.")
    }

    @Test func totalFailureIsAnError() {
        let summary = ApplySummary.describe(Self.result(
            (.wallpaper, .failed, "File missing."),
            (.appearanceMode, .skippedByUser, nil)), themeName: "Snow", mode: .light)

        #expect(summary.kind == .error)
        #expect(summary.title == "Couldn't apply Snow")
    }

    @Test func snapshotFailureExplainsNothingChanged() {
        let summary = ApplySummary.describe(Self.result(
            (.saveOriginal, .failed, "Couldn't save your current desktop, so nothing was changed."),
            (.wallpaper, .notAttempted, nil)), themeName: "Snow", mode: .light)

        #expect(summary.kind == .error)
        #expect(summary.message.contains("nothing was changed"))
    }

    @Test func everythingSkippedIsInformational() {
        let summary = ApplySummary.describe(Self.result(
            (.wallpaper, .skippedByUser, nil),
            (.appearanceMode, .skippedByUser, nil)), themeName: "Snow", mode: .light)

        #expect(summary.kind == .info)
    }
}
