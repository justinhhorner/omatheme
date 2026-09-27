using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Tests.TestSupport;

namespace OmarchyThemes.Core.Tests;

public class PaletteParserTests
{
    [Fact]
    public void Parses_terminal_style_colors_toml()
    {
        var p = ColorsTomlParser.Parse(Fixture.Read("colors-ansi.toml"));

        Assert.Equal(RgbColor.Parse("#0e091d"), p.Background);
        Assert.Equal(RgbColor.Parse("#14b9b5"), p.Foreground);
        Assert.Equal(RgbColor.Parse("#be3f50"), p.Accent);
        Assert.Equal(RgbColor.Parse("#ff7f41"), p.Cursor);
        Assert.Equal(RgbColor.Parse("#14b9b5"), p.Selection);
        Assert.Equal(16, p.Swatches.Count);
        Assert.Equal(new NamedColor("Black", RgbColor.Parse("#000000")), p.Swatches[0]);
        Assert.Equal("Bright white", p.Swatches[15].Name);
        Assert.Null(p.DeclaredMode);
        Assert.Equal(AppearanceMode.Dark, p.Mode);
        Assert.Equal(PaletteSource.ColorsToml, p.Source);
    }

    [Fact]
    public void Parses_named_colors_toml_with_explicit_mode()
    {
        var p = ColorsTomlParser.Parse(Fixture.Read("colors-named.toml"));

        Assert.Equal(AppearanceMode.Light, p.DeclaredMode);
        Assert.Equal(AppearanceMode.Light, p.Mode);
        Assert.Equal(RgbColor.Parse("#2e7de9"), p.Accent);
        Assert.Equal(RgbColor.Parse("#b7c1e3"), p.Selection);
        Assert.Equal(["Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Brown", "Bright red", "Bright blue"],
            p.Swatches.Select(s => s.Name));
    }

    [Fact]
    public void Accent_falls_back_to_color4_then_foreground()
    {
        var withColor4 = ColorsTomlParser.Parse("""
            background = "#000000"
            foreground = "#ffffff"
            color4 = "#0000ff"
            """);
        var bare = ColorsTomlParser.Parse("""
            background = "#000000"
            foreground = "#eeeeee"
            """);

        Assert.Equal(RgbColor.Parse("#0000ff"), withColor4.Accent);
        Assert.Equal(RgbColor.Parse("#eeeeee"), bare.Accent);
    }

    [Fact]
    public void Infers_light_mode_from_background_when_not_declared()
    {
        var p = ColorsTomlParser.Parse("""
            background = "#fdf6e3"
            foreground = "#657b83"
            """);

        Assert.Null(p.DeclaredMode);
        Assert.Equal(AppearanceMode.Light, p.Mode);
    }

    [Fact]
    public void Falls_back_to_lenient_parsing_for_invalid_toml()
    {
        // Duplicate keys and a stray line make this invalid TOML, but the colors are still usable.
        var p = ColorsTomlParser.Parse("""
            background = "#101010"
            foreground = "#f0f0f0"
            foreground = "#e0e0e0"
            this line is not toml
            accent = '#ff0000'
            """);

        Assert.Equal(RgbColor.Parse("#101010"), p.Background);
        Assert.Equal(RgbColor.Parse("#e0e0e0"), p.Foreground);
        Assert.Equal(RgbColor.Parse("#ff0000"), p.Accent);
    }

    [Theory]
    [InlineData("")]
    [InlineData("foreground = \"#ffffff\"")]
    [InlineData("background = \"not-a-color\"\nforeground = \"#ffffff\"")]
    public void Rejects_colors_toml_without_background_or_foreground(string text)
    {
        Assert.Throws<PaletteParseException>(() => ColorsTomlParser.Parse(text));
    }

    [Fact]
    public void Parses_alacritty_toml()
    {
        var p = AlacrittyParser.Parse(Fixture.Read("alacritty.toml"));

        Assert.Equal(RgbColor.Parse("#1b1112"), p.Background);
        Assert.Equal(RgbColor.Parse("#f2e8e8"), p.Foreground);
        Assert.Equal(RgbColor.Parse("#d66b6b"), p.Accent);
        Assert.Equal(RgbColor.Parse("#eaeaea"), p.Cursor);
        Assert.Equal(RgbColor.Parse("#372223"), p.Selection);
        Assert.Equal(16, p.Swatches.Count);
        Assert.Equal(new NamedColor("Bright black", RgbColor.Parse("#2b1818")), p.Swatches[8]);
        Assert.Equal(PaletteSource.Alacritty, p.Source);
    }

    [Fact]
    public void Rejects_alacritty_without_primary_colors()
    {
        Assert.Throws<PaletteParseException>(() => AlacrittyParser.Parse("[font]\nsize = 12"));
    }
}
