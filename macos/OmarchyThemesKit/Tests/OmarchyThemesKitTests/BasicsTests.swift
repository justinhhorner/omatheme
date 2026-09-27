import Foundation
import Testing
@testable import OmarchyThemesKit

struct AppPathsTests {
    let paths = AppPaths(root: FileManager.default.temporaryDirectory.appending(path: "omatheme-tests"))

    @Test(arguments: ["tokyo-night", "omarchy_arc.blueberry"])
    func themeDirAcceptsSimpleSlugs(slug: String) throws {
        #expect(try paths.themeDir(slug) == paths.themesDir.appending(path: slug, directoryHint: .isDirectory))
    }

    @Test(arguments: ["", "..", "../evil", "a/b", "a\\b", "C:", "~"])
    func themeDirRejectsPathEscapes(slug: String) {
        #expect(throws: InvalidSlugError.self) { try paths.themeDir(slug) }
    }

    @Test func layoutMatchesWindows() {
        #expect(paths.cacheDir.lastPathComponent == "cache")
        #expect(paths.themesDir.lastPathComponent == "themes")
        #expect(paths.settingsFile.lastPathComponent == "settings.json")
        #expect(paths.snapshotFile.lastPathComponent == "original-desktop.json")
        #expect(AppPaths.default().root.path.hasSuffix("Library/Application Support/OmarchyThemes"))
    }
}

struct ColorTests {
    @Test(arguments: [
        ("#7aa2f7", 0x7a, 0xa2, 0xf7),
        ("7AA2F7", 0x7a, 0xa2, 0xf7),
        ("0x7aa2f7", 0x7a, 0xa2, 0xf7),
        ("#fff", 0xff, 0xff, 0xff),
        ("#7aa2f7cc", 0x7a, 0xa2, 0xf7),
        ("  #000000 ", 0, 0, 0),
    ] as [(String, UInt8, UInt8, UInt8)])
    func parsesHexForms(text: String, r: UInt8, g: UInt8, b: UInt8) {
        #expect(RgbColor(hex: text) == RgbColor(r: r, g: g, b: b))
    }

    @Test(arguments: ["", "CellForeground", "#12345", "#gggggg", "+fffff", "#-12345"])
    func rejectsNonColors(text: String) {
        #expect(RgbColor(hex: text) == nil)
    }

    @Test func serializesAsHexString() throws {
        let json = String(decoding: try JSONFile.makeEncoder().encode(["c": RgbColor("#7AA2F7")]), as: UTF8.self)
        #expect(json.contains("\"#7aa2f7\""))
        #expect(try JSONFile.makeDecoder().decode(RgbColor.self, from: Data("\"#7aa2f7\"".utf8)) == RgbColor("#7aa2f7"))
        #expect(throws: DecodingError.self) { try JSONFile.makeDecoder().decode(RgbColor.self, from: Data("\"nope\"".utf8)) }
    }

    @Test func lightAndDarkBackgroundsAreClassified() {
        #expect(!RgbColor("#1a1b26").isLight)
        #expect(RgbColor("#fdf6e3").isLight)
        #expect(RgbColor("#e1e2e7").isLight)
    }
}

struct RepoRefTests {
    @Test(arguments: [
        ("https://github.com/JJDizz1L/aetheria", "JJDizz1L", "aetheria", nil, nil),
        ("https://github.com/owner/repo.git", "owner", "repo", nil, nil),
        ("https://www.github.com/owner/repo/", "owner", "repo", nil, nil),
        ("http://github.com/owner/repo", "owner", "repo", nil, nil),
        ("  https://github.com/owner/repo  ", "owner", "repo", nil, nil),
        ("https://github.com/owner/repo/tree/dev", "owner", "repo", "dev", nil),
        ("https://github.com/owner/mono/tree/main/themes/foo", "owner", "mono", "main", "themes/foo"),
    ] as [(String, String, String, String?, String?)])
    func parsesRepoLinks(url: String, owner: String, name: String, ref: String?, subPath: String?) {
        #expect(RepoRef(parsing: url) == RepoRef(owner: owner, name: name, ref: ref, subPath: subPath))
    }

    @Test(arguments: [
        nil, "", "not a url", "https://gitlab.com/owner/repo", "https://github.com/owner",
        "https://github.com/omacom/omarchy-site/compare", "https://github.com/owner/repo/issues/1",
        "https://github.com/owner/repo/tree", "https://github.com/orgs/basecamp",
        "https://github.com/owner/repo/tree/main/../../etc", "ftp://github.com/owner/repo",
    ] as [String?])
    func rejectsNonRepoLinks(url: String?) {
        #expect(RepoRef(parsing: url) == nil)
    }

    @Test func defaultRefIsHEADAndPathsArePrefixedWithSubPath() {
        let plain = RepoRef(owner: "o", name: "r")
        let nested = RepoRef(owner: "o", name: "r", ref: "main", subPath: "themes/foo")

        #expect(plain.effectiveRef == "HEAD")
        #expect(plain.repoPath("colors.toml") == "colors.toml")
        #expect(nested.repoPath("colors.toml") == "themes/foo/colors.toml")
        #expect(plain.htmlURL.absoluteString == "https://github.com/o/r")
        #expect(nested.htmlURL.absoluteString == "https://github.com/o/r/tree/main/themes/foo")
    }

    @Test func rawURLsEscapePathSegments() {
        let url = GitHubClient.rawURL(RepoRef(owner: "o", name: "r", subPath: "my themes"), "backgrounds/1 dark.png")
        #expect(url.absoluteString == "https://raw.githubusercontent.com/o/r/HEAD/my%20themes/backgrounds/1%20dark.png")
    }
}
