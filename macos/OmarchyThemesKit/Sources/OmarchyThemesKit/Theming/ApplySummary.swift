import Foundation

public enum SummaryKind: Sendable {
    case success
    case info
    case warning
    case error
}

/// Human-readable outcome of an Apply, for the UI's result banner.
public struct ApplySummary: Sendable, Equatable {
    public var kind: SummaryKind
    public var title: String
    public var message: String

    public init(kind: SummaryKind, title: String, message: String) {
        self.kind = kind
        self.title = title
        self.message = message
    }

    public static func describe(_ result: ApplyResult, themeName: String, mode: AppearanceMode) -> ApplySummary {
        if let save = result.result(for: .saveOriginal), save.outcome == .failed {
            return ApplySummary(kind: .error, title: "Theme not applied", message: save.error ?? "Couldn't save your current desktop.")
        }

        let applied = result.steps
            .filter { $0.outcome == .applied && $0.step != .saveOriginal }
            .map { label($0.step, mode) }
        let failed = result.steps.filter { $0.outcome == .failed }

        if failed.isEmpty {
            if applied.isEmpty {
                return ApplySummary(
                    kind: .info, title: "Nothing changed", message: "All options were turned off, so \(themeName) wasn't applied.")
            }
            return ApplySummary(kind: .success, title: "\(themeName) applied", message: "Updated \(joinList(applied)).")
        }

        let problems = failed
            .map { "\(label($0.step, mode).uppercasingFirst): \($0.error ?? "Unknown error.")" }
            .joined(separator: " ")
        return applied.isEmpty
            ? ApplySummary(kind: .error, title: "Couldn't apply \(themeName)", message: problems)
            : ApplySummary(kind: .warning, title: "\(themeName) partly applied", message: "Updated \(joinList(applied)). \(problems)")
    }

    private static func label(_ step: ApplyStep, _ mode: AppearanceMode) -> String {
        switch step {
        case .wallpaper: "the wallpaper"
        case .appearanceMode: mode == .light ? "light mode" : "dark mode"
        case .accentColor: "the accent color"
        case .saveOriginal: "your saved desktop"
        }
    }

    static func joinList(_ items: [String]) -> String {
        switch items.count {
        case 0: ""
        case 1: items[0]
        case 2: "\(items[0]) and \(items[1])"
        default: items.dropLast().joined(separator: ", ") + ", and " + items[items.count - 1]
        }
    }
}
