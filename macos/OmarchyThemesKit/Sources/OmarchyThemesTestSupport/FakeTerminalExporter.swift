import Foundation
import OmarchyThemesKit
import OmarchyThemesMac

/// A terminal that keeps "added" schemes in memory and records what it was given.
public final class FakeTerminalExporter: TerminalExporter, @unchecked Sendable {
    private let lock = NSLock()
    private var added: [String: TerminalColors] = [:]
    private var failing = false

    public let id: String
    public let displayName: String
    public let isInstalled: Bool
    public let canRemove: Bool

    public init(id: String, displayName: String? = nil, isInstalled: Bool = true, canRemove: Bool = true) {
        self.id = id
        self.displayName = displayName ?? id
        self.isInstalled = isInstalled
        self.canRemove = canRemove
    }

    /// Colors added per slug.
    public var colors: [String: TerminalColors] { lock.withLock { added } }

    public func failNextCalls() {
        lock.withLock { failing = true }
    }

    public var addHint: String { "Adds to \(displayName)." }

    public func isAdded(slug: String, themeName: String) -> Bool {
        lock.withLock { added[slug] != nil }
    }

    public func add(slug: String, themeName: String, colors: TerminalColors) throws {
        try lock.withLock {
            if failing { throw FakeFailure(message: "\(displayName) is read-only") }
            added[slug] = colors
        }
    }

    public func remove(slug: String, themeName: String) throws {
        try lock.withLock {
            if failing { throw FakeFailure(message: "\(displayName) is read-only") }
            added[slug] = nil
        }
    }

    public func addedMessage(scheme: String) -> String { "Added \(scheme)." }

    public func removedMessage(scheme: String) -> String { "Removed \(scheme)." }

    public func removeInstructions(scheme: String) -> String { "Remove \(scheme) by hand." }
}
