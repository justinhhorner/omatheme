import Foundation

/// Where the snapshot of the user's original desktop is kept between the first Apply and Restore.
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
        try FileManager.default.removeItemIfPresent(at: paths.snapshotFile)
    }
}
