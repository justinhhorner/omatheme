import Foundation
import SwiftSoup

/// One theme card from omarchy.org/themes.
public struct CatalogEntry: Hashable, Sendable, Codable, Identifiable {
    /// Stable local id (screenshot file stem, else derived from the repo name).
    public let slug: String
    public let name: String
    public let repoURL: URL
    public let screenshotURL: URL?

    public init(slug: String, name: String, repoURL: URL, screenshotURL: URL?) {
        self.slug = slug
        self.name = name
        self.repoURL = repoURL
        self.screenshotURL = screenshotURL
    }

    public var id: String { slug }

    /// One of the themes that ship with Omarchy (see `DefaultThemes`).
    public var isDefaultTheme: Bool { DefaultThemes.isDefault(slug) }

    /// "owner/repo" for community themes; default themes all live in Omarchy's repo.
    public var repoDisplay: String {
        if isDefaultTheme { return "Included with Omarchy" }
        return RepoRef(parsing: repoURL.absoluteString)?.fullName ?? repoURL.absoluteString
    }
}

/// Parses the omarchy.org/themes gallery. The live page (an Astro build) renders each theme as
/// `<li><a href="https://github.com/…"><img src="/assets/themes/x.webp"><span>Name</span></a></li>`.
/// Rather than depend on classes or exact nesting, it looks for GitHub repo links that have a
/// screenshot, either inside the link or in the same `li`/`figure`. That also covers a
/// `<figure><img><figcaption><a>` layout.
public enum CatalogParser {
    public static let defaultPageURL = URL(string: "https://omarchy.org/themes/")!

    private static let cardTags: Set<String> = ["li", "figure", "article"]

    public static func parse(_ html: String, pageURL: URL = defaultPageURL) -> [CatalogEntry] {
        guard let document = try? SwiftSoup.parse(html, pageURL.absoluteString),
              let anchors = try? document.select("a[href]")
        else { return [] }

        var entries: [CatalogEntry] = []
        var seenRepos: Set<String> = []
        var seenSlugs: Set<String> = []

        for anchor in anchors {
            guard let repo = RepoRef(parsing: try? anchor.attr("href")) else { continue }

            let card = closest(anchor, tags: cardTags)
            // Text links such as "Share your theme" are not theme cards.
            guard let img = (try? anchor.select("img").first()) ?? (try? card?.select("img").first()) else { continue }

            let repoURL = repo.htmlURL
            guard seenRepos.insert(repoURL.absoluteString.lowercased()).inserted else { continue }

            let screenshot = resolve(try? img.attr("src"), against: pageURL)
            let name = firstNonEmpty(
                (try? card?.select("figcaption").first()?.text()) ?? nil,
                try? anchor.text(),
                stripScreenshotSuffix(try? img.attr("alt"))
            ) ?? repo.name

            let slug = unique(slug(screenshot: screenshot, repo: repo), in: &seenSlugs)
            entries.append(CatalogEntry(slug: slug, name: name, repoURL: repoURL, screenshotURL: screenshot))
        }
        return entries
    }

    private static func closest(_ element: Element, tags: Set<String>) -> Element? {
        var node: Element? = element
        while let current = node {
            if tags.contains(current.tagName().lowercased()) { return current }
            node = current.parent()
        }
        return nil
    }

    private static func resolve(_ src: String?, against pageURL: URL) -> URL? {
        guard let src = src?.trimmingCharacters(in: .whitespacesAndNewlines), !src.isEmpty,
              let url = URL(string: src, relativeTo: pageURL)?.absoluteURL.standardized,
              let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http"
        else { return nil }
        return url
    }

    private static func firstNonEmpty(_ candidates: String?...) -> String? {
        candidates.lazy.compactMap { $0.map(collapseWhitespace) }.first { !$0.isEmpty }
    }

    private static func collapseWhitespace(_ s: String) -> String {
        s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    private static func stripScreenshotSuffix(_ alt: String?) -> String? {
        guard let alt else { return nil }
        let suffix = " theme screenshot"
        return alt.lowercased().hasSuffix(suffix) ? String(alt.dropLast(suffix.count)) : alt
    }

    static func slug(screenshot: URL?, repo: RepoRef) -> String {
        if let screenshot {
            let fromScreenshot = sanitize(screenshot.deletingPathExtension().lastPathComponent)
            if !fromScreenshot.isEmpty { return fromScreenshot }
        }

        let name = repo.subPath.map { $0.split(separator: "/").last.map(String.init) ?? $0 } ?? repo.name
        var slug = sanitize(name)
        if slug.hasPrefix("omarchy-"), slug.count > 8 { slug = String(slug.dropFirst(8)) }
        if slug.hasSuffix("-theme"), slug.count > 6 { slug = String(slug.dropLast(6)) }
        return slug.isEmpty ? "theme" : slug
    }

    private static func sanitize(_ s: String) -> String {
        let mapped = String(s.lowercased().map { c in
            c.isASCII && (c.isLetter || c.isNumber || c == "-" || c == "_") ? c : "-"
        })
        return mapped.trimmingCharacters(in: CharacterSet(charactersIn: "-"))
    }

    private static func unique(_ slug: String, in seen: inout Set<String>) -> String {
        var candidate = slug
        var i = 2
        while !seen.insert(candidate.lowercased()).inserted {
            candidate = "\(slug)-\(i)"
            i += 1
        }
        return candidate
    }
}
