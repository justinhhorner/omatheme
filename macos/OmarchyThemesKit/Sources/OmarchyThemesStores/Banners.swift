import Foundation
import Observation
import OmarchyThemesKit
import os

/// Where a result banner is shown.
public enum BannerContext: Hashable, Sendable {
    case gallery
    case currentTheme
    case downloaded
    case settings
    case theme(String)
    /// Results of sending a theme to a terminal, shown in that section of its page.
    case terminal(String)
}

/// A dismissable result or notice.
public struct Banner: Equatable, Identifiable, Sendable {
    public let id = UUID()
    public var kind: SummaryKind
    public var title: String
    public var message: String

    public init(kind: SummaryKind, title: String, message: String) {
        self.kind = kind
        self.title = title
        self.message = message
    }

    public init(_ summary: ApplySummary) {
        self.init(kind: summary.kind, title: summary.title, message: summary.message)
    }
}

/// The banner currently shown in each context. Stores post results here; views show and dismiss them.
@MainActor
@Observable
public final class Banners {
    private var items: [BannerContext: Banner] = [:]

    public init() {}

    public subscript(context: BannerContext) -> Banner? {
        get { items[context] }
        set { items[context] = newValue }
    }
}

extension Date {
    /// "5 minutes ago", "yesterday".
    public var relativeDescription: String {
        formatted(.relative(presentation: .named))
    }
}

let log = Logger(subsystem: "com.justinhhorner.OmarchyThemes", category: "app")

/// Logs "<action> failed: <error>".
func logFailure(_ action: String, _ error: any Error) {
    log.error("\(action, privacy: .public) failed: \(String(describing: error), privacy: .public)")
}
