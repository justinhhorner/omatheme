using System.Text.Json;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

public class ColorTests
{
    [Theory]
    [InlineData("#7aa2f7", 0x7a, 0xa2, 0xf7)]
    [InlineData("7AA2F7", 0x7a, 0xa2, 0xf7)]
    [InlineData("0x7aa2f7", 0x7a, 0xa2, 0xf7)]
    [InlineData("#fff", 0xff, 0xff, 0xff)]
    [InlineData("#7aa2f7cc", 0x7a, 0xa2, 0xf7)]
    [InlineData("  #000000 ", 0, 0, 0)]
    public void Parses_hex_forms(string text, byte r, byte g, byte b)
    {
        Assert.True(RgbColor.TryParse(text, out var c));
        Assert.Equal(new RgbColor(r, g, b), c);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("CellForeground")]
    [InlineData("#12345")]
    [InlineData("#gggggg")]
    public void Rejects_non_colors(string? text)
    {
        Assert.False(RgbColor.TryParse(text, out _));
    }

    [Fact]
    public void Round_trips_through_hsl()
    {
        foreach (var hex in new[] { "#7aa2f7", "#be3f50", "#000000", "#ffffff", "#9ece6a", "#808080" })
        {
            var c = RgbColor.Parse(hex);
            var (h, s, l) = c.ToHsl();
            Assert.Equal(c, RgbColor.FromHsl(h, s, l));
        }
    }

    [Fact]
    public void Serializes_as_hex_string()
    {
        var json = JsonSerializer.Serialize(new { c = RgbColor.Parse("#7AA2F7") }, JsonFile.Options);
        Assert.Contains("\"#7aa2f7\"", json);
        Assert.Equal(RgbColor.Parse("#7aa2f7"), JsonSerializer.Deserialize<RgbColor>("\"#7aa2f7\"", JsonFile.Options));
    }

    [Fact]
    public void Light_and_dark_backgrounds_are_classified()
    {
        Assert.False(RgbColor.Parse("#1a1b26").IsLight);
        Assert.True(RgbColor.Parse("#fdf6e3").IsLight);
        Assert.True(RgbColor.Parse("#e1e2e7").IsLight);
    }
}

public class AccentMathTests
{
    private static readonly RgbColor Accent = new(0x12, 0x34, 0x56);

    [Fact]
    public void Packs_abgr_and_argb()
    {
        Assert.Equal(0xFF563412u, AccentMath.ToAbgr(Accent));
        Assert.Equal(0xC4123456u, AccentMath.ToArgb(Accent, 0xC4));
        Assert.Equal(Accent, AccentMath.FromAbgr(AccentMath.ToAbgr(Accent)));
    }

    [Fact]
    public void Shades_run_from_lightest_to_darkest_around_the_accent()
    {
        var accent = RgbColor.Parse("#0078d4");
        var shades = AccentMath.Shades(accent);

        Assert.Equal(7, shades.Count);
        Assert.Equal(accent, shades[3]);
        var lightness = shades.Select(s => s.ToHsl().L).ToList();
        Assert.Equal(lightness.OrderByDescending(l => l), lightness);
    }

    [Fact]
    public void Accent_palette_is_32_bytes_of_rgba_with_accent_in_slot_3()
    {
        var accent = RgbColor.Parse("#0078d4");
        var bytes = AccentMath.ToAccentPaletteBytes(accent);

        Assert.Equal(32, bytes.Length);
        Assert.Equal(new byte[] { 0x00, 0x78, 0xd4, 0x00 }, bytes[12..16]);
    }

    [Theory]
    [InlineData("#000000")]
    [InlineData("#050505")]
    [InlineData("#ffffff")]
    [InlineData("#fdf6e3")]
    public void Normalizes_unusable_accents_into_a_readable_lightness_range(string hex)
    {
        var (_, _, l) = AccentMath.NormalizeAccent(RgbColor.Parse(hex)).ToHsl();
        Assert.InRange(l, 0.24, 0.76);
    }

    [Fact]
    public void Leaves_reasonable_accents_alone()
    {
        var accent = RgbColor.Parse("#7aa2f7");
        Assert.Equal(accent, AccentMath.NormalizeAccent(accent));
    }
}
