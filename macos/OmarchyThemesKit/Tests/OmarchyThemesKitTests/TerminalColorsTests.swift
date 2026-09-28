import Testing
@testable import OmarchyThemesKit

/// Mirrors the Windows `TerminalColorsTests`, so both apps export the same colors.
struct TerminalColorsTests {
    @Test func namedColorsTomlFollowsOmarchysTerminalTemplate() throws {
        let palette = try ColorsTomlParser.parse("""
            background = "#1a1b26"
            foreground = "#a9b1d6"
            muted = "#414868"
            bright_foreground = "#c0caf5"
            selection = "#292e42"
            red = "#f7768e"
            green = "#9ece6a"
            yellow = "#e0af68"
            blue = "#7aa2f7"
            magenta = "#ad8ee6"
            cyan = "#449dab"
            orange = "#eb927b"
            bright_red = "#ff7a93"
            bright_blue = "#7da6ff"
            """)
        #expect(palette.muted == RgbColor("#414868"))
        #expect(palette.brightForeground == RgbColor("#c0caf5"))

        let t = TerminalColors(palette)

        #expect(t.background == RgbColor("#1a1b26"))
        #expect(t.foreground == RgbColor("#a9b1d6"))
        #expect(t.cursor == RgbColor("#c0caf5")) // bright_foreground
        #expect(t.selectionBackground == RgbColor("#292e42"))
        #expect(t.ansi == [
            "#1a1b26", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#ad8ee6", "#449dab", "#a9b1d6",
            "#414868", "#ff7a93", "#9ece6a", "#e0af68", "#7da6ff", "#ad8ee6", "#449dab", "#c0caf5",
        ].map { RgbColor($0) })
    }

    @Test func terminalStyleColorsTomlUsesIts16ColorsAsIs() throws {
        let t = TerminalColors(try ColorsTomlParser.parse(Fixture.read("colors-ansi.toml")))

        #expect(t.ansi.count == 16)
        #expect(t.ansi[0] == RgbColor("#000000"))
        #expect(t.ansi[1] == RgbColor("#c8e967"))
        #expect(t.ansi[8] == RgbColor("#c53253"))
        #expect(t.ansi[15] == RgbColor("#11aeb3"))
        #expect(t.cursor == RgbColor("#ff7f41"))
    }

    @Test func alacrittyUsesItsNormalAndBrightColors() throws {
        let t = TerminalColors(try AlacrittyParser.parse(Fixture.read("alacritty.toml")))

        #expect(t.ansi[0] == RgbColor("#1a0e0e"))
        #expect(t.ansi[7] == RgbColor("#f2e8e8"))
        #expect(t.ansi[8] == RgbColor("#2b1818")) // 0x-prefixed in the file
        #expect(t.ansi[15] == RgbColor("#fff1f1"))
        #expect(t.cursor == RgbColor("#eaeaea"))
    }

    @Test func missingColorsHaveSensibleFallbacks() throws {
        let t = TerminalColors(try ColorsTomlParser.parse("""
            background = "#000000"
            foreground = "#ffffff"
            """))

        #expect(t.ansi[0] == RgbColor("#000000"))  // black = background
        #expect(t.ansi[1] == RgbColor("#ffffff"))  // no red: foreground
        #expect(t.ansi[7] == RgbColor("#ffffff"))  // white = foreground
        #expect(t.ansi[8] == RgbColor("#595959"))  // bright black: between background and foreground
        #expect(t.ansi[15] == RgbColor("#ffffff")) // bright white: white
        #expect(t.cursor == RgbColor("#ffffff"))   // cursor: foreground
        #expect(t.selectionBackground == RgbColor("#333333"))
    }
}

/// Mirrors the Windows `ThemeStoreTests` "Applying_…" cases.
struct AfterApplyTests {
    static func result(_ outcomes: StepOutcome...) -> ApplyResult {
        let steps: [ApplyStep] = [.wallpaper, .appearanceMode, .accentColor]
        return ApplyResult(steps: zip(steps, outcomes).map { StepResult($0, $1) })
    }

    @Test func applyingRecordsTheThemeAndTheWallpaperThatWasSet() {
        let after = AppSettings().afterApply("tokyo", wallpaperFile: "2.png", result: Self.result(.applied))

        #expect(after.lastAppliedSlug == "tokyo")
        #expect(after.lastAppliedWallpaper == "2.png")
    }

    @Test func applyingWithoutTheWallpaperDoesNotClaimItsWallpaperIsOnTheDesktop() {
        var before = AppSettings()
        before.lastAppliedSlug = "tokyo"
        before.lastAppliedWallpaper = "2.png"

        // Same theme, wallpaper unchecked: the desktop still shows 2.png.
        #expect(before.afterApply("tokyo", wallpaperFile: "1.png", result: Self.result(.skippedByUser, .applied)).lastAppliedWallpaper == "2.png")

        // Another theme, wallpaper unchecked or failed: none of its wallpapers is on the desktop.
        let other = before.afterApply("snow", wallpaperFile: "1.png", result: Self.result(.failed, .applied))
        #expect(other.lastAppliedSlug == "snow")
        #expect(other.lastAppliedWallpaper == nil)
    }

    @Test func applyingNothingChangesNothing() {
        var before = AppSettings()
        before.lastAppliedSlug = "tokyo"
        before.lastAppliedWallpaper = "2.png"

        #expect(before.afterApply("snow", wallpaperFile: "1.png", result: Self.result(.failed, .failed)) == before)
    }
}
