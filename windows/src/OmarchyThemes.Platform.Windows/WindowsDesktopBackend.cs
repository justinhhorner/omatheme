using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows.Registry;
using OmarchyThemes.Platform.Windows.Wallpaper;

namespace OmarchyThemes.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="IDesktopBackend"/>. Everything is per-user (HKCU) and
/// needs no elevation.
/// <list type="bullet">
/// <item>Wallpaper: IDesktopWallpaper (SystemParametersInfo fallback).</item>
/// <item>Light/dark: Themes\Personalize AppsUseLightTheme + SystemUsesLightTheme.</item>
/// <item>Accent: DWM AccentColor/ColorizationColor + Explorer\Accent AccentPalette. Windows has
/// no public API for the accent, so this mirrors what the Settings app stores; some surfaces
/// only refresh after sign-out.</item>
/// </list>
/// </summary>
public sealed class WindowsDesktopBackend : IDesktopBackend
{
    internal const string PersonalizeKey = @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    internal const string DwmKey = @"Software\Microsoft\Windows\DWM";
    internal const string AccentKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
    internal const string DesktopKey = @"Control Panel\Desktop";

    /// <summary>Every registry value this backend may change, and therefore snapshots.</summary>
    internal static readonly (string Key, string Name)[] TrackedValues =
    [
        (PersonalizeKey, "AppsUseLightTheme"),
        (PersonalizeKey, "SystemUsesLightTheme"),
        (DwmKey, "AccentColor"),
        (DwmKey, "ColorizationColor"),
        (DwmKey, "ColorizationAfterglow"),
        (AccentKey, "AccentPalette"),
        (AccentKey, "AccentColorMenu"),
        (AccentKey, "StartColorMenu"),
        (DesktopKey, "AutoColorization"),
    ];

    private const string RegPrefix = "reg:";
    private const string MonitorPrefix = "monitor:";
    private const string CopyPrefix = "monitor-copy:";
    private const string FitKey = "wallpaper-fit";

    private readonly IWallpaperApi _wallpaper;
    private readonly IRegistryAccess _registry;
    private readonly ISettingsBroadcaster _broadcaster;
    private readonly IImageConverter _images;
    private readonly string _snapshotAssetsDir;
    private readonly TimeProvider _time;

    public WindowsDesktopBackend(
        IWallpaperApi wallpaper,
        IRegistryAccess registry,
        ISettingsBroadcaster broadcaster,
        IImageConverter images,
        string snapshotAssetsDir,
        TimeProvider? time = null)
    {
        _wallpaper = wallpaper;
        _registry = registry;
        _broadcaster = broadcaster;
        _images = images;
        _snapshotAssetsDir = snapshotAssetsDir;
        _time = time ?? TimeProvider.System;
    }

    /// <param name="snapshotAssetsDir">Where copies of the user's original wallpapers are kept for Restore.</param>
    public static WindowsDesktopBackend CreateDefault(string snapshotAssetsDir) => new(
        new DesktopWallpaperApi(), new CurrentUserRegistry(), new Win32SettingsBroadcaster(), new WicImageConverter(), snapshotAssetsDir);

    public DesktopCapabilities Capabilities =>
        DesktopCapabilities.Wallpaper | DesktopCapabilities.AppearanceMode | DesktopCapabilities.AccentColor;

    public async Task<DesktopSnapshot> CaptureAsync(CancellationToken ct = default)
    {
        var values = new Dictionary<string, string>();

        foreach (var (key, name) in TrackedValues)
            values[$"{RegPrefix}{key}|{name}"] = RegValue.Serialize(_registry.Read(key, name));

        var state = await _wallpaper.GetAsync().ConfigureAwait(false);
        if (state.Fit is { } fit)
            values[FitKey] = fit.ToString();

        // Keep a private copy of each original wallpaper: Windows' own copy (TranscodedWallpaper)
        // is overwritten when we set a new one, and the user may delete the original file later.
        if (Directory.Exists(_snapshotAssetsDir))
            Directory.Delete(_snapshotAssetsDir, recursive: true);
        var copies = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        for (var i = 0; i < state.Monitors.Count; i++)
        {
            var monitor = state.Monitors[i];
            values[MonitorPrefix + monitor.MonitorId] = monitor.Path;
            if (monitor.Path.Length == 0 || !File.Exists(monitor.Path))
                continue;
            if (!copies.TryGetValue(monitor.Path, out var copy))
            {
                Directory.CreateDirectory(_snapshotAssetsDir);
                copy = Path.Combine(_snapshotAssetsDir, $"{i}{ImageExtension(monitor.Path)}");
                File.Copy(monitor.Path, copy, overwrite: true);
                copies[monitor.Path] = copy;
            }
            values[CopyPrefix + monitor.MonitorId] = copy;
        }

        return new DesktopSnapshot(_time.GetUtcNow(), values);
    }

    public async Task RestoreAsync(DesktopSnapshot snapshot, CancellationToken ct = default)
    {
        foreach (var (key, name) in TrackedValues)
        {
            if (!snapshot.Values.TryGetValue($"{RegPrefix}{key}|{name}", out var serialized))
                continue;
            if (RegValue.Deserialize(serialized) is { } value)
                _registry.Write(key, name, value);
            else
                _registry.Delete(key, name);
        }
        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);

        var monitors = snapshot.Values
            .Where(kv => kv.Key.StartsWith(MonitorPrefix, StringComparison.Ordinal))
            .Select(kv =>
            {
                var id = kv.Key[MonitorPrefix.Length..];
                var path = kv.Value;
                if (path.Length > 0 && !File.Exists(path)
                    && snapshot.Values.TryGetValue(CopyPrefix + id, out var copy) && File.Exists(copy))
                    path = copy;
                return new MonitorWallpaper(id, path);
            })
            .ToList();
        if (monitors.Count > 0)
        {
            WallpaperFit? fit = snapshot.Values.TryGetValue(FitKey, out var f) && Enum.TryParse<WallpaperFit>(f, out var parsed) ? parsed : null;
            await _wallpaper.RestoreAsync(new WallpaperState(monitors, fit)).ConfigureAwait(false);
        }
    }

    public async Task SetWallpaperAsync(string imagePath, WallpaperFit fit, CancellationToken ct = default)
    {
        var usable = await _images.EnsureWallpaperFormatAsync(imagePath, ct).ConfigureAwait(false);
        ct.ThrowIfCancellationRequested();
        await _wallpaper.SetAsync(usable, fit).ConfigureAwait(false);
    }

    public Task SetAppearanceModeAsync(AppearanceMode mode, CancellationToken ct = default)
    {
        var light = RegValue.DWord(mode == AppearanceMode.Light ? 1u : 0u);
        _registry.Write(PersonalizeKey, "AppsUseLightTheme", light);
        _registry.Write(PersonalizeKey, "SystemUsesLightTheme", light);
        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);
        return Task.CompletedTask;
    }

    public Task SetAccentColorAsync(RgbColor accent, CancellationToken ct = default)
    {
        var color = AccentMath.NormalizeAccent(accent);
        var shades = AccentMath.Shades(color);

        // Stop Windows from re-deriving the accent from the wallpaper we may have just set.
        _registry.Write(DesktopKey, "AutoColorization", RegValue.DWord(0));

        _registry.Write(AccentKey, "AccentPalette", RegValue.Binary(AccentMath.ToAccentPaletteBytes(color)));
        _registry.Write(AccentKey, "AccentColorMenu", RegValue.DWord(AccentMath.ToAbgr(color)));
        _registry.Write(AccentKey, "StartColorMenu", RegValue.DWord(AccentMath.ToAbgr(shades[4])));

        _registry.Write(DwmKey, "AccentColor", RegValue.DWord(AccentMath.ToAbgr(color)));
        _registry.Write(DwmKey, "ColorizationColor", RegValue.DWord(AccentMath.ToArgb(color, 0xC4)));
        _registry.Write(DwmKey, "ColorizationAfterglow", RegValue.DWord(AccentMath.ToArgb(color, 0xC4)));

        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);
        return Task.CompletedTask;
    }

    private static string ImageExtension(string path)
    {
        // TranscodedWallpaper has no extension; Windows sniffs the content, so any image extension works.
        var ext = Path.GetExtension(path);
        return string.IsNullOrEmpty(ext) ? ".jpg" : ext;
    }
}
