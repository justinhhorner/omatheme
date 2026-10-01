import Foundation

/// The app's identity, as both apps send it (Windows: `AppInfo` in Core).
public enum AppInfo {
    static let bundleIdentifier = "com.justinhhorner.OmarchyThemes"

    /// "0.1.0", the app's marketing version (`MARKETING_VERSION` in project.yml). Outside the app
    /// (e.g. under `swift test`, where the main bundle is the test runner) it's "0.0.0".
    public static let version = version(of: .main)

    /// Sent with every request (GitHub rejects API requests without one).
    public static let userAgent = userAgent(version: version)

    static func version(of bundle: Bundle) -> String {
        guard bundle.bundleIdentifier == bundleIdentifier,
              let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        else { return "0.0.0" }
        return version
    }

    static func userAgent(version: String) -> String {
        "OmarchyThemes/\(version) (+https://github.com/basecamp/omarchy)"
    }
}
