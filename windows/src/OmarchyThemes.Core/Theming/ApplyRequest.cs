using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Core.Theming;

/// <summary>The user's per-aspect choices in the Apply dialog.</summary>
public sealed record ApplyOptions
{
    public bool Wallpaper { get; init; } = true;
    public bool AppearanceMode { get; init; } = true;
    public bool AccentColor { get; init; } = true;
    public WallpaperFit Fit { get; init; } = WallpaperFit.Fill;
}

/// <summary>What to apply. Built from a downloaded theme, so no network is involved.</summary>
public sealed record ApplyRequest(string ThemeName, string? WallpaperPath, AppearanceMode Mode, RgbColor? Accent, ApplyOptions Options)
{
    /// <summary>The theme's background color, used as the desktop fill around Fit/Center wallpapers.</summary>
    public RgbColor? Background { get; init; }

    /// <param name="wallpaperFile">One of <see cref="InstalledTheme.Wallpapers"/>; defaults to the first.</param>
    public static ApplyRequest FromInstalled(InstalledTheme theme, string? wallpaperFile, ApplyOptions options)
    {
        var file = wallpaperFile is not null && theme.Wallpapers.Contains(wallpaperFile)
            ? wallpaperFile
            : theme.Wallpapers.FirstOrDefault();
        return new ApplyRequest(theme.Name, file is null ? null : theme.WallpaperPath(file), theme.Mode, theme.Palette?.Accent, options)
        {
            Background = theme.Palette?.Background,
        };
    }
}
