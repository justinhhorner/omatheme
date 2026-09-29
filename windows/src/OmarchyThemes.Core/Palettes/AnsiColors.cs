namespace OmarchyThemes.Core.Palettes;

/// <summary>
/// Names of the 16 ANSI terminal colors as palette swatches: "Black" … "White", then "Bright black" …
/// "Bright white". The palette parsers name swatches this way and <see cref="TerminalColors"/> looks them
/// up by these names, so they must agree.
/// </summary>
public static class AnsiColors
{
    /// <summary>The 8 normal colors, in ANSI order.</summary>
    public static readonly string[] Names = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

    /// <summary>The swatch name of ANSI color <paramref name="index"/> (0–15).</summary>
    public static string SwatchName(int index) =>
        index < 8 ? Names[index] : "Bright " + Names[index - 8].ToLowerInvariant();
}
