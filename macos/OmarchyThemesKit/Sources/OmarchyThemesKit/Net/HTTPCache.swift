import CryptoKit
import Foundation

/// Disk-backed HTTP GET cache. Responses are stored with their ETag / Last-Modified and
/// revalidated with conditional requests (a GitHub 304 doesn't count against the rate limit).
/// When the network or server fails and a cached copy exists, the stale copy is returned with
/// the error attached, which keeps the app usable offline.
public final class HTTPCache: Sendable {
    private let transport: any HTTPTransport
    private let directory: URL
    private let now: @Sendable () -> Date

    public init(transport: any HTTPTransport, directory: URL, now: @escaping @Sendable () -> Date = { Date() }) {
        self.transport = transport
        self.directory = directory
        self.now = now
    }

    public func get(_ url: URL, options: CacheOptions = CacheOptions()) async throws -> CachedResponse {
        let cached = cachedResponse(for: url)

        if let cached, !options.forceRevalidate, now().timeIntervalSince(cached.fetchedAt) < options.maxAge {
            return cached
        }

        var request = URLRequest(url: url)
        options.configureRequest?(&request)
        if let etag = cached?.etag {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        } else if let lastModified = cached?.lastModified {
            request.setValue(lastModified, forHTTPHeaderField: "If-Modified-Since")
        }

        let data: Data
        let response: HTTPURLResponse
        do {
            (data, response) = try await transport.send(request)
        } catch let error as URLError where error.code != .cancelled && !Task.isCancelled {
            if let cached { return cached.stale(because: error) }
            throw error
        }

        if response.statusCode == 304, var refreshed = cached {
            refreshed.fetchedAt = now()
            try? writeMeta(for: url, refreshed)
            return refreshed
        }

        guard (200..<300).contains(response.statusCode) else {
            let error = options.mapError?(response) ?? HTTPStatusError(status: response.statusCode, host: url.host() ?? "The server")
            if let cached, options.allowStaleOnError { return cached.stale(because: error) }
            throw error
        }

        let fresh = CachedResponse(
            body: data,
            fetchedAt: now(),
            isStale: false,
            etag: response.value(forHTTPHeaderField: "ETag"),
            lastModified: response.value(forHTTPHeaderField: "Last-Modified"))
        // Caching is best-effort: a full disk shouldn't turn a good response into an error.
        try? store(fresh, for: url)
        return fresh
    }

    /// The cached copy without touching the network, or nil.
    public func cachedResponse(for url: URL) -> CachedResponse? {
        let (bodyURL, metaURL) = paths(for: url)
        guard let meta = JSONFile.read(CacheMeta.self, from: metaURL), meta.url == url.absoluteString,
              let body = try? Data(contentsOf: bodyURL)
        else { return nil }
        return CachedResponse(body: body, fetchedAt: meta.fetchedAt, isStale: false,
                              etag: meta.etag, lastModified: meta.lastModified)
    }

    /// Deletes every cached response. Only the cache's own `<xx>/` folders go, so other caches kept
    /// in the same folder (the app's thumbnails in `images/`) are left to their owners.
    public func clear() throws {
        let fm = FileManager.default
        let items = (try? fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        for item in items where Self.isShardFolder(item.lastPathComponent) {
            try fm.removeItem(at: item)
        }
    }

    private static func isShardFolder(_ name: String) -> Bool {
        name.count == 2 && name.allSatisfy(\.isHexDigit)
    }

    private func store(_ response: CachedResponse, for url: URL) throws {
        try FileManager.default.writeAtomically(response.body, to: paths(for: url).body)
        try writeMeta(for: url, response)
    }

    private func writeMeta(for url: URL, _ response: CachedResponse) throws {
        let meta = CacheMeta(
            url: url.absoluteString, fetchedAt: response.fetchedAt, etag: response.etag, lastModified: response.lastModified)
        try JSONFile.write(meta, to: paths(for: url).meta)
    }

    private func paths(for url: URL) -> (body: URL, meta: URL) {
        let key = SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
        let base = directory.appending(path: String(key.prefix(2)), directoryHint: .isDirectory).appending(path: key)
        return (base.appendingPathExtension("body"), base.appendingPathExtension("json"))
    }

    private struct CacheMeta: Codable {
        var url: String
        var fetchedAt: Date
        var etag: String?
        var lastModified: String?
    }
}

public struct CacheOptions: Sendable {
    /// How long a cached copy is used without any network request. Zero always revalidates.
    public var maxAge: TimeInterval

    /// Revalidate even if the cached copy is younger than `maxAge`.
    public var forceRevalidate: Bool

    /// Return the cached copy (marked stale) when the server responds with an error.
    public var allowStaleOnError: Bool

    public var configureRequest: (@Sendable (inout URLRequest) -> Void)?

    /// Turns a non-success response into a domain error (e.g. GitHub rate limiting).
    public var mapError: (@Sendable (HTTPURLResponse) -> (any Error)?)?

    public init(
        maxAge: TimeInterval = 0,
        forceRevalidate: Bool = false,
        allowStaleOnError: Bool = true,
        configureRequest: (@Sendable (inout URLRequest) -> Void)? = nil,
        mapError: (@Sendable (HTTPURLResponse) -> (any Error)?)? = nil
    ) {
        self.maxAge = maxAge
        self.forceRevalidate = forceRevalidate
        self.allowStaleOnError = allowStaleOnError
        self.configureRequest = configureRequest
        self.mapError = mapError
    }
}

public struct CachedResponse: Sendable {
    public var body: Data
    public var fetchedAt: Date
    public var isStale: Bool
    public var etag: String?
    public var lastModified: String?

    /// Why a stale copy was served instead of a fresh one.
    public var error: (any Error)?

    public var text: String {
        let s = String(decoding: body, as: UTF8.self)
        return s.hasPrefix("\u{FEFF}") ? String(s.dropFirst()) : s
    }

    func stale(because error: any Error) -> CachedResponse {
        var copy = self
        copy.isStale = true
        copy.error = error
        return copy
    }
}
