using OmarchyThemes.Core.Colors;

namespace OmarchyThemes.Core.Palettes;

public enum AppearanceMode
{
    Dark,
    Light,
}

/// <summary>Which file in the theme repo the palette was read from.</summary>
public enum PaletteSource
{
    ColorsToml,
    Alacritty,
}

public sealed record NamedColor(string Name, RgbColor Color);

/// <summary>A theme's colors, normalized from whichever file format the theme ships.</summary>
public sealed record Palette
{
    public required RgbColor Background { get; init; }
    public required RgbColor Foreground { get; init; }
    public required RgbColor Accent { get; init; }
    public RgbColor? Cursor { get; init; }
    public RgbColor? Selection { get; init; }

    /// <summary>Mode stated by the theme itself (colors.toml `mode` or a light.mode file).</summary>
    public AppearanceMode? DeclaredMode { get; init; }

    /// <summary>Terminal-style colors for display, in a stable order.</summary>
    public IReadOnlyList<NamedColor> Swatches { get; init; } = [];

    public required PaletteSource Source { get; init; }

    /// <summary>The declared mode, otherwise inferred from the background's luminance.</summary>
    public AppearanceMode Mode => DeclaredMode ?? (Background.IsLight ? AppearanceMode.Light : AppearanceMode.Dark);
}

public sealed class PaletteParseException(string message, Exception? inner = null) : Exception(message, inner);
