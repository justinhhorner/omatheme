import Foundation
@testable import OmarchyThemesKit

/// Routes requests by exact URL (or a fallback) and records what was sent.
final class FakeTransport: HTTPTransport, @unchecked Sendable {
    typealias Handler = @Sendable (URLRequest) throws -> (Data, HTTPURLResponse)

    private let lock = NSLock()
    private var routes: [String: Handler] = [:]
    private var recorded: [URLRequest] = []

    var requests: [URLRequest] { lock.withLock { recorded } }

    @discardableResult
    func on(_ url: String, _ handler: @escaping Handler) -> Self {
        lock.withLock { routes[url] = handler }
        return self
    }

    @discardableResult
    func on(_ url: String, body: String, etag: String? = nil) -> Self {
        on(url) { request in Self.text(body, for: request, etag: etag) }
    }

    func count(for url: String) -> Int {
        requests.filter { $0.url?.absoluteString == url }.count
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let handler = lock.withLock {
            recorded.append(request)
            return routes[request.url!.absoluteString]
        }
        guard let handler else { return Self.status(404, for: request) }
        return try handler(request)
    }

    static func text(_ body: String, for request: URLRequest, etag: String? = nil, status: Int = 200) -> (Data, HTTPURLResponse) {
        var headers = ["Content-Type": "text/plain; charset=utf-8"]
        if let etag { headers["ETag"] = etag }
        return (Data(body.utf8), HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!)
    }

    static func status(_ status: Int, for request: URLRequest, headers: [String: String] = [:]) -> (Data, HTTPURLResponse) {
        (Data(), HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!)
    }

    static func offline() throws -> (Data, HTTPURLResponse) {
        throw URLError(.notConnectedToInternet)
    }
}

final class ManualClock: @unchecked Sendable {
    private let lock = NSLock()
    private var current: Date

    init(_ start: Date) {
        current = start
    }

    var now: Date { lock.withLock { current } }

    func advance(by interval: TimeInterval) {
        lock.withLock { current += interval }
    }

    var function: @Sendable () -> Date { { self.now } }
}

/// A unique folder under the temp directory, deleted when the test's reference goes away.
final class TempDir {
    let url = FileManager.default.temporaryDirectory
        .appending(path: "omatheme-tests", directoryHint: .isDirectory)
        .appending(path: UUID().uuidString, directoryHint: .isDirectory)

    init() {
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }

    var paths: AppPaths { AppPaths(root: url) }
}

/// Writes the URL into the file instead of downloading; can be told to fail for a URL.
final class FakeDownloader: Downloader, @unchecked Sendable {
    private let lock = NSLock()
    private var failing: Set<String> = []
    private var done: [URL] = []

    var downloaded: [URL] { lock.withLock { done } }

    func fail(_ url: URL) {
        lock.withLock { _ = failing.insert(url.absoluteString) }
    }

    func download(from url: URL, to destination: URL, progress: (@Sendable (Int64) -> Void)?) async throws {
        try Task.checkCancellation()
        if lock.withLock({ failing.contains(url.absoluteString) }) {
            throw URLError(.cannotConnectToHost)
        }
        let bytes = Data(url.absoluteString.utf8)
        try bytes.write(to: destination)
        progress?(Int64(bytes.count))
        lock.withLock { done.append(url) }
    }
}

/// Records every OS call instead of touching the real desktop.
final class FakeDesktopBackend: DesktopBackend, @unchecked Sendable {
    private let lock = NSLock()
    private var _capabilities: DesktopCapabilities = .all
    private var _calls: [String] = []
    private var _failOn: Set<String> = []
    private var _restored: DesktopSnapshot?

    let snapshotToReturn = DesktopSnapshot(takenAt: Date(timeIntervalSince1970: 0), values: ["wallpaper": "/Users/me/original.jpg"])

    var capabilities: DesktopCapabilities {
        get { lock.withLock { _capabilities } }
        set { lock.withLock { _capabilities = newValue } }
    }

    let supportedFits = WallpaperFit.allCases

    var calls: [String] { lock.withLock { _calls } }
    var restored: DesktopSnapshot? { lock.withLock { _restored } }

    func failOn(_ name: String) {
        lock.withLock { _ = _failOn.insert(name) }
    }

    func clearCalls() {
        lock.withLock { _calls.removeAll() }
    }

    private func record(_ call: String) throws {
        let name = String(call.split(separator: ":")[0])
        let fail = lock.withLock {
            _calls.append(call)
            return _failOn.contains(name)
        }
        if fail { throw FakeFailure(message: "\(name) exploded") }
    }

    func capture() async throws -> DesktopSnapshot {
        try record("capture")
        return snapshotToReturn
    }

    func restore(_ snapshot: DesktopSnapshot) async throws {
        lock.withLock { _restored = snapshot }
        try record("restore")
    }

    func setWallpaper(_ image: URL, fit: WallpaperFit, fillColor: RgbColor?) async throws {
        try record("wallpaper:\(image.lastPathComponent):\(fit.rawValue):\(fillColor?.hex ?? "none")")
    }

    func setAppearanceMode(_ mode: AppearanceMode) async throws {
        try record("mode:\(mode.rawValue)")
    }

    func setAccentColor(_ accent: RgbColor) async throws {
        try record("accent:\(accent.hex)")
    }
}

struct FakeFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

final class InMemorySnapshotStore: SnapshotStore, @unchecked Sendable {
    private let lock = NSLock()
    private var stored: DesktopSnapshot?

    var snapshot: DesktopSnapshot? { lock.withLock { stored } }

    func load() -> DesktopSnapshot? { snapshot }
    func save(_ snapshot: DesktopSnapshot) throws { lock.withLock { stored = snapshot } }
    func clear() throws { lock.withLock { stored = nil } }
}

/// Thread-safe collector for progress callbacks.
final class Recorder<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var items: [T] = []

    var values: [T] { lock.withLock { items } }

    func append(_ item: T) {
        lock.withLock { items.append(item) }
    }
}

enum Fixture {
    /// The repo's shared `fixtures/` folder (also used by the Windows tests).
    static let directory = URL(filePath: #filePath) // …/macos/OmarchyThemesKit/Tests/OmarchyThemesKitTests/Support/Fakes.swift
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent() // repo root
        .appending(path: "fixtures", directoryHint: .isDirectory)

    static func read(_ name: String) -> String {
        try! String(contentsOf: directory.appending(path: name), encoding: .utf8)
    }
}

extension RgbColor {
    /// Test shorthand; crashes on invalid input.
    init(_ hex: String) {
        self = RgbColor(hex: hex)!
    }
}
