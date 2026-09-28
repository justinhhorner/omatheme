import Foundation
import OmarchyThemesKit

/// Where a result banner is shown.
enum BannerContext: Hashable {
    case gallery
    case currentTheme
    case downloaded
    case settings
    case theme(String)
    /// Results of sending a theme to a terminal, shown in that section of its page.
    case terminal(String)
}

struct Banner: Equatable, Identifiable {
    let id = UUID()
    var kind: SummaryKind
    var title: String
    var message: String

    init(kind: SummaryKind, title: String, message: String) {
        self.kind = kind
        self.title = title
        self.message = message
    }

    init(_ summary: ApplySummary) {
        self.init(kind: summary.kind, title: summary.title, message: summary.message)
    }
}
