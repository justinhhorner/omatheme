import Foundation

public protocol Downloader: Sendable {
    /// Downloads `url` to `destination`, reporting the bytes received so far.
    func download(from url: URL, to destination: URL, progress: (@Sendable (Int64) -> Void)?) async throws
}

/// Downloads to a temporary file first, so the destination is either complete or absent.
public struct URLSessionDownloader: Downloader {
    private let session: URLSession

    public init(session: URLSession) {
        self.session = session
    }

    public func download(from url: URL, to destination: URL, progress: (@Sendable (Int64) -> Void)?) async throws {
        let observer = ProgressObserver(report: progress)
        let (temporary, response) = try await session.download(for: URLRequest(url: url), delegate: observer)
        defer { try? FileManager.default.removeItem(at: temporary) }

        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw DownloadError(fileName: url.lastPathComponent, status: http.statusCode)
        }
        try Task.checkCancellation()

        try FileManager.default.removeItemIfPresent(at: destination)
        try FileManager.default.moveItem(at: temporary, to: destination)
    }

    /// Reports bytes received by observing the task's `countOfBytesReceived`. (Its `progress`
    /// counts in its own units, not bytes.)
    private final class ProgressObserver: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
        private let report: (@Sendable (Int64) -> Void)?
        private let lock = NSLock()
        private var observation: NSKeyValueObservation?

        init(report: (@Sendable (Int64) -> Void)?) {
            self.report = report
        }

        func urlSession(_ session: URLSession, didCreateTask task: URLSessionTask) {
            guard let report else { return }
            let observation = task.observe(\.countOfBytesReceived, options: [.new]) { task, _ in
                report(task.countOfBytesReceived)
            }
            lock.withLock { self.observation = observation }
        }

        deinit {
            observation?.invalidate()
        }
    }
}

public struct DownloadError: LocalizedError, Sendable {
    public let fileName: String
    public let status: Int

    public var errorDescription: String? {
        "Downloading \(fileName) failed: \(status) \(HTTPURLResponse.localizedString(forStatusCode: status))."
    }
}
