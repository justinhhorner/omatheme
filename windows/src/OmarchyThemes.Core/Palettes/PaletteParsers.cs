using OmarchyThemes.Core.Colors;

namespace OmarchyThemes.Core.Palettes;

/// <summary>
/// Omarchy's colors.toml. Two shapes exist in the wild:
/// <list type="bullet">
/// <item>Terminal-style: accent, cursor, foreground, background, selection_*, color0..color15.</item>
/// <item>Named (newer Omarchy): mode, accent, background, foreground, red, bright_red, … .</item>
/// </list>
/// Both are accepted, and a file mixing them is fine.
/// </summary>
public static class ColorsTomlParser
{
    private static readonly string[] AnsiNames =
        ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

    private static readonly string[] NamedKeys =
    [
        "red", "orange", "yellow", "green", "cyan", "blue", "magenta", "brown",
        "bright_red", "bright_yellow", "bright_green", "bright_cyan", "bright_blue", "bright_magenta",
    ];

    public static Palette Parse(string text)
    {
        var values = FlatToml.Parse(text);
        var get = ColorLookup(values);

        var background = get("background") ?? throw new PaletteParseException("colors.toml has no valid 'background' color.");
        var foreground = get("foreground") ?? throw new PaletteParseException("colors.toml has no valid 'foreground' color.");

        var swatches = new List<NamedColor>();
        for (var i = 0; i < 16; i++)
        {
            if (get($"color{i}") is { } c)
                swatches.Add(new NamedColor(i < 8 ? AnsiNames[i] : "Bright " + AnsiNames[i - 8].ToLowerInvariant(), c));
        }
        if (swatches.Count == 0)
        {
            foreach (var key in NamedKeys)
            {
                if (get(key) is { } c)
                    swatches.Add(new NamedColor(Humanize(key), c));
            }
        }

        return new Palette
        {
            Background = background,
            Foreground = foreground,
            Accent = get("accent") ?? get("color4") ?? get("blue") ?? foreground,
            Cursor = get("cursor"),
            Selection = get("selection_background") ?? get("selection"),
            Muted = get("muted"),
            BrightForeground = get("bright_foreground"),
            DeclaredMode = ParseMode(values.GetValueOrDefault("mode")),
            Swatches = swatches,
            Source = PaletteSource.ColorsToml,
        };
    }

    internal static AppearanceMode? ParseMode(string? mode) => mode?.Trim().ToLowerInvariant() switch
    {
        "light" => AppearanceMode.Light,
        "dark" => AppearanceMode.Dark,
        _ => null,
    };

    internal static Func<string, RgbColor?> ColorLookup(IReadOnlyDictionary<string, string> values) =>
        key => values.TryGetValue(key, out var s) && RgbColor.TryParse(s, out var c) ? c : null;

    private static string Humanize(string key) =>
        char.ToUpperInvariant(key[0]) + key[1..].Replace('_', ' ');
}

/// <summary>alacritty.toml, the fallback for themes that predate colors.toml.</summary>
public static class AlacrittyParser
{
    private static readonly string[] Names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];

    public static Palette Parse(string text)
    {
        var values = FlatToml.Parse(text);
        var get = ColorsTomlParser.ColorLookup(values);

        var background = get("colors.primary.background")
            ?? throw new PaletteParseException("alacritty.toml has no valid [colors.primary] background.");
        var foreground = get("colors.primary.foreground")
            ?? throw new PaletteParseException("alacritty.toml has no valid [colors.primary] foreground.");

        var swatches = new List<NamedColor>();
        foreach (var (group, label) in new[] { ("normal", ""), ("bright", "Bright ") })
        {
            foreach (var name in Names)
            {
                if (get($"colors.{group}.{name}") is { } c)
                    swatches.Add(new NamedColor(label.Length == 0 ? char.ToUpperInvariant(name[0]) + name[1..] : label + name, c));
            }
        }

        return new Palette
        {
            Background = background,
            Foreground = foreground,
            Accent = get("colors.normal.blue") ?? foreground,
            Cursor = get("colors.cursor.cursor"),
            Selection = get("colors.selection.background"),
            Swatches = swatches,
            Source = PaletteSource.Alacritty,
        };
    }
}
