using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;

namespace OmarchyThemes.Core.Theming;

/// <summary>What a platform backend can change. Add a flag here when adding a new theming API.</summary>
[Flags]
public enum DesktopCapabilities
{
    None = 0,
    Wallpaper = 1 << 0,
    AppearanceMode = 1 << 1,
    AccentColor = 1 << 2,
}

public enum WallpaperFit
{
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
    Span,
}

/// <summary>
/// Opaque, backend-defined record of the user's desktop before we changed it. Core only stores
/// and returns it; the backend decides what goes in <see cref="Values"/>.
/// </summary>
public sealed record DesktopSnapshot(DateTimeOffset TakenAt, IReadOnlyDictionary<string, string> Values);

/// <summary>
/// The OS-specific side of theming. Implementations call the platform's real APIs
/// (Windows: IDesktopWallpaper + Personalize/DWM registry values). Methods for capabilities the
/// backend doesn't report are never called.
/// </summary>
public interface IDesktopBackend
{
    DesktopCapabilities Capabilities { get; }

    /// <summary>
    /// Added to the summary after the accent color is applied, for what the user should know about how
    /// this OS picks it up (e.g. that some parts only update after signing out). Null for nothing.
    /// </summary>
    string? AccentColorNote => null;

    Task<DesktopSnapshot> CaptureAsync(CancellationToken ct = default);

    Task RestoreAsync(DesktopSnapshot snapshot, CancellationToken ct = default);

    /// <summary>Sets <paramref name="imagePath"/> (a local file) as the wallpaper on every monitor.</summary>
    /// <param name="fillColor">
    /// The desktop color around the image when it doesn't cover the screen (Fit, Center); the theme's
    /// background, so the wallpaper is framed in the theme's color instead of black. Null leaves it unchanged.
    /// </param>
    Task SetWallpaperAsync(string imagePath, WallpaperFit fit, RgbColor? fillColor, CancellationToken ct = default);

    Task SetAppearanceModeAsync(AppearanceMode mode, CancellationToken ct = default);

    Task SetAccentColorAsync(RgbColor accent, CancellationToken ct = default);
}
