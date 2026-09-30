using System.Text;
using System.Text.Json;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;

namespace OmarchyThemes.Platform.Windows.Tests;

/// <summary>Writes fragments to a temp folder, never Windows Terminal's real one.</summary>
public sealed class WindowsTerminalSchemesTests : IDisposable
{
    private static readonly string[] ColorKeys =
    [
        "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
        "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
    ];

    private readonly string _dir = Path.Combine(Path.GetTempPath(), "omatheme-wt-tests", Guid.NewGuid().ToString("N"), "OmarchyThemes");
    private readonly WindowsTerminalSchemes _schemes;

    public WindowsTerminalSchemesTests() => _schemes = new WindowsTerminalSchemes(_dir, isAvailable: true);

    public void Dispose()
    {
        try { Directory.Delete(Path.GetDirectoryName(_dir)!, recursive: true); }
        catch (IOException) { }
    }

    private static TerminalColors Colors() => new(
        RgbColor.Parse("#1a1b26"), RgbColor.Parse("#a9b1d6"), RgbColor.Parse("#c0caf5"), RgbColor.Parse("#292e42"),
        Enumerable.Range(0, 16).Select(i => new RgbColor((byte)(i * 16), 0x20, 0x30)).ToList());

    [Fact]
    public void Adds_a_complete_named_scheme_as_a_fragment()
    {
        _schemes.Add("omarchy.tokyo-night", "Tokyo Night", Colors());

        Assert.True(_schemes.IsAdded("omarchy.tokyo-night"));
        var bytes = File.ReadAllBytes(Path.Combine(_dir, "omarchy.tokyo-night.json"));
        Assert.False(bytes.AsSpan().StartsWith(Encoding.UTF8.Preamble), "Terminal needs UTF-8 without a BOM");

        using var json = JsonDocument.Parse(bytes);
        var scheme = Assert.Single(json.RootElement.GetProperty("schemes").EnumerateArray());
        Assert.Equal("Tokyo Night (Omarchy)", scheme.GetProperty("name").GetString());
        Assert.Equal("#1a1b26", scheme.GetProperty("background").GetString());
        Assert.Equal("#a9b1d6", scheme.GetProperty("foreground").GetString());
        Assert.Equal("#c0caf5", scheme.GetProperty("cursorColor").GetString());
        Assert.Equal("#292e42", scheme.GetProperty("selectionBackground").GetString());
        // Terminal ignores a scheme that doesn't define every color in the table.
        for (var i = 0; i < ColorKeys.Length; i++)
            Assert.Equal(new RgbColor((byte)(i * 16), 0x20, 0x30).ToHex(), scheme.GetProperty(ColorKeys[i]).GetString());
        Assert.False(json.RootElement.TryGetProperty("profiles", out _), "Only schemes; the user's profiles are left alone");
    }

    [Fact]
    public void Adding_again_replaces_and_removing_cleans_up()
    {
        _schemes.Add("aetheria", "Aetheria", Colors());
        _schemes.Add("aetheria", "Aetheria", Colors());
        Assert.Single(Directory.GetFiles(_dir));

        _schemes.Remove("aetheria");

        Assert.False(_schemes.IsAdded("aetheria"));
        Assert.False(Directory.Exists(_dir), "The empty app folder is removed too");
        _schemes.Remove("aetheria"); // removing twice is fine
    }

    [Fact]
    public void Names_with_quotes_and_accents_stay_valid_json()
    {
        _schemes.Add("rose-pine", "Rosé \"Pine\"", Colors());

        using var json = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(_dir, "rose-pine.json")));
        Assert.Equal("Rosé \"Pine\" (Omarchy)", json.RootElement.GetProperty("schemes")[0].GetProperty("name").GetString());
    }

    [Theory]
    [InlineData("../evil")]
    [InlineData("a\\b")]
    [InlineData("")]
    public void Rejects_slugs_that_could_escape_the_folder(string slug)
    {
        Assert.Throws<ArgumentException>(() => _schemes.Add(slug, "X", Colors()));
    }

    [Fact]
    public void Default_folder_is_terminals_per_user_fragments_folder()
    {
        Assert.EndsWith(@"Microsoft\Windows Terminal\Fragments\OmarchyThemes", WindowsTerminalSchemes.DefaultFragmentsDir);
    }
}
