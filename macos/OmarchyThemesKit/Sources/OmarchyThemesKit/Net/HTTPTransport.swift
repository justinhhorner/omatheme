import Foundation

/// Sends one HTTP request. `URLSessionTransport` in the app; a routing fake in tests, so the
/// cache and GitHub client are tested without the network.
public protocol HTTPTransport: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionTransport: HTTPTransport {
    public let session: URLSession

    public init(session: URLSession) {
        self.session = session
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        return (data, http)
    }
}

public enum HTTPSessions {
    public static let userAgent = "OmarchyThemes/0.1 (+https://github.com/basecamp/omarchy)"

    /// A session with URLSession's own cache turned off: `HTTPCache` does ETag revalidation itself
    /// and needs to see real 304s.
    public static func make(requestTimeout: TimeInterval = 30, resourceTimeout: TimeInterval = 7 * 24 * 3600) -> URLSession {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.timeoutIntervalForRequest = requestTimeout
        configuration.timeoutIntervalForResource = resourceTimeout
        configuration.httpAdditionalHeaders = ["User-Agent": userAgent]
        return URLSession(configuration: configuration)
    }
}

/// A non-success HTTP status that wasn't mapped to a domain error.
public struct HTTPStatusError: LocalizedError, Sendable {
    public let status: Int
    public let host: String

    public init(status: Int, host: String) {
        self.status = status
        self.host = host
    }

    public var errorDescription: String? {
        let reason = HTTPURLResponse.localizedString(forStatusCode: status)
        return "\(host) returned \(status) \(reason.prefix(1).uppercased() + reason.dropFirst())."
    }
}
