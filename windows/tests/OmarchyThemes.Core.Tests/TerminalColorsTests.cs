using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Tests.TestSupport;

namespace OmarchyThemes.Core.Tests;

public class TerminalColorsTests
{
    private static RgbColor C(string hex) => RgbColor.Parse(hex);

    [Fact]
    public void Named_colors_toml_follows_omarchys_terminal_template()
    {
        var palette = ColorsTomlParser.Parse("""
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
            """);

        var t = TerminalColors.From(palette);

        Assert.Equal(C("#1a1b26"), t.Background);
        Assert.Equal(C("#a9b1d6"), t.Foreground);
        Assert.Equal(C("#c0caf5"), t.Cursor);              // bright_foreground
        Assert.Equal(C("#292e42"), t.SelectionBackground);
        Assert.Equal(
            [
                C("#1a1b26"), C("#f7768e"), C("#9ece6a"), C("#e0af68"), C("#7aa2f7"), C("#ad8ee6"), C("#449dab"), C("#a9b1d6"),
                C("#414868"), C("#ff7a93"), C("#9ece6a"), C("#e0af68"), C("#7da6ff"), C("#ad8ee6"), C("#449dab"), C("#c0caf5"),
            ],
            t.Ansi);
    }

    [Fact]
    public void Terminal_style_colors_toml_uses_its_16_colors_as_is()
    {
        var t = TerminalColors.From(ColorsTomlParser.Parse(Fixture.Read("colors-ansi.toml")));

        Assert.Equal(16, t.Ansi.Count);
        Assert.Equal(C("#000000"), t.Ansi[0]);
        Assert.Equal(C("#c8e967"), t.Ansi[1]);
        Assert.Equal(C("#c53253"), t.Ansi[8]);
        Assert.Equal(C("#11aeb3"), t.Ansi[15]);
        Assert.Equal(C("#ff7f41"), t.Cursor);
    }

    [Fact]
    public void Alacritty_uses_its_normal_and_bright_colors()
    {
        var t = TerminalColors.From(AlacrittyParser.Parse(Fixture.Read("alacritty.toml")));

        Assert.Equal(C("#1a0e0e"), t.Ansi[0]);
        Assert.Equal(C("#f2e8e8"), t.Ansi[7]);
        Assert.Equal(C("#2b1818"), t.Ansi[8]);  // 0x-prefixed in the file
        Assert.Equal(C("#fff1f1"), t.Ansi[15]);
        Assert.Equal(C("#eaeaea"), t.Cursor);
    }

    [Fact]
    public void Missing_colors_have_sensible_fallbacks()
    {
        var t = TerminalColors.From(ColorsTomlParser.Parse("""
            background = "#000000"
            foreground = "#ffffff"
            """));

        Assert.Equal(C("#000000"), t.Ansi[0]);           // black = background
        Assert.Equal(C("#ffffff"), t.Ansi[1]);           // no red: foreground
        Assert.Equal(C("#ffffff"), t.Ansi[7]);           // white = foreground
        Assert.Equal(C("#595959"), t.Ansi[8]);           // bright black: between background and foreground
        Assert.Equal(C("#ffffff"), t.Ansi[15]);          // bright white: white
        Assert.Equal(C("#ffffff"), t.Cursor);            // cursor: foreground
        Assert.Equal(C("#333333"), t.SelectionBackground);
    }
}
