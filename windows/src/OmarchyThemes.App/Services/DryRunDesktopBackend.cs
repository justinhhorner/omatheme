using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows;

namespace OmarchyThemes.App.Services;

/// <summary>
/// Used when OMARCHY_THEMES_DRY_RUN=1: reports what the Windows backend does (capabilities, accent note) but
/// changes nothing, so Apply and Restore can be exercised end to end without touching the desktop.
/// </summary>
public sealed class DryRunDesktopBackend : IDesktopBackend
{
    public DesktopCapabilities Capabilities => WindowsDesktopBackend.Supported;

    public string? AccentColorNote => WindowsDesktopBackend.AccentNote;

    public Task<DesktopSnapshot> CaptureAsync(CancellationToken ct = default) =>
        Task.FromResult(new DesktopSnapshot(DateTimeOffset.UtcNow, new Dictionary<string, string> { ["dryRun"] = "1" }));

    public Task RestoreAsync(DesktopSnapshot snapshot, CancellationToken ct = default) => Pretend(ct);

    public Task SetWallpaperAsync(string imagePath, WallpaperFit fit, RgbColor? fillColor, CancellationToken ct = default) => Pretend(ct);

    public Task SetAppearanceModeAsync(AppearanceMode mode, CancellationToken ct = default) => Pretend(ct);

    public Task SetAccentColorAsync(RgbColor accent, CancellationToken ct = default) => Pretend(ct);

    /// <summary>A short delay so progress states show up as they would for real.</summary>
    private static Task Pretend(CancellationToken ct) => Task.Delay(300, ct);
}
