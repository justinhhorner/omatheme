import Foundation

extension FileManager {
    /// Deletes `url` if it exists. A missing file isn't an error.
    public func removeItemIfPresent(at url: URL) throws {
        if fileExists(atPath: url.path) {
            try removeItem(at: url)
        }
    }

    /// Writes via a temp file + rename, creating the folder if needed, so a crash never leaves a
    /// half-written file.
    public func writeAtomically(_ data: Data, to url: URL) throws {
        try createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url, options: .atomic)
    }
}
