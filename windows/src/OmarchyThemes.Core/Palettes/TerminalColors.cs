using OmarchyThemes.Core.Colors;

namespace OmarchyThemes.Core.Palettes;

/// <summary>
/// A theme's colors as a terminal color scheme: background, foreground, cursor, selection and the
/// 16 ANSI colors. For Omarchy's named colors.toml this follows Omarchy's own terminal template
/// (default/themed/alacritty.toml.tpl): black = background, white = foreground, bright black = muted,
/// bright white and the cursor = bright_foreground. Themes that ship the 16 colors (color0..15 or
/// alacritty.toml) use them as-is. Missing bright colors fall back to their normal ones.
/// </summary>
/// <param name="Ansi">16 colors: black, red, green, yellow, blue, magenta, cyan, white, then the bright ones.</param>
public sealed record TerminalColors(
    RgbColor Background,
    RgbColor Foreground,
    RgbColor Cursor,
    RgbColor SelectionBackground,
    IReadOnlyList<RgbColor> Ansi)
{
    public static TerminalColors From(Palette palette)
    {
        var swatches = palette.Swatches
            .GroupBy(s => s.Name, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => g.First().Color, StringComparer.OrdinalIgnoreCase);
        RgbColor? Swatch(string name) => swatches.TryGetValue(name, out var c) ? c : null;

        // A neutral between background and foreground, for themes with neither muted nor a bright black.
        var between = Mix(palette.Background, palette.Foreground, 0.35);

        var normal = AnsiColors.Names.Select(name => name switch
        {
            "Black" => Swatch("Black") ?? palette.Background,
            "White" => Swatch("White") ?? palette.Foreground,
            _ => Swatch(name) ?? palette.Foreground,
        }).ToArray();

        var bright = AnsiColors.Names.Select((name, i) => name switch
        {
            "Black" => Swatch("Bright black") ?? palette.Muted ?? between,
            "White" => Swatch("Bright white") ?? palette.BrightForeground ?? normal[i],
            _ => Swatch(AnsiColors.SwatchName(i + 8)) ?? normal[i],
        }).ToArray();

        return new TerminalColors(
            palette.Background,
            palette.Foreground,
            palette.Cursor ?? palette.BrightForeground ?? palette.Foreground,
            palette.Selection ?? Mix(palette.Background, palette.Foreground, 0.2),
            [.. normal, .. bright]);
    }

    private static RgbColor Mix(RgbColor a, RgbColor b, double t) => new(
        (byte)Math.Round(a.R + (b.R - a.R) * t),
        (byte)Math.Round(a.G + (b.G - a.G) * t),
        (byte)Math.Round(a.B + (b.B - a.B) * t));
}
