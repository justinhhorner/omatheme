import Foundation

/// Where the snapshot of the user's original desktop is kept between the first Apply and Restore.
public protocol SnapshotStore: Sendable {
    /// The saved desktop, or nil if none was saved. Throws `SnapshotUnreadableError` if one was
    /// saved but can't be read.
    func load() throws -> DesktopSnapshot?
    func save(_ snapshot: DesktopSnapshot) throws
    func clear() throws
}

/// The saved original desktop exists but can't be read. It's the only way back to the user's own
/// desktop, so it must not be treated as missing (and replaced by the themed desktop).
public struct SnapshotUnreadableError: LocalizedError, Sendable {
    public let fileName: String

    public var errorDescription: String? {
        "The saved copy of your original desktop can't be read (\(fileName) in the data folder)."
    }
}

/// original-desktop.json in the data folder (docs/data-format.md).
public struct FileSnapshotStore: SnapshotStore {
    private let paths: AppPaths

    public init(paths: AppPaths) {
        self.paths = paths
    }

    public func load() throws -> DesktopSnapshot? {
        let url = paths.snapshotFile
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        do {
            return try JSONFile.makeDecoder().decode(DesktopSnapshot.self, from: Data(contentsOf: url))
        } catch {
            throw SnapshotUnreadableError(fileName: url.lastPathComponent)
        }
    }

    public func save(_ snapshot: DesktopSnapshot) throws {
        try JSONFile.write(snapshot, to: paths.snapshotFile)
    }

    public func clear() throws {
        try FileManager.default.removeItemIfPresent(at: paths.snapshotFile)
    }
}
