import Testing
@testable import OmarchyThemesKit

struct PaletteParserTests {
    @Test func parsesTerminalStyleColorsToml() throws {
        let p = try ColorsTomlParser.parse(Fixture.read("colors-ansi.toml"))

        #expect(p.background == RgbColor("#0e091d"))
        #expect(p.foreground == RgbColor("#14b9b5"))
        #expect(p.accent == RgbColor("#be3f50"))
        #expect(p.cursor == RgbColor("#ff7f41"))
        #expect(p.selection == RgbColor("#14b9b5"))
        #expect(p.swatches.count == 16)
        #expect(p.swatches[0] == NamedColor(name: "Black", color: RgbColor("#000000")))
        #expect(p.swatches[15].name == "Bright white")
        #expect(p.declaredMode == nil)
        #expect(p.mode == .dark)
        #expect(p.source == .colorsToml)
    }

    @Test func parsesNamedColorsTomlWithExplicitMode() throws {
        let p = try ColorsTomlParser.parse(Fixture.read("colors-named.toml"))

        #expect(p.declaredMode == .light)
        #expect(p.mode == .light)
        #expect(p.accent == RgbColor("#2e7de9"))
        #expect(p.selection == RgbColor("#b7c1e3"))
        #expect(p.swatches.map(\.name) == ["Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Brown", "Bright red", "Bright blue"])
    }

    @Test func accentFallsBackToColor4ThenForeground() throws {
        let withColor4 = try ColorsTomlParser.parse("""
            background = "#000000"
            foreground = "#ffffff"
            color4 = "#0000ff"
            """)
        let bare = try ColorsTomlParser.parse("""
            background = "#000000"
            foreground = "#eeeeee"
            """)

        #expect(withColor4.accent == RgbColor("#0000ff"))
        #expect(bare.accent == RgbColor("#eeeeee"))
    }

    @Test func infersLightModeFromBackgroundWhenNotDeclared() throws {
        let p = try ColorsTomlParser.parse("""
            background = "#fdf6e3"
            foreground = "#657b83"
            """)

        #expect(p.declaredMode == nil)
        #expect(p.mode == .light)
    }

    @Test func fallsBackToLenientParsingForInvalidToml() throws {
        // Duplicate keys and a stray line make this invalid TOML, but the colors are still usable.
        let p = try ColorsTomlParser.parse("""
            background = "#101010"
            foreground = "#f0f0f0"
            foreground = "#e0e0e0"
            this line is not toml
            accent = '#ff0000'
            """)

        #expect(p.background == RgbColor("#101010"))
        #expect(p.foreground == RgbColor("#e0e0e0"))
        #expect(p.accent == RgbColor("#ff0000"))
    }

    @Test(arguments: [
        "",
        "foreground = \"#ffffff\"",
        "background = \"not-a-color\"\nforeground = \"#ffffff\"",
    ])
    func rejectsColorsTomlWithoutBackgroundOrForeground(text: String) {
        #expect(throws: PaletteParseError.self) { try ColorsTomlParser.parse(text) }
    }

    @Test func parsesAlacrittyToml() throws {
        let p = try AlacrittyParser.parse(Fixture.read("alacritty.toml"))

        #expect(p.background == RgbColor("#1b1112"))
        #expect(p.foreground == RgbColor("#f2e8e8"))
        #expect(p.accent == RgbColor("#d66b6b"))
        #expect(p.cursor == RgbColor("#eaeaea"))
        #expect(p.selection == RgbColor("#372223"))
        #expect(p.swatches.count == 16)
        #expect(p.swatches[8] == NamedColor(name: "Bright black", color: RgbColor("#2b1818")))
        #expect(p.source == .alacritty)
    }

    @Test func rejectsAlacrittyWithoutPrimaryColors() {
        #expect(throws: PaletteParseError.self) { try AlacrittyParser.parse("[font]\nsize = 12") }
    }
}

/// The strict TOML reader, for syntax the lenient scanner can't handle.
struct FlatTomlTests {
    @Test func readsInlineTablesQuotedAndDottedKeys() {
        let values = FlatToml.parse("""
            # comment
            [colors]
            primary = { background = "#1e1e2e", foreground = "#cdd6f4" } # trailing comment
            "quoted key" = "a"
            normal.blue = "#89b4fa"

            [colors.bright]
            red = 'literal \\n kept'
            """)

        #expect(values["colors.primary.background"] == "#1e1e2e")
        #expect(values["colors.primary.foreground"] == "#cdd6f4")
        #expect(values["colors.quoted key"] == "a")
        #expect(values["colors.normal.blue"] == "#89b4fa")
        #expect(values["colors.bright.red"] == "literal \\n kept")
    }

    @Test func handlesEscapesMultilineStringsAndSkipsOtherValues() {
        let values = FlatToml.parse(#"""
            name = "Rosé \"Pine\""
            description = """
            two
            lines"""
            size = 12.5
            enabled = true
            when = 1979-05-27 07:32:00Z
            list = [
              "#000000", # comment
              ["nested"],
            ]
            Accent = "#ABCDEF"
            """#)

        #expect(values["name"] == "Rosé \"Pine\"")
        #expect(values["description"] == "two\nlines")
        #expect(values["size"] == nil)
        #expect(values["list"] == nil)
        #expect(values["accent"] == "#ABCDEF") // keys are case-insensitive
    }

    @Test func lenientScannerIsUsedForBrokenFiles() {
        let values = FlatToml.parse("""
            [colors.primary]
            background = "#000000"
            oops =
            foreground = "#ffffff"
            """)

        #expect(values["colors.primary.background"] == "#000000")
        #expect(values["colors.primary.foreground"] == "#ffffff")
    }
}
